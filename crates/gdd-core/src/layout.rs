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

use std::fs::{self, Metadata};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::git::{self, Git};

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
        Layout::of(old, new)
    }

    /// The layout of the project as the commit `commit` of `git` has it, by the same rule as
    /// [`Layout::at`], from the files git keeps there: what to read a file of that commit by,
    /// which may not be the layout of the checkout (a branch that moved the files, read from
    /// main). An error when git cannot list them.
    pub fn at_commit(git: &Git, commit: &str) -> Result<Layout, git::Error> {
        let old = git.file_at(commit, OLD_PROJECT)?.is_some();
        let new = git.file_at(commit, NEW_PROJECT)?.is_some();
        Ok(Layout::of(old, new))
    }

    /// The layout of a project that has, or not, `gdd/project.toml` (`old`) and
    /// `tminus/project.toml` (`new`): the old one only with the first alone.
    fn of(old: bool, new: bool) -> Layout {
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
    /// project of this layout names it (see [`Layout::localize`]). Under the folder of the files
    /// (`tminus/`) or of the state (`.tminus/`), only that folder takes the old name: what comes
    /// after it, such as the id of a change (`.tminus/changes/rename-tminus/g2.json`), is data
    /// and stays as it is; a path already under `gdd/` or `.gdd/` stays whole, so naming a path
    /// twice names it once. Any other path (`.github/workflows/tminus.yml`) is localized whole.
    pub fn path(self, path: &str) -> String {
        let (first, rest) = match path.split_once('/') {
            Some((first, rest)) => (first, Some(rest)),
            None => (path, None),
        };
        match (first, rest) {
            ("tminus" | ".tminus", Some(rest)) => format!("{}/{rest}", self.localize(first)),
            ("gdd" | ".gdd", _) => path.to_string(),
            _ => self.localize(path),
        }
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
    match seen(fs::symlink_metadata(&path)) {
        Ok(Some(_)) => Err(format!(
            "{} keeps its files in {files} but has {other}/, the state of the {kind} layout: move \
             {other} to {own} or remove it",
            state_root.display()
        )),
        Ok(None) => Ok(()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// The root of the project the folder `folder` is in, with its project file from that root: the
/// nearest folder, from `folder` upwards, with `tminus/project.toml` or, set up before the rename,
/// `gdd/project.toml`, looked at in that order. `None` without one. A project file that cannot be
/// looked at on the way is an error that names it and why (see [`looked`]): that folder may be
/// the project, and is never taken for nothing so that a folder above it is.
pub fn project_above(folder: &Path) -> Result<Option<(PathBuf, &'static str)>, String> {
    for above in folder.ancestors() {
        for file in [NEW_PROJECT, OLD_PROJECT] {
            if looked(&above.join(file))? {
                return Ok(Some((above.to_path_buf(), file)));
            }
        }
    }
    Ok(None)
}

/// `Ok` unless `folder` has both `tminus/project.toml` and `gdd/project.toml`, or one of them
/// cannot be looked at.
fn one_project(folder: &Path) -> Result<(), String> {
    let mut found = Vec::new();
    for file in [NEW_PROJECT, OLD_PROJECT] {
        if looked(&folder.join(file))? {
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
/// folder on the way is a file (see [`seen`]). Any other error is an error.
fn is_file(path: &Path) -> io::Result<bool> {
    Ok(seen(fs::metadata(path))?.is_some_and(|meta| meta.is_file()))
}

/// Whether `path` is a file (see [`is_file`]); when it cannot be looked at, why, with its path:
/// `cannot read <path>: <why>`.
fn looked(path: &Path) -> Result<bool, String> {
    is_file(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// What a look at a path (`fs::metadata`, or `fs::symlink_metadata` not to follow a link) found
/// there: `None` when nothing is there, the path does not exist or a folder on the way is a file.
/// Any other error is an error.
fn seen(looked: io::Result<Metadata>) -> io::Result<Option<Metadata>> {
    match looked {
        Ok(meta) => Ok(Some(meta)),
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Whether `c` is an ASCII letter, an ASCII digit or `_`: what a name touching it would belong
/// to.
fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `text` in the old names (see [`Layout::localize`]), name by name: a name is a run of
/// [`word`] characters as long as it goes, so a name that touches none on either side is a
/// whole run.
fn to_old_names(text: &str) -> String {
    text.split_inclusive(|c: char| !word(c))
        .map(|piece| {
            let name = piece.trim_end_matches(|c: char| !word(c));
            format!("{}{}", old_name(name), &piece[name.len()..])
        })
        .collect()
}

/// The whole name `name` in the old names: `tminus` is `gdd`, and `TMINUS_` before a capital
/// letter is `GDD_`. Any other name stays.
fn old_name(name: &str) -> String {
    if name == "tminus" {
        return "gdd".to_string();
    }
    match name.strip_prefix("TMINUS_") {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_uppercase()) => format!("GDD_{rest}"),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};

    #[test]
    fn the_layout_of_a_commit_is_the_one_its_files_say_not_the_checkout() {
        let repo = Repo::new();
        let git = repo.tool();
        let first = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(Layout::at_commit(&git, &first).unwrap(), Layout::Tminus);
        repo.write("gdd/project.toml", "name = \"x\"\n");
        repo.commit();
        assert_eq!(Layout::at_commit(&git, "HEAD").unwrap(), Layout::Gdd);
        // The checkout moved its files to tminus/, uncommitted: the commit still keeps gdd/.
        repo.git(&["mv", "gdd", "tminus"]);
        assert_eq!(Layout::at(repo.root()), Layout::Tminus);
        assert_eq!(Layout::at_commit(&git, "HEAD").unwrap(), Layout::Gdd);
        repo.commit();
        assert_eq!(Layout::at_commit(&git, "HEAD").unwrap(), Layout::Tminus);
        assert_eq!(Layout::at_commit(&git, "HEAD~1").unwrap(), Layout::Gdd);
        // Both kept: the new one.
        repo.write("gdd/project.toml", "name = \"x\"\n");
        repo.commit();
        assert_eq!(Layout::at_commit(&git, "HEAD").unwrap(), Layout::Tminus);
        assert!(Layout::at_commit(&git, "nowhere").is_err());
    }

    #[test]
    fn the_project_is_the_nearest_folder_upwards_with_a_project_file() {
        let folder = Folder::new();
        let deep = folder.0.join("a/b");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(project_above(&deep), Ok(None));
        folder.write("gdd/project.toml", "");
        let old = Some((folder.0.clone(), OLD_PROJECT));
        assert_eq!(project_above(&deep), Ok(old));
        folder.write("a/tminus/project.toml", "");
        let new = Some((folder.0.join("a"), NEW_PROJECT));
        assert_eq!(project_above(&deep), Ok(new.clone()));
        // Both in one folder: the new one is looked at first.
        folder.write("a/gdd/project.toml", "");
        assert_eq!(project_above(&deep), Ok(new));
        // A folder named project.toml is no project file.
        let empty = Folder::new();
        fs::create_dir_all(empty.0.join("tminus/project.toml")).unwrap();
        fs::create_dir_all(empty.0.join("gdd/project.toml")).unwrap();
        assert_eq!(project_above(&empty.0), Ok(None));
    }

    #[test]
    fn a_project_file_that_cannot_be_looked_at_on_the_way_up_is_refused() {
        let folder = Folder::new();
        folder.write("tminus/project.toml", "");
        let below = folder.0.join("below");
        fs::create_dir_all(&below).unwrap();
        assert_eq!(
            project_above(&below),
            Ok(Some((folder.0.clone(), NEW_PROJECT)))
        );
        // A link to itself where a project file of `below` would be: looking under it fails, and
        // the project above is not taken in its place.
        for name in ["tminus", "gdd"] {
            let link = below.join(name);
            std::os::unix::fs::symlink(name, &link).unwrap();
            let path = link.join("project.toml");
            let err = project_above(&below).unwrap_err();
            let start = format!("cannot read {}: ", path.display());
            assert!(err.starts_with(&start), "{err}");
            fs::remove_file(&link).unwrap();
        }
    }

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
        assert!(
            err.starts_with(&format!("cannot read {}: ", path.display())),
            "{err}"
        );
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
        assert_eq!(Layout::Tminus.localize("TMINUS_CHANGE"), "TMINUS_CHANGE");
        assert_eq!(Layout::Gdd.localize("TMINUS_CHANGE"), "GDD_CHANGE");
        assert_eq!(twin("TMINUS_CHANGE").as_deref(), Some("GDD_CHANGE"));
        assert_eq!(twin("GDD_DIFF_FILES").as_deref(), Some("TMINUS_DIFF_FILES"));
        for none in [
            "TMINUS_",
            "GDD_",
            "PATH",
            "MY_GDD_X",
            "gdd_x",
            "XTMINUS_A",
            "",
        ] {
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
        assert!(
            err.starts_with(&format!("{} has both", folder.0.display())),
            "{err}"
        );
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
        assert_eq!(
            join(&old.0, "tminus/zones.toml"),
            old.0.join("gdd/zones.toml")
        );
        let new = Folder::new();
        assert_eq!(named(&new.0, ".tminus/changes"), ".tminus/changes");
        assert_eq!(
            join(&new.0, "tminus/zones.toml"),
            new.0.join("tminus/zones.toml")
        );
        assert_eq!(
            Layout::Gdd.path(".github/workflows/tminus.yml"),
            ".github/workflows/gdd.yml"
        );
        assert_eq!(
            Layout::Tminus.path("tminus/rules.toml"),
            "tminus/rules.toml"
        );
    }

    #[test]
    fn under_the_folders_of_the_layout_only_the_folder_takes_the_old_name() {
        let gdd = Layout::Gdd;
        for (new, old) in [
            (
                ".tminus/changes/rename-tminus/g2.json",
                ".gdd/changes/rename-tminus/g2.json",
            ),
            ("tminus/tminus.toml", "gdd/tminus.toml"),
            (".tminus/changes/", ".gdd/changes/"),
            (".tminus/", ".gdd/"),
            (".tminus", ".gdd"),
            ("tminus", "gdd"),
            (".github/workflows/tminus.yml", ".github/workflows/gdd.yml"),
            ("tminus-x/tminus", "gdd-x/gdd"),
            ("", ""),
        ] {
            assert_eq!(gdd.path(new), old, "{new:?}");
            assert_eq!(Layout::Tminus.path(new), new, "{new:?}");
            // Named twice, a path is named once.
            assert_eq!(gdd.path(old), old, "{old:?}");
        }
        for named in [
            ".gdd/changes/rename-tminus",
            "gdd/tminus.toml",
            "gdd",
            ".gdd",
        ] {
            assert_eq!(gdd.path(named), named);
        }
        let old = Folder::new();
        old.write("gdd/project.toml", "");
        let g2 = ".tminus/changes/rename-tminus/g2.json";
        assert_eq!(named(&old.0, g2), ".gdd/changes/rename-tminus/g2.json");
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
            (
                "$TMINUS_ROLE and ${TMINUS_STATE}",
                "$GDD_ROLE and ${GDD_STATE}",
            ),
            ("tminus·tminus", "gdd·gdd"),
            ("ñtminusñ", "ñgddñ"),
            (
                "GDD · Gate-Driven Development",
                "GDD · Gate-Driven Development",
            ),
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
