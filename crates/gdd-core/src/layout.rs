//! Where a project keeps its files and its state: in `tminus/` and `.tminus/`, as tminus sets a
//! project up, or in `gdd/` and `.gdd/`, as a project set up before the tool was called tminus
//! keeps them. The tool reads and writes each project where it already keeps things, and moves
//! nothing.
//!
//! Every path the tool knows is written in the new names (`tminus/zones.toml`,
//! `.tminus/changes`...) and resolved against the folder it is joined to: the project root for
//! the files of the project, the state root (see [`crate::state::root`]) for the state. What the
//! tool writes into a project of the old layout (its agents' commands, its commits, its workflow)
//! is what it wrote before the rename: [`Layout::localize`] is the exact reverse of the rename.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// The file that marks a project of the new layout, from its root.
const NEW_PROJECT: &str = "tminus/project.toml";

/// The file that marks a project of the old layout, from its root.
const OLD_PROJECT: &str = "gdd/project.toml";

/// Where a project keeps its files and its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `tminus/` and `.tminus/`: every project tminus sets up.
    Tminus,
    /// `gdd/` and `.gdd/`: a project set up before the rename.
    Gdd,
}

impl Layout {
    /// The layout of the project at `folder`: [`Layout::Gdd`] when `folder/gdd/project.toml` is
    /// a file and `folder/tminus/project.toml` is not; else [`Layout::Tminus`]. A file that
    /// cannot be looked at (see [`check`], which refuses it first) counts as there.
    pub fn at(folder: &Path) -> Layout {
        let old = is_file(&folder.join(OLD_PROJECT)).unwrap_or(true);
        let new = is_file(&folder.join(NEW_PROJECT)).unwrap_or(true);
        if old && !new {
            Layout::Gdd
        } else {
            Layout::Tminus
        }
    }

    /// The name of the tool in this layout, which its folders, its agents' commands, its
    /// commits and its variables take: `tminus` or `gdd`.
    pub fn name(self) -> &'static str {
        match self {
            Layout::Tminus => "tminus",
            Layout::Gdd => "gdd",
        }
    }

    /// `text`, written in the new names, as a project of this layout names things: the same in
    /// the new layout. In the old one, every `tminus` that touches no ASCII letter, digit or `_`
    /// becomes `gdd`, and every `TMINUS_` that follows none of them and comes before a capital
    /// letter becomes `GDD_`. Nothing else changes: it is the exact reverse of the rename.
    pub fn localize(self, text: &str) -> String {
        match self {
            Layout::Tminus => text.to_string(),
            Layout::Gdd => to_old_names(text),
        }
    }

    /// `path`, written in the new names (`.tminus/changes`, `tminus/zones.toml`...), as a
    /// project of this layout names it (see [`Layout::localize`]).
    pub fn path(self, path: &str) -> String {
        self.localize(path)
    }

    /// The variable `name`, written in the new names (`TMINUS_CHANGE`...), as the tool sets it
    /// for what it starts in a project of this layout (see [`Layout::localize`]).
    pub fn variable(self, name: &str) -> String {
        self.localize(name)
    }
}

/// The twin of the variable `name` in the other layout: `GDD_X` for `TMINUS_X` and `TMINUS_X`
/// for `GDD_X`; `None` for any other name. What the tool starts gets the names of the project's
/// layout only: the twin goes, so an inherited one never overrides what the tool set.
pub fn twin(name: &str) -> Option<String> {
    let rest = |prefix: &str| name.strip_prefix(prefix).filter(|rest| !rest.is_empty());
    if let Some(rest) = rest("TMINUS_") {
        Some(format!("GDD_{rest}"))
    } else {
        rest("GDD_").map(|rest| format!("TMINUS_{rest}"))
    }
}

/// `root` joined with `path`, written in the new names, where the project at `root` keeps it
/// (see [`Layout::at`]): `root` is the project root for the files of the project and the state
/// root for the state.
pub fn join(root: &Path, path: &str) -> PathBuf {
    root.join(named(root, path))
}

/// `path`, written in the new names, as the project at `root` names it (see [`Layout::at`]):
/// what the messages about it show.
pub fn named(root: &Path, path: &str) -> String {
    Layout::at(root).path(path)
}

