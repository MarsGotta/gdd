//! The files of `.gdd/` that gdd reads and writes around an agent call, where the agent could
//! have left a link to a file outside the project, or a second name of one. gdd never reads or
//! writes through either: what stands where its folder or its file goes is removed, never what
//! it points to, and a file is written new, never over another name of it.

use std::fs::{self, File, FileType};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Why a file is not read: what is at its path is something else.
const NOT_OWN: &str = "not a file of its own (a link, a folder or a file with other names)";

/// The bytes of the file `path` of the project at `root`; `None` when there is none. A link or a
/// file on the way, or anything at `path` but a file with no other name, is an error.
pub fn read(root: &Path, path: &str) -> io::Result<Option<Vec<u8>>> {
    let (folders, file) = on_the_way(root, path);
    for folder in &folders {
        match kind(folder)? {
            None => return Ok(None),
            Some(found) if found.is_dir() => {}
            Some(_) => return Err(not_a_folder(root, folder)),
        }
    }
    match fs::symlink_metadata(&file) {
        Ok(meta) if meta.is_file() && meta.nlink() == 1 => fs::read(&file).map(Some),
        Ok(_) => Err(io::Error::other(NOT_OWN)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Makes `path` of the project at `root` a new file with `bytes`. What is in the way goes
/// first: a link or a file where a folder goes, and whatever is at `path`.
pub fn replace(root: &Path, path: &str, bytes: &[u8]) -> io::Result<()> {
    let (folders, file) = on_the_way(root, path);
    for folder in &folders {
        match kind(folder)? {
            Some(found) if found.is_dir() => continue,
            Some(_) => fs::remove_file(folder)?,
            None => {}
        }
        fs::create_dir(folder)?;
    }
    clear(&file)?;
    File::create_new(&file)?.write_all(bytes)
}

/// Leaves nothing at `path` of the project at `root`: a link on the way goes, not what it
/// points to.
pub fn remove(root: &Path, path: &str) -> io::Result<()> {
    let (folders, file) = on_the_way(root, path);
    for folder in &folders {
        match kind(folder)? {
            None => return Ok(()),
            Some(found) if found.is_dir() => {}
            Some(_) => return fs::remove_file(folder),
        }
    }
    clear(&file)
}

/// The folders between `root` and the file `path` of the project, outermost first, and the
/// file.
fn on_the_way(root: &Path, path: &str) -> (Vec<PathBuf>, PathBuf) {
    let file = root.join(path);
    let mut folders: Vec<PathBuf> = file
        .ancestors()
        .skip(1)
        .take_while(|folder| *folder != root)
        .map(Path::to_path_buf)
        .collect();
    folders.reverse();
    (folders, file)
}

/// What is at `path`, without following a link: `None` when there is nothing.
fn kind(path: &Path) -> io::Result<Option<FileType>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta.file_type())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Removes what is at `path`: a folder with all it holds, a link and not what it points to.
fn clear(path: &Path) -> io::Result<()> {
    match kind(path)? {
        None => Ok(()),
        Some(found) if found.is_dir() => fs::remove_dir_all(path),
        Some(_) => fs::remove_file(path),
    }
}

/// Why a file is not read when `folder`, on the way from `root`, is not a folder.
fn not_a_folder(root: &Path, folder: &Path) -> io::Error {
    let shown = folder.strip_prefix(root).unwrap_or(folder);
    io::Error::other(format!("{} is not a folder", shown.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;
    use std::os::unix::fs::symlink;

    /// A project and a folder outside it, each with the file `f` that says `outside`.
    fn two() -> (Folder, Folder) {
        let (project, outside) = (Folder::new(), Folder::new());
        outside.write("f", "outside");
        (project, outside)
    }

    fn text(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn only_a_file_of_its_own_is_read() {
        let (project, outside) = two();
        let root = &project.0;
        project.write(".gdd/a", "mine");
        assert_eq!(read(root, ".gdd/a").unwrap(), Some(b"mine".to_vec()));
        assert_eq!(read(root, ".gdd/none").unwrap(), None);
        assert_eq!(read(root, "nowhere/a").unwrap(), None);
        symlink(outside.0.join("f"), root.join(".gdd/link")).unwrap();
        fs::hard_link(outside.0.join("f"), root.join(".gdd/hard")).unwrap();
        fs::create_dir(root.join(".gdd/folder")).unwrap();
        for path in [".gdd/link", ".gdd/hard", ".gdd/folder"] {
            let err = read(root, path).unwrap_err();
            assert_eq!(err.to_string(), NOT_OWN, "{path}");
        }
        symlink(&outside.0, root.join(".gdd/out")).unwrap();
        let err = read(root, ".gdd/out/f").unwrap_err();
        assert_eq!(err.to_string(), ".gdd/out is not a folder");
        project.write("file", "");
        let err = read(root, "file/a").unwrap_err();
        assert_eq!(err.to_string(), "file is not a folder");
    }

    #[test]
    fn a_path_that_cannot_be_looked_at_is_an_error_not_nothing() {
        let project = Folder::new();
        let root = &project.0;
        project.write(".gdd/a", "mine");
        let long = "x".repeat(300);
        for path in [format!(".gdd/{long}"), format!(".gdd/{long}/a")] {
            assert!(read(root, &path).is_err(), "{path}");
            assert!(remove(root, &path).is_err(), "{path}");
        }
    }

    #[test]
    fn a_file_is_written_new_and_never_through_a_link() {
        let (project, outside) = two();
        let root = &project.0;
        replace(root, ".gdd/c1/new.md", b"one").unwrap();
        assert_eq!(text(&root.join(".gdd/c1/new.md")), "one");
        replace(root, ".gdd/c1/new.md", b"two").unwrap();
        assert_eq!(text(&root.join(".gdd/c1/new.md")), "two");
        symlink(outside.0.join("f"), root.join(".gdd/link")).unwrap();
        fs::hard_link(outside.0.join("f"), root.join(".gdd/hard")).unwrap();
        fs::create_dir_all(root.join(".gdd/folder/inside")).unwrap();
        symlink(&outside.0, root.join(".gdd/out")).unwrap();
        project.write(".gdd/file", "");
        for path in [
            ".gdd/link",
            ".gdd/hard",
            ".gdd/folder",
            ".gdd/out/f",
            ".gdd/file/a",
        ] {
            replace(root, path, b"gdd").unwrap();
            assert_eq!(read(root, path).unwrap(), Some(b"gdd".to_vec()), "{path}");
        }
        assert_eq!(text(&outside.0.join("f")), "outside");
    }

    #[test]
    fn what_is_removed_is_never_what_a_link_points_to() {
        let (project, outside) = two();
        let root = &project.0;
        project.write(".gdd/a", "mine");
        symlink(outside.0.join("f"), root.join(".gdd/link")).unwrap();
        fs::create_dir_all(root.join(".gdd/folder/inside")).unwrap();
        symlink(&outside.0, root.join(".gdd/out")).unwrap();
        for path in [".gdd/a", ".gdd/link", ".gdd/folder", ".gdd/out/f"] {
            remove(root, path).unwrap();
            assert_eq!(read(root, path).unwrap(), None, "{path}");
        }
        assert!(!root.join(".gdd/out").exists());
        assert_eq!(text(&outside.0.join("f")), "outside");
        assert!(remove(root, ".gdd/none").is_ok());
        assert!(remove(root, "nowhere/a").is_ok());
    }
}
