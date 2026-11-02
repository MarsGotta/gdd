//! The run state of a change, `.tminus/changes/<id>/run.json` (`.gdd/` in a state root of the old
//! layout): where its recipe stopped to wait for the human. It only stops at G4, for `tminus
//! resume` to answer.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::change;
use crate::gates::verdict;

/// The gate the recipe waits at.
const G4: &str = "G4";

/// What the run state keeps.
#[derive(Serialize, Deserialize)]
struct State {
    /// The gate the recipe waits at.
    paused_at: String,
}

/// The run state of a change, in its folder.
const FILE: &str = "run.json";

/// Where the run state of change `id` lives, from the state root (see [`crate::state::root`]), in
/// the new names (see [`crate::layout`]).
pub fn path(id: &str) -> String {
    format!(".tminus/changes/{id}/{FILE}")
}

/// Where the run state of change `id` lives under the state root `root`, as that root names it
/// (see [`verdict::change_file`]).
fn named(root: &Path, id: &str) -> String {
    verdict::change_file(root, id, FILE)
}

/// Records that the recipe of change `id`, of the project whose state root is `root`, waits for
/// the human at G4.
pub fn pause_at_g4(root: &Path, id: &str) -> Result<(), String> {
    let state = State {
        paused_at: G4.to_string(),
    };
    let text = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    change::write(root, &named(root, id), &format!("{text}\n"))
}

/// Whether the recipe of change `id`, of the project whose state root is `root`, waits at G4.
/// Without a run state, it does not.
pub fn paused_at_g4(root: &Path, id: &str) -> Result<bool, String> {
    let path = named(root, id);
    let text = match fs::read_to_string(root.join(&path)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("cannot read {path}: {e}")),
    };
    let state: State = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(state.paused_at == G4)
}

/// Clears the run state of change `id`, of the project whose state root is `root`: its recipe no
/// longer waits.
pub fn clear(root: &Path, id: &str) -> Result<(), String> {
    let path = named(root, id);
    match fs::remove_file(root.join(&path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    #[test]
    fn a_pause_at_g4_is_kept_until_it_is_cleared() {
        let folder = Folder::new();
        assert_eq!(path("c1"), ".tminus/changes/c1/run.json");
        assert_eq!(paused_at_g4(&folder.0, "c1"), Ok(false));
        assert_eq!(clear(&folder.0, "c1"), Ok(()));
        pause_at_g4(&folder.0, "c1").unwrap();
        let saved = fs::read_to_string(folder.0.join(".tminus/changes/c1/run.json")).unwrap();
        assert_eq!(saved, "{\n  \"paused_at\": \"G4\"\n}\n");
        assert_eq!(paused_at_g4(&folder.0, "c1"), Ok(true));
        assert_eq!(paused_at_g4(&folder.0, "c2"), Ok(false));
        clear(&folder.0, "c1").unwrap();
        assert_eq!(paused_at_g4(&folder.0, "c1"), Ok(false));
        folder.write(".tminus/changes/c1/run.json", r#"{"paused_at": "G3"}"#);
        assert_eq!(paused_at_g4(&folder.0, "c1"), Ok(false));
    }

    #[test]
    fn a_state_root_of_the_old_layout_keeps_the_run_state_in_its_gdd_folder() {
        let folder = Folder::new();
        folder.write("gdd/project.toml", "");
        pause_at_g4(&folder.0, "c1").unwrap();
        assert!(folder.0.join(".gdd/changes/c1/run.json").is_file());
        assert_eq!(paused_at_g4(&folder.0, "c1"), Ok(true));
        clear(&folder.0, "c1").unwrap();
        assert!(!folder.0.join(".gdd/changes/c1/run.json").exists());
        assert!(!folder.0.join(".tminus").exists());
        folder.write(".gdd/changes/c2/run.json", "{not json");
        let err = paused_at_g4(&folder.0, "c2").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c2/run.json: "), "{err}");
        // An id that holds the new name keeps it.
        pause_at_g4(&folder.0, "rename-tminus").unwrap();
        let run = folder.0.join(".gdd/changes/rename-tminus/run.json");
        assert!(run.is_file());
        assert_eq!(paused_at_g4(&folder.0, "rename-tminus"), Ok(true));
    }

    #[test]
    fn a_run_state_that_cannot_be_read_or_removed_is_an_error() {
        let folder = Folder::new();
        let path = ".tminus/changes/c1/run.json";
        folder.write(path, "{not json");
        let err = paused_at_g4(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with(&format!("{path}: ")), "{err}");
        fs::remove_file(folder.0.join(path)).unwrap();
        fs::create_dir_all(folder.0.join(path).join("x")).unwrap();
        let err = paused_at_g4(&folder.0, "c1").unwrap_err();
        let start = format!("cannot read {path}: ");
        assert!(err.starts_with(&start), "{err}");
        let err = clear(&folder.0, "c1").unwrap_err();
        let start = format!("cannot remove {path}: ");
        assert!(err.starts_with(&start), "{err}");
        let err = pause_at_g4(&folder.0, "c1").unwrap_err();
        let start = format!("cannot write {path}: ");
        assert!(err.starts_with(&start), "{err}");
    }
}
