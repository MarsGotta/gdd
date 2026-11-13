//! The private folder where a run keeps what it leaves for later: the gates' outputs, a role's
//! prompt and output, the files a merge sets aside.
//!
//! It is one user's, never shared. On a machine whose temporary folder other users write to as
//! well, someone else could create the folder first, or plant links in it, and the layer would
//! write where they chose. So every level of it must be a real directory of this user's that no
//! one else can write, and every file in it is created new: a link planted at its path is never
//! followed, and nothing already there is truncated.

use std::io;
use std::path::{Path, PathBuf};

/// The folder of one run, `<base>/<project>-<hash>/<id>/<what>-<stamp>-<random>`, created now.
/// `<base>` is `GDD_RUNS_DIR` when it is set, else `gdd-runs-<uid>` in the temporary folder. When
/// `<base>` or a level under it fails the checks, the run goes to a fresh private folder instead,
/// and the second value says where and why.
pub fn for_run(
    root: &Path,
    id: &str,
    what: &str,
    stamp: &str,
) -> io::Result<(PathBuf, Option<String>)> {
    let base = match std::env::var("GDD_RUNS_DIR") {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => std::env::temp_dir().join(format!("gdd-runs-{}", user())),
    };
    let project = base.join(project_folder(root));
    let change = project.join(id);
    let mut note = None;
    let parent = match private(&base)
        .and_then(|_| private(&project))
        .and_then(|_| private(&change))
    {
        Ok(()) => change,
        Err(why) => {
            let fresh = private_folder("gdd-runs-").tempdir()?.keep();
            note = Some(format!(
                "{why}: the outputs go to a fresh private folder instead, {}",
                fresh.display()
            ));
            fresh
        }
    };
    let leaf = private_folder(&format!("{what}-{stamp}-"))
        .tempdir_in(&parent)?
        .keep();
    Ok((leaf, note))
}

/// A new folder with a random name that no one had, readable by this user only: `tempfile` makes
/// folders with the default mode unless told.
fn private_folder(prefix: &str) -> tempfile::Builder<'_, 'static> {
    let mut b = tempfile::Builder::new();
    b.prefix(prefix);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        b.permissions(std::fs::Permissions::from_mode(0o700));
    }
    b
}

/// The folder of a project under the base: its name and a hash of its path, so two projects
/// with the same name never share one.
fn project_folder(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());
    let mut h: u64 = 1469598103934665603;
    for b in root.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    format!("{name}-{:08x}", h as u32)
}

/// Create a file that must not exist yet, readable by this user only.
pub fn create_new(path: &Path) -> io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)
}

/// Write a file that must not exist yet.
pub fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    create_new(path)?.write_all(bytes)
}

/// Copy a file to a path that must not exist yet.
pub fn copy_new(from: &Path, to: &Path) -> io::Result<u64> {
    let mut src = std::fs::File::open(from)?;
    let mut dst = create_new(to)?;
    io::copy(&mut src, &mut dst)
}

/// Create the folders down to `dir` inside a run's folder, readable by this user only.
pub fn create_dirs(dir: &Path) -> io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)
}

#[cfg(unix)]
fn user() -> u32 {
    // SAFETY: geteuid takes nothing, can't fail and touches no memory of ours.
    unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "user".into())
}

/// Make sure `dir` is a folder only this user can write: create it with mode 0700 when it isn't
/// there; when it is, it must be a real directory, not a link, owned by this user, and not
/// writable by the group or by others. Else, why not, in words.
fn private(dir: &Path) -> Result<(), String> {
    let shown = dir.display();
    match std::fs::symlink_metadata(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Err(e) = create_dirs(dir)
                && e.kind() != io::ErrorKind::AlreadyExists
            {
                return Err(format!("{shown} can't be created: {e}"));
            }
        }
        Err(e) => return Err(format!("{shown} can't be read: {e}")),
        Ok(_) => {}
    }
    let m = std::fs::symlink_metadata(dir).map_err(|e| format!("{shown} can't be read: {e}"))?;
    if m.file_type().is_symlink() {
        return Err(format!("{shown} is a link, not a folder of its own"));
    }
    if !m.is_dir() {
        return Err(format!("{shown} is not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if m.uid() != user() {
            return Err(format!(
                "{shown} belongs to another user (uid {}, not {})",
                m.uid(),
                user()
            ));
        }
        if m.mode() & 0o022 != 0 {
            return Err(format!(
                "others can write in {shown} (mode {:o})",
                m.mode() & 0o777
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_created_new_and_never_through_a_link() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("out.txt");
        write_new(&p, b"one").unwrap();
        // Nothing that is already there is truncated.
        assert!(write_new(&p, b"two").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"one");
        // A planted link is never followed, even one that points nowhere yet.
        #[cfg(unix)]
        {
            let target = d.path().join("victim.txt");
            let link = d.path().join("planted.txt");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(write_new(&link, b"x").is_err());
            assert!(!target.exists());
            std::fs::write(&target, "kept").unwrap();
            assert!(copy_new(&p, &link).is_err());
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "kept");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_is_a_link_or_open_to_others_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let mine = d.path().join("mine");
        private(&mine).unwrap();
        let mode = std::fs::metadata(&mine).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let link = d.path().join("link");
        std::os::unix::fs::symlink(&mine, &link).unwrap();
        assert!(private(&link).unwrap_err().contains("is a link"));
        let open = d.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(private(&open).unwrap_err().contains("others can write"));
        let file = d.path().join("file");
        std::fs::write(&file, "").unwrap();
        assert!(private(&file).unwrap_err().contains("not a folder"));
    }
}
