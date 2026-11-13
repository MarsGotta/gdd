//! The few git commands the layer needs, each one a small function. It never fetches, pushes or
//! deletes a branch: every command here reads, or writes objects and the refs of a seal.

use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Whether a git answers one command once: the hash of the empty blob, on one line. On macOS, the
/// launcher at `/usr/bin/git` and the git inside the full developer tools were seen, from inside a
/// sandbox that keeps them from writing their cache, running each command three times and exiting 0
/// after a fatal error: lists came back tripled and a throwaway index was never written.
fn answers_once(git: &str) -> bool {
    Command::new(git)
        .args(["hash-object", "-t", "blob", "--stdin"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            let mut lines = out.lines();
            o.status.success()
                && lines
                    .next()
                    .is_some_and(|h| h.len() >= 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
                && lines.next().is_none()
        })
}

/// The git to run: `GDD_GIT` when set, else the first of the one on the path and the usual places
/// of a real one that answers a command once. With none, every git command is refused, loudly:
/// a layer that keeps the evidence can't run on a git that repeats itself.
fn program() -> Result<&'static str, String> {
    static GIT: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    GIT.get_or_init(|| {
        if let Ok(g) = std::env::var("GDD_GIT")
            && !g.is_empty()
        {
            return Ok(g);
        }
        let mut tries: Vec<String> = vec![];
        let on_path = std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("git"))
                .find(|g| g.is_file())
        });
        let launcher = on_path.as_deref() == Some(Path::new("/usr/bin/git"));
        if !launcher {
            tries.push("git".into());
        }
        tries.push("/Library/Developer/CommandLineTools/usr/bin/git".into());
        tries.push("/opt/homebrew/bin/git".into());
        tries.push("/usr/local/bin/git".into());
        if launcher {
            tries.push("git".into());
        }
        for g in &tries {
            if answers_once(g) {
                return Ok(g.clone());
            }
        }
        Err(format!(
            "no git here answers a command once (tried {}): set GDD_GIT to a real git",
            tries.join(", ")
        ))
    })
    .as_ref()
    .map(String::as_str)
    .map_err(Clone::clone)
}

fn base(root: &Path) -> Result<Command, String> {
    let mut c = Command::new(program()?);
    c.arg("-C")
        .arg(root)
        .args(["-c", "core.quotepath=off"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null());
    Ok(c)
}

/// With `GDD_TRACE_GIT` set, say on standard error what each git command was given and gave back.
fn trace(args: &[&str], o: &Output) {
    if std::env::var_os("GDD_TRACE_GIT").is_some() {
        eprintln!(
            "gdd-git {:?}: exit {:?}, stdout {:?}, stderr {:?}",
            args,
            o.status.code(),
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
    }
}

pub fn run(root: &Path, args: &[&str]) -> Result<Output, String> {
    let o = base(root)?
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    trace(args, &o);
    Ok(o)
}

pub fn run_env(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<Output, String> {
    let mut c = base(root)?;
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    trace(args, &o);
    Ok(o)
}

/// Run git and return its output, trimmed, or what it said on failure.
pub fn out(root: &Path, args: &[&str]) -> Result<String, String> {
    let o = run(root, args)?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        ))
    }
}

pub fn out_env(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
    let o = run_env(root, args, env)?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        ))
    }
}

