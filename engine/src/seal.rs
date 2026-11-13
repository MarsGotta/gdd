//! Sealing: committing what judges a change on `gdd-seal/<id>`, without touching the checkout or
//! its index. It writes git objects through a throwaway index and moves only the seal's ref.

use crate::git;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Sealed {
    pub reference: String,
    pub parent: String,
    pub commit: String,
    /// Each file with its blob hash, as sealed.
    pub files: Vec<(String, String)>,
    /// Nothing changed: the seal already held these files as they are.
    pub unchanged: bool,
}

pub fn seal_ref(id: &str) -> String {
    format!("refs/heads/gdd-seal/{id}")
}

/// Commit some files of the working copy on the seal of a change, on top of `parent` (the seal's
/// tip, or the base when there is no seal yet), and check each sealed blob is the working copy's.
pub fn seal(
    root: &Path,
    id: &str,
    parent: &str,
    files: &[String],
    message: &str,
) -> Result<Sealed, String> {
    if files.is_empty() {
        return Err("name the files to seal: the brief and its scenarios".into());
    }
    commit_on(root, &seal_ref(id), parent, files, message)
}

/// Commit some files of the working copy on a branch, on top of `parent`, through a throwaway
/// index: neither the checkout nor its index is touched, and only that branch's ref moves.
pub fn commit_on(
    root: &Path,
    reference: &str,
    parent: &str,
    files: &[String],
    message: &str,
) -> Result<Sealed, String> {
    let reference = reference.to_string();
    let old = git::commit_of(root, &reference);
    if let Some(o) = &old
        && o != parent
    {
        return Err(format!(
            "{reference} is at {o}, not at {parent}: seal on top of its tip"
        ));
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let index = dir.path().join("index").to_string_lossy().to_string();
    let env = [("GIT_INDEX_FILE", index.as_str())];
    git::out_env(root, &["read-tree", parent], &env)?;
    let mut sealed = vec![];
    for f in files {
        let path = root.join(f);
        if !path.is_file() {
            return Err(format!("{f} is not a file of the working copy"));
        }
        let hash = git::out(root, &["hash-object", "-w", "--", f])?;
        let mode = mode_of(&path);
        git::out_env(
            root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("{mode},{hash},{f}"),
            ],
            &env,
        )?;
        sealed.push((f.clone(), hash));
    }
    let tree = git::out_env(root, &["write-tree"], &env)?;
    let parent_tree = git::out(root, &["rev-parse", &format!("{parent}^{{tree}}")])?;
    if tree == parent_tree {
        return Ok(Sealed {
            reference,
            parent: parent.into(),
            commit: parent.into(),
            files: sealed,
            unchanged: true,
        });
    }
    let commit = git::out(root, &["commit-tree", &tree, "-p", parent, "-m", message])?;
    let zero = "0".repeat(parent.len());
    let expected_old = old.unwrap_or(zero);
    git::out(root, &["update-ref", &reference, &commit, &expected_old])?;
    for (f, h) in &sealed {
        let at = git::blob_at(root, &commit, f);
        if at.as_deref() != Some(h.as_str()) {
            return Err(format!("{f} at {commit} is not the working copy's {h}"));
        }
    }
    Ok(Sealed {
        reference,
        parent: parent.into(),
        commit,
        files: sealed,
        unchanged: false,
    })
}

#[cfg(unix)]
fn mode_of(path: &Path) -> &'static str {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(m) if m.permissions().mode() & 0o111 != 0 => "100755",
        _ => "100644",
    }
}

#[cfg(not(unix))]
fn mode_of(_: &Path) -> &'static str {
    "100644"
}

/// Merge the main branch into the seal, through `git merge-tree`, without a checkout. Returns the
/// new tip, or the old one when the main branch is already in it.
pub fn merge_main(root: &Path, id: &str, main: &str) -> Result<String, String> {
    let reference = seal_ref(id);
    let tip = git::commit_of(root, &reference).ok_or(format!("{reference} doesn't exist"))?;
    let main_tip = git::commit_of(root, main).ok_or(format!("{main} doesn't exist"))?;
    if git::is_ancestor(root, &main_tip, &tip) {
        return Ok(tip);
    }
    let o = git::run(
        root,
        &["merge-tree", "--write-tree", "--name-only", &tip, &main_tip],
    )?;
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    if !o.status.success() {
        let conflicts: Vec<&str> = text.lines().skip(1).filter(|l| !l.is_empty()).collect();
        return Err(format!(
            "merging {main} into the seal conflicts in {}: whoever wrote what conflicts settles it",
            conflicts.join(", ")
        ));
    }
    let tree = text.lines().next().unwrap_or("").trim().to_string();
    let msg = format!("Merge {main} into the seal of {id}");
    let commit = git::out(
        root,
        &[
            "commit-tree",
            &tree,
            "-p",
            &tip,
            "-p",
            &main_tip,
            "-m",
            &msg,
        ],
    )?;
    git::out(root, &["update-ref", &reference, &commit, &tip])?;
    Ok(commit)
}
