//! What the agents of the recipe answer in their files: the Reviewer's `VERDICT: OK|KO` in
//! `.gdd/changes/<id>/review.md`, and where the replan sends the change, `NEXT: test|build` in
//! `.gdd/changes/<id>/replan.md`. The last answer of a file counts; a file without a readable one
//! says KO, and build mode.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::{change, log};

/// Why a Reviewer without a readable verdict counts as KO.
const NO_VERDICT: &str = "The Reviewer left no readable VERDICT: counts as KO.";

/// Where the Reviewer of change `id` leaves its verdict, from the state root (see
/// [`crate::state::root`]).
pub fn review_path(id: &str) -> String {
    format!(".gdd/changes/{id}/review.md")
}

/// Where the replan of change `id` says where the change goes next, from the state root.
pub fn replan_path(id: &str) -> String {
    format!(".gdd/changes/{id}/replan.md")
}

/// The verdict of a Reviewer's text: whether the last `VERDICT:` followed by `OK` or `KO` says
/// OK, in any case and with Markdown bold allowed before the word. `None` without one.
pub fn verdict(text: &str) -> Option<bool> {
    let upper = text.to_ascii_uppercase();
    for (at, mark) in upper.rmatch_indices("VERDICT:") {
        let word = answer(&upper[at + mark.len()..]);
        if word.starts_with("OK") || word.starts_with("KO") {
            return Some(word.starts_with("OK"));
        }
    }
    None
}

/// Whether a replan's text sends the change back to test mode: its last line that starts with
/// `NEXT:` followed by `test` or `build` says `test`, in any case and with Markdown bold allowed.
pub fn sends_to_test(text: &str) -> bool {
    for line in text.lines().rev() {
        let line = line.trim_start().to_ascii_lowercase();
        let Some(rest) = line.strip_prefix("next:") else {
            continue;
        };
        let word = answer(rest);
        if word.starts_with("test") || word.starts_with("build") {
            return word.starts_with("test");
        }
    }
    false
}

/// The answer after a mark: without the blanks and Markdown bold before it.
fn answer(rest: &str) -> &str {
    rest.trim_start().trim_start_matches('*').trim_start()
}

/// The verdict of the Reviewer of change `id`, of the project whose state root is `state` (see
/// [`verdict`]): whether it is OK. No file, or no readable verdict, is KO. Logged as a machine
/// step of G3.
pub fn review(state: &Path, id: &str) -> Result<bool, String> {
    let said = verdict(&read(state, &review_path(id))?);
    let ok = said == Some(true);
    let entry = log::Entry {
        change: id.to_string(),
        phase: "3-integrate".to_string(),
        gate: "G3".to_string(),
        role: "machine".to_string(),
        agent: "verdict".to_string(),
        result: if ok { "ok" } else { "ko" }.to_string(),
        duration_s: 0.0,
        note: if said.is_none() { NO_VERDICT } else { "" }.to_string(),
    };
    change::logged(state, &entry)?;
    Ok(ok)
}

/// Whether the replan of change `id`, of the project whose state root is `state`, sends it back to
/// test mode (see [`sends_to_test`]). No file says build mode.
pub fn replanned_to_test(state: &Path, id: &str) -> Result<bool, String> {
    Ok(sends_to_test(&read(state, &replan_path(id))?))
}

/// The text of the file `path` under the state root `state`: empty when there is none. Bytes that
/// are not UTF-8 are replaced.
fn read(state: &Path, path: &str) -> Result<String, String> {
    match fs::read(state.join(path)) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;

    #[test]
    fn the_answers_live_next_to_the_record() {
        assert_eq!(review_path("c1"), ".gdd/changes/c1/review.md");
        assert_eq!(replan_path("c1"), ".gdd/changes/c1/replan.md");
    }

    #[test]
    fn the_last_readable_verdict_counts() {
        for (text, said) in [
            ("VERDICT: OK", Some(true)),
            ("VERDICT: KO", Some(false)),
            ("verdict:ok", Some(true)),
            ("**VERDICT:** OK", Some(true)),
            ("VERDICT: ** ** OK", None),
            ("VERDICT: **ko** because", Some(false)),
            ("# Review\nVERDICT: KO\n...\nVERDICT: OK\n", Some(true)),
            ("VERDICT: OK\nlater: VERDICT: KO", Some(false)),
            ("VERDICT: OK then VERDICT: maybe", Some(true)),
            ("VERDICT: maybe", None),
            ("VERDICT OK", None),
            ("OK", None),
            ("", None),
        ] {
            assert_eq!(verdict(text), said, "{text:?}");
        }
    }

    #[test]
    fn the_last_next_line_says_where_the_change_goes() {
        for (text, test) in [
            ("NEXT: test", true),
            ("  next:test\n", true),
            ("NEXT: **Test** mode", true),
            ("**NEXT:** test", false),
            ("NEXT: build", false),
            ("NEXT: test\nNEXT: build", false),
            ("NEXT: build\nthen NEXT: test", false),
            ("NEXT: build\nNEXT: maybe\nNEXT: TEST", true),
            ("NEXT: test\nNEXT: maybe", true),
            ("NEXT: maybe", false),
            ("", false),
        ] {
            assert_eq!(sends_to_test(text), test, "{text:?}");
        }
    }

    fn log_lines(folder: &Folder) -> Vec<log::Record> {
        let text = fs::read_to_string(folder.0.join(log::PATH)).unwrap();
        log::parse(&text).unwrap()
    }

    #[test]
    fn the_review_is_read_and_logged() {
        let folder = Folder::new();
        assert_eq!(review(&folder.0, "c1"), Ok(false));
        folder.write(".gdd/changes/c1/review.md", "Fine.\nVERDICT: OK\n");
        assert_eq!(review(&folder.0, "c1"), Ok(true));
        let lines = log_lines(&folder);
        let [missing, found] = lines.as_slice() else {
            panic!("two log lines: {lines:?}");
        };
        for (field, value) in [
            ("change", "c1"),
            ("phase", "3-integrate"),
            ("gate", "G3"),
            ("role", "machine"),
            ("agent", "verdict"),
            ("result", "ok"),
            ("note", ""),
        ] {
            assert_eq!(found.text(field), value, "{field}");
        }
        assert_eq!(missing.text("result"), "ko");
        assert_eq!(missing.text("note"), NO_VERDICT);
        folder.write(".gdd/changes/c1/review.md", "VERDICT: KO");
        assert_eq!(review(&folder.0, "c1"), Ok(false));
        assert_eq!(log_lines(&folder)[2].text("note"), "");
    }

    #[test]
    fn the_replan_is_read_from_its_file() {
        let folder = Folder::new();
        assert_eq!(replanned_to_test(&folder.0, "c1"), Ok(false));
        folder.write(".gdd/changes/c1/replan.md", "Plan.\nNEXT: test\n");
        assert_eq!(replanned_to_test(&folder.0, "c1"), Ok(true));
        assert!(!folder.0.join(log::PATH).exists());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error() {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/review.md")).unwrap();
        let err = review(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/review.md: ";
        assert!(err.starts_with(start), "{err}");
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/replan.md")).unwrap();
        let err = replanned_to_test(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/replan.md: ";
        assert!(err.starts_with(start), "{err}");
    }
}