fn z_list(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

pub fn is_repo(root: &Path) -> bool {
    run(root, &["rev-parse", "--git-dir"]).is_ok_and(|o| o.status.success())
}

/// The commit a name points to, or `None`.
pub fn commit_of(root: &Path, rev: &str) -> Option<String> {
    let spec = format!("{rev}^{{commit}}");
    out(root, &["rev-parse", "--verify", "--quiet", &spec])
        .ok()
        .filter(|s| !s.is_empty())
}

/// The blob of a path at a commit, or `None`.
pub fn blob_at(root: &Path, commit: &str, path: &str) -> Option<String> {
    let spec = format!("{commit}:{path}");
    out(root, &["rev-parse", "--verify", "--quiet", &spec])
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn show(root: &Path, commit: &str, path: &str) -> Option<String> {
    let spec = format!("{commit}:{path}");
    let o = run(root, &["show", &spec]).ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The bytes of a path at a commit, or `None`.
pub fn blob_bytes(root: &Path, commit: &str, path: &str) -> Option<Vec<u8>> {
    let spec = format!("{commit}:{path}");
    let o = run(root, &["cat-file", "blob", &spec]).ok()?;
    o.status.success().then_some(o.stdout)
}

/// `git hash-object <file>`, the blob hash of a file in the working copy.
pub fn hash_object(root: &Path, path: &str) -> Result<String, String> {
    out(root, &["hash-object", "--", path])
}

pub fn is_ancestor(root: &Path, a: &str, b: &str) -> bool {
    run(root, &["merge-base", "--is-ancestor", a, b]).is_ok_and(|o| o.status.success())
}

pub fn merge_base(root: &Path, a: &str, b: &str) -> Option<String> {
    out(root, &["merge-base", a, b])
        .ok()
        .filter(|s| !s.is_empty())
}

/// The paths that differ between two commits, `git diff --name-only a b`.
pub fn diff_names(root: &Path, a: &str, b: &str) -> Result<Vec<String>, String> {
    let o = run(
        root,
        &["diff", "--name-only", "-z", "--no-renames", a, b, "--"],
    )?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(z_list(&o.stdout))
}

/// The paths that differ between two commits within some pathspecs.
pub fn diff_names_in(
    root: &Path,
    a: &str,
    b: &str,
    specs: &[String],
) -> Result<Vec<String>, String> {
    let mut args = vec!["diff", "--name-only", "-z", "--no-renames", a, b, "--"];
    args.extend(specs.iter().map(String::as_str));
    let o = run(root, &args)?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(z_list(&o.stdout))
}

/// Files changed, lines added and lines taken out between two commits.
pub fn shortstat(root: &Path, a: &str, b: &str) -> Option<(i64, i64, i64)> {
    let o = out(root, &["diff", "--numstat", "--no-renames", a, b]).ok()?;
    let (mut files, mut add, mut del) = (0, 0, 0);
    for line in o.lines() {
        let mut parts = line.split('\t');
        let (x, y) = (parts.next()?, parts.next()?);
        files += 1;
        add += x.parse::<i64>().unwrap_or(0);
        del += y.parse::<i64>().unwrap_or(0);
    }
    Some((files, add, del))
}

/// The lines added and taken out between two commits, without the orchestrator's bookkeeping,
/// as `git diff --shortstat` counts them.
pub fn changed_lines(root: &Path, a: &str, b: &str) -> Option<i64> {
    let o = out(
        root,
        &[
            "diff",
            "--numstat",
            "--no-renames",
            a,
            b,
            "--",
            ".",
            ":(exclude)gdd/log",
            ":(exclude)gdd/backlog.md",
        ],
    )
    .ok()?;
    let mut n = 0;
    for line in o.lines() {
        let mut parts = line.split('\t');
        n += parts.next()?.parse::<i64>().unwrap_or(0);
        n += parts.next()?.parse::<i64>().unwrap_or(0);
    }
    Some(n)
}

/// The tracked files of the index that match some pathspecs, `git ls-files -- <pathspec>...`.
pub fn ls_files(root: &Path, specs: &[String]) -> Result<Vec<String>, String> {
    let mut args = vec!["ls-files", "-z", "--"];
    args.extend(specs.iter().map(String::as_str));
    let o = run(root, &args)?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(z_list(&o.stdout))
}

/// Which of some paths, existing or not, git's own pathspec matching puts under some pathspecs.
/// It builds a throwaway index that holds just those paths and asks `git ls-files` about it, so the
/// answer is git's, `:(glob)` and every other magic included, and the real index is never touched.
pub fn match_paths(root: &Path, paths: &[String], specs: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() || specs.is_empty() {
        return Ok(vec![]);
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let index = dir.path().join("index");
    let index = index.to_string_lossy().to_string();
    let empty = out(root, &["hash-object", "-t", "blob", "--stdin"])
        .ok()
        .filter(|h| h.len() >= 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391".into());
    let mut input = String::new();
    for p in paths {
        input.push_str(&format!("100644 {empty}\t{p}\0"));
    }
    let mut c = base(root)?;
    c.env("GIT_INDEX_FILE", &index)
        .args(["update-index", "--add", "-z", "--index-info"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().ok_or("no stdin for git")?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    trace(&["update-index", "--index-info", &input], &o);
    if !o.status.success() {
        return Err(format!(
            "git update-index: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    // A wrong answer here is silent: paths that never reached the throwaway index look like paths
    // no zone covers. So look before asking.
    let held = run_env(root, &["ls-files", "-z"], &[("GIT_INDEX_FILE", &index)])?;
    if !held.status.success() || z_list(&held.stdout).len() != paths.len() {
        return Err(format!(
            "git did not take {} paths into a throwaway index (it holds {}): {}",
            paths.len(),
            z_list(&held.stdout).len(),
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    let mut args = vec!["ls-files", "-z", "--"];
    args.extend(specs.iter().map(String::as_str));
    let o = run_env(root, &args, &[("GIT_INDEX_FILE", &index)])?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(z_list(&o.stdout))
}

/// Whether git can write a ref here: it creates a throwaway ref, `refs/gdd-probe/<random>`, and
/// deletes it. A repository whose `.git` can't be written passes every read and fails here, before
/// anything is sealed or built.
pub fn can_write_ref(root: &Path) -> Result<(), String> {
    let commit = commit_of(root, "HEAD")
        .or_else(|| commit_of(root, &main_branch(root)))
        .ok_or("the repository has no commit yet")?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let name = format!("refs/gdd-probe/{}-{nanos}", std::process::id());
    out(root, &["update-ref", &name, &commit])?;
    out(root, &["update-ref", "-d", &name, &commit])?;
    Ok(())
}

/// The main branch: `GDD_MAIN` when set, else `main`, else `master`.
pub fn main_branch(root: &Path) -> String {
    if let Ok(m) = std::env::var("GDD_MAIN")
        && !m.is_empty()
    {
        return m;
    }
    for name in ["main", "master"] {
        if commit_of(root, &format!("refs/heads/{name}")).is_some() {
            return name.into();
        }
    }
    "main".into()
}

/// Whether a working copy is at a commit with nothing changed and nothing untracked.
pub fn clean_at(dir: &Path, commit: &str) -> Result<(), String> {
    let head = commit_of(dir, "HEAD").ok_or("it is not a git checkout")?;
    if head != commit {
        return Err(format!("it is at {head}, not at {commit}"));
    }
    let status = out(dir, &["status", "--porcelain", "--untracked-files=all"])?;
    if !status.is_empty() {
        return Err(format!(
            "its tree isn't clean: {}",
            status.lines().take(3).collect::<Vec<_>>().join("; ")
        ));
    }
    Ok(())
}

pub fn worktree_add(root: &Path, dir: &Path, commit: &str) -> Result<(), String> {
    let d = dir.to_string_lossy().to_string();
    out(
        root,
        &["worktree", "add", "--detach", "--quiet", &d, commit],
    )
    .map(|_| ())
}

pub fn worktree_remove(root: &Path, dir: &Path) -> Result<(), String> {
    let d = dir.to_string_lossy().to_string();
    let r = out(root, &["worktree", "remove", "--force", &d]).map(|_| ());
    let _ = out(root, &["worktree", "prune"]);
    r
}

/// The first branch of a change that exists: `work/<id>`, `gdd-build/<id>` or `build/<id>`.
pub fn change_branch(root: &Path, id: &str) -> Option<String> {
    ["work", "gdd-build", "build"]
        .iter()
        .map(|p| format!("{p}/{id}"))
        .find(|b| commit_of(root, &format!("refs/heads/{b}")).is_some())
}