/// `Ok` when the project at `project_root`, whose state root is `state_root`, keeps one layout:
/// refused when a folder has both `tminus/project.toml` and `gdd/project.toml`, or when the state
/// root also has the state folder of the other layout (`.gdd` in the new one, `.tminus` in the
/// old). The tool never picks one of two places to read approvals and records from. A file that
/// cannot be looked at is refused too, with its path and why.
pub fn check(project_root: &Path, state_root: &Path) -> Result<(), String> {
    one_project(project_root)?;
    if state_root != project_root {
        one_project(state_root)?;
    }
    let (files, other, own, kind) = match Layout::at(state_root) {
        Layout::Tminus => ("tminus/", ".gdd", ".tminus", "old"),
        Layout::Gdd => ("gdd/", ".tminus", ".gdd", "new"),
    };
    let path = state_root.join(other);
    match fs::symlink_metadata(&path) {
        Ok(_) => Err(format!(
            "{} keeps its files in {files} but has {other}/, the state of the {kind} layout: move \
             {other} to {own} or remove it",
            state_root.display()
        )),
        Err(e) if absent(&e) => Ok(()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// `Ok` unless `folder` has both `tminus/project.toml` and `gdd/project.toml`, or one of them
/// cannot be looked at.
fn one_project(folder: &Path) -> Result<(), String> {
    let mut found = Vec::new();
    for file in [NEW_PROJECT, OLD_PROJECT] {
        let path = folder.join(file);
        let there = is_file(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if there {
            found.push(file);
        }
    }
    if found.len() == 2 {
        return Err(format!(
            "{} has both {NEW_PROJECT} and {OLD_PROJECT}: keep one of them",
            folder.display()
        ));
    }
    Ok(())
}

/// Whether `path` is a file, following symbolic links: not when nothing is there, or when a
/// folder on the way is a file. Any other error is an error.
fn is_file(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.is_file()),
        Err(e) if absent(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Whether `error` says that nothing is there: the path does not exist, or a folder on the way
/// is a file.
fn absent(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

/// Whether `byte` is an ASCII letter, an ASCII digit or `_`: what a name touching it would
/// belong to.
fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// `text` in the old names (see [`Layout::localize`]): scanned by hand, a byte at a time.
fn to_old_names(text: &str) -> String {
    const NEW: &[u8] = b"tminus";
    const NEW_PREFIX: &[u8] = b"TMINUS_";
    let bytes = text.as_bytes();
    let mut renamed = String::with_capacity(text.len());
    let (mut at, mut copied) = (0, 0);
    while at < bytes.len() {
        let free_before = at == 0 || !word(bytes[at - 1]);
        let rest = &bytes[at..];
        let (found, old) = if !free_before {
            (0, "")
        } else if rest.starts_with(NEW) && !rest.get(NEW.len()).is_some_and(|&b| word(b)) {
            (NEW.len(), "gdd")
        } else if rest.starts_with(NEW_PREFIX)
            && rest.get(NEW_PREFIX.len()).is_some_and(u8::is_ascii_uppercase)
        {
            (NEW_PREFIX.len(), "GDD_")
        } else {
            (0, "")
        };
        if found == 0 {
            at += 1;
            continue;
        }
        // Both names are ASCII: `at` and `at + found` are on character boundaries.
        renamed.push_str(&text[copied..at]);
        renamed.push_str(old);
        at += found;
        copied = at;
    }
    renamed.push_str(&text[copied..]);
    renamed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    #[test]
    fn a_folder_with_only_gdd_project_toml_keeps_the_old_layout() {
        let folder = Folder::new();
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
        folder.write("gdd/project.toml", "name = \"x\"\n");
        assert_eq!(Layout::at(&folder.0), Layout::Gdd);
        folder.write("tminus/project.toml", "name = \"x\"\n");
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
        fs::remove_file(folder.0.join("gdd/project.toml")).unwrap();
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
    }

    #[test]
    fn a_project_toml_that_is_not_a_file_is_not_there() {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join("gdd/project.toml")).unwrap();
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
        assert_eq!(check(&folder.0, &folder.0), Ok(()));
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        // A file named `tminus` is no folder: nothing is under it.
        old.write("tminus", "");
        assert_eq!(Layout::at(&old.0), Layout::Gdd);
        assert_eq!(check(&old.0, &old.0), Ok(()));
        let new = Folder::new();
        new.write("tminus/project.toml", "");
        fs::create_dir_all(new.0.join("tminus/x")).unwrap();
        new.write("gdd", "");
        assert_eq!(Layout::at(&new.0), Layout::Tminus);
        assert_eq!(check(&new.0, &new.0), Ok(()));
    }

    #[test]
    fn a_project_toml_that_cannot_be_looked_at_counts_as_there_and_is_refused() {
        // A link to itself: looking under it fails, neither absent nor a file.
        let folder = Folder::new();
        folder.write("gdd/project.toml", "");
        std::os::unix::fs::symlink("tminus", folder.0.join("tminus")).unwrap();
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
        let err = check(&folder.0, &folder.0).unwrap_err();
        let path = folder.0.join("tminus/project.toml");
        let start = format!("cannot read {}: ", path.display());
        assert!(err.starts_with(&start), "{err}");
        let other = Folder::new();
        other.write("tminus/project.toml", "");
        std::os::unix::fs::symlink("gdd", other.0.join("gdd")).unwrap();
        assert_eq!(Layout::at(&other.0), Layout::Tminus);
        let err = check(&other.0, &other.0).unwrap_err();
        let path = other.0.join("gdd/project.toml");
        assert!(err.starts_with(&format!("cannot read {}: ", path.display())), "{err}");
        let looped = Folder::new();
        std::os::unix::fs::symlink("gdd", looped.0.join("gdd")).unwrap();
        assert_eq!(Layout::at(&looped.0), Layout::Gdd);
        assert!(check(&looped.0, &looped.0).is_err());
    }

    #[test]
    fn the_name_of_each_layout() {
        assert_eq!(Layout::Tminus.name(), "tminus");
        assert_eq!(Layout::Gdd.name(), "gdd");
    }

    #[test]
    fn each_variable_is_named_as_the_layout_sets_it_and_has_a_twin() {
        assert_eq!(Layout::Tminus.variable("TMINUS_CHANGE"), "TMINUS_CHANGE");
        assert_eq!(Layout::Gdd.variable("TMINUS_CHANGE"), "GDD_CHANGE");
        assert_eq!(twin("TMINUS_CHANGE").as_deref(), Some("GDD_CHANGE"));
        assert_eq!(twin("GDD_DIFF_FILES").as_deref(), Some("TMINUS_DIFF_FILES"));
        for none in ["TMINUS_", "GDD_", "PATH", "MY_GDD_X", "gdd_x", "XTMINUS_A", ""] {
            assert_eq!(twin(none), None, "{none:?}");
        }
    }

    #[test]
    fn a_folder_with_both_kinds_of_project_is_refused() {
        let folder = Folder::new();
        folder.write("tminus/project.toml", "");
        folder.write("gdd/project.toml", "");
        let both = format!(
            "{} has both tminus/project.toml and gdd/project.toml: keep one of them",
            folder.0.display()
        );
        assert_eq!(check(&folder.0, &folder.0), Err(both.clone()));
        // Also when the folder is the state root of a project somewhere else.
        let worktree = Folder::new();
        worktree.write("tminus/project.toml", "");
        assert_eq!(check(&worktree.0, &folder.0), Err(both));
        assert_eq!(check(&worktree.0, &worktree.0), Ok(()));
        let err = check(&folder.0, &worktree.0).unwrap_err();
        assert!(err.starts_with(&format!("{} has both", folder.0.display())), "{err}");
    }

    #[test]
    fn a_state_root_with_the_state_of_the_other_layout_is_refused() {
        let new = Folder::new();
        new.write("tminus/project.toml", "");
        new.write(".tminus/log.jsonl", "");
        assert_eq!(check(&new.0, &new.0), Ok(()));
        new.write(".gdd/log.jsonl", "");
        let refused = format!(
            "{} keeps its files in tminus/ but has .gdd/, the state of the old layout: move .gdd \
             to .tminus or remove it",
            new.0.display()
        );
        assert_eq!(check(&new.0, &new.0), Err(refused));
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        old.write(".gdd/log.jsonl", "");
        assert_eq!(check(&old.0, &old.0), Ok(()));
        // Whatever it is: a file, an empty folder or a link.
        old.write(".tminus", "");
        let refused = format!(
            "{} keeps its files in gdd/ but has .tminus/, the state of the new layout: move \
             .tminus to .gdd or remove it",
            old.0.display()
        );
        assert_eq!(check(&old.0, &old.0), Err(refused));
        fs::remove_file(old.0.join(".tminus")).unwrap();
        std::os::unix::fs::symlink("nowhere", old.0.join(".tminus")).unwrap();
        assert!(check(&old.0, &old.0).is_err());
    }

    #[test]
    fn the_state_follows_the_layout_of_the_state_root() {
        // A branch that moved the files to tminus/, while main keeps gdd/ and .gdd/.
        let main = Folder::new();
        main.write("gdd/project.toml", "");
        main.write(".gdd/log.jsonl", "");
        let branch = Folder::new();
        branch.write("tminus/project.toml", "");
        assert_eq!(check(&branch.0, &main.0), Ok(()));
        main.write(".tminus/log.jsonl", "");
        assert!(check(&branch.0, &main.0).is_err());
        let plain = Folder::new();
        plain.write(".gdd/x", "");
        assert!(check(&plain.0, &plain.0).is_err());
    }

    #[test]
    fn a_state_folder_that_cannot_be_looked_at_is_refused() {
        let folder = Folder::new();
        // A file where the state root should be a folder: a file on the way counts as nothing
        // there.
        folder.write("root", "");
        let root = folder.0.join("root");
        assert_eq!(check(&root, &root), Ok(()));
        // A name too long for the file system is another error.
        let long = folder.0.join("x".repeat(300));
        let err = check(&long, &long).unwrap_err();
        assert!(err.starts_with("cannot read "), "{err}");
    }

    #[test]
    fn paths_are_joined_and_named_as_the_project_names_them() {
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        assert_eq!(named(&old.0, ".tminus/changes"), ".gdd/changes");
        assert_eq!(join(&old.0, "tminus/zones.toml"), old.0.join("gdd/zones.toml"));
        let new = Folder::new();
        assert_eq!(named(&new.0, ".tminus/changes"), ".tminus/changes");
        assert_eq!(join(&new.0, "tminus/zones.toml"), new.0.join("tminus/zones.toml"));
        assert_eq!(Layout::Gdd.path(".github/workflows/tminus.yml"), ".github/workflows/gdd.yml");
        assert_eq!(Layout::Tminus.path("tminus/rules.toml"), "tminus/rules.toml");
    }

    #[test]
    fn the_old_names_are_the_new_ones_where_the_word_stands_alone() {
        let gdd = Layout::Gdd;
        let pairs = [
            ("tminus", "gdd"),
            ("tminus g2 c1", "gdd g2 c1"),
            ("run `tminus ci`", "run `gdd ci`"),
            ("/tminus-build $tminus-review", "/gdd-build $gdd-review"),
            ("tminus(c1): done", "gdd(c1): done"),
            (".tminus/changes/c1", ".gdd/changes/c1"),
            ("tminus/zones.toml", "gdd/zones.toml"),
            ("<!-- tminus: abc -->", "<!-- gdd: abc -->"),
            ("TMINUS_CHANGE", "GDD_CHANGE"),
            ("$TMINUS_ROLE and ${TMINUS_STATE}", "$GDD_ROLE and ${GDD_STATE}"),
            ("tminus·tminus", "gdd·gdd"),
            ("ñtminusñ", "ñgddñ"),
            ("GDD · Gate-Driven Development", "GDD · Gate-Driven Development"),
            ("", ""),
        ];
        for (new, old) in pairs {
            assert_eq!(gdd.localize(new), old, "{new:?}");
            assert_eq!(Layout::Tminus.localize(new), new, "{new:?}");
        }
    }

    #[test]
    fn what_only_looks_like_the_new_name_stays() {
        for kept in [
            "tminuses",
            "xtminus",
            "tminus_x",
            "_tminus",
            "tminus1",
            "9tminus",
            "MY_TMINUS_X",
            "XTMINUS_A",
            "TMINUS_x",
            "TMINUS_",
            "TMINUS_1",
            "Tminus",
            "TMINUS",
            "T-minus",
            "tminustminus",
            "tminu",
            "gdd",
        ] {
            assert_eq!(Layout::Gdd.localize(kept), kept, "{kept:?}");
        }
        assert_eq!(Layout::Gdd.localize("a TMINUS_X"), "a GDD_X");
        assert_eq!(Layout::Gdd.localize("-TMINUS_X-tminus-"), "-GDD_X-gdd-");
    }
}
