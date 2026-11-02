//! What the agents of the recipe answer in their files: the Reviewer's `VERDICT: OK|KO` in
//! `.tminus/changes/<id>/review.md`, with the files its findings name, and where the replan sends
//! the change, `NEXT: test|build` in `.tminus/changes/<id>/replan.md` (`.gdd/` in a project of the
//! old layout, see [`crate::layout`]). The last answer of a file counts; a file without a
//! readable one says KO, and build mode. Any agent of the recipe may also end its text saying it
//! cannot do the task, `CANNOT: <why>` (see [`cannot`]).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::{change, layout, log};

/// Why a Reviewer without a readable verdict counts as KO.
const NO_VERDICT: &str = "The Reviewer left no readable VERDICT: counts as KO.";

/// The mark of an agent that says it cannot do the task.
const CANNOT: &str = "CANNOT:";

/// The reason of an agent that says it cannot do the task without giving one.
const NO_REASON: &str = "it gave no reason";

/// The folder of the changes, from the state root, in the new names.
const CHANGES: &str = ".tminus/changes";

/// The file `file` of change `id`, from the state root `state`, as the project there names its
/// state folder (see [`crate::layout`]). Only the folder is named anew: the id stays as it is,
/// even one that holds the new name, as `rename-tminus` does.
pub(crate) fn change_file(state: &Path, id: &str, file: &str) -> String {
    format!("{}/{id}/{file}", layout::named(state, CHANGES))
}

/// Where the Reviewer of change `id` leaves its verdict, from the state root (see
/// [`crate::state::root`]), in the new names (see [`crate::layout`]).
pub fn review_path(id: &str) -> String {
    format!(".tminus/changes/{id}/review.md")
}

/// Where the replan of change `id` says where the change goes next, from the state root, in the
/// new names.
pub fn replan_path(id: &str) -> String {
    format!(".tminus/changes/{id}/replan.md")
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

/// The reason of an agent's text that says it cannot do the task: what follows its last line that
/// starts with `CANNOT:`, in any case and with blanks and Markdown bold allowed before the mark,
/// trimmed and without the bold after it. A mark without a reason says [`NO_REASON`]. `None`
/// without one.
pub fn cannot(text: &str) -> Option<String> {
    text.lines().rev().find_map(|line| {
        let line = answer(line);
        let mark = line.get(..CANNOT.len())?;
        mark.eq_ignore_ascii_case(CANNOT).then(|| {
            let reason = answer(&line[CANNOT.len()..]).trim_end();
            let reason = reason.trim_end_matches('*').trim_end();
            if reason.is_empty() { NO_REASON } else { reason }.to_string()
        })
    })
}

/// The answer after a mark: without the blanks and Markdown bold before it.
fn answer(rest: &str) -> &str {
    rest.trim_start().trim_start_matches('*').trim_start()
}

/// The files the findings of a Reviewer's text name, in order: of each line shaped
/// `F<n> · <class> · <file>:<line> · <what fails>`, the file, without its `:<line>`, which may be
/// missing. A leading `-` or `*`, and Markdown bold or code marks around a field, are allowed. A
/// file written as a Markdown link, `[<file>:<line>](<target>)`, is read as the link's text (see
/// [`unlinked`]); a link without text names no file.
pub fn finding_files(text: &str) -> Vec<String> {
    text.lines().filter_map(finding_file).collect()
}

/// The file the finding on `line` names: `None` when the line is not a finding.
fn finding_file(line: &str) -> Option<String> {
    let line = line.trim_start();
    let line = line.strip_prefix(['-', '*']).unwrap_or(line);
    let mut fields = line.split('·');
    let number = bare(fields.next()?).strip_prefix('F').unwrap_or_default();
    let numbered = !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
    let (_class, field) = (fields.next()?, unlinked(fields.next()?));
    let file = bare(field.split(':').next().unwrap_or_default());
    let named = !file.is_empty() && !file.contains("](");
    (numbered && named).then(|| file.to_string())
}

/// A field of a finding without the blanks, Markdown bold and code marks around it.
fn bare(field: &str) -> &str {
    field.trim().trim_matches(['*', '`']).trim()
}

/// `text` with each Markdown link, `[label](target)` with a label, replaced by its label.
pub fn unlinked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find(']').filter(|&close| close > 0);
        let link = close.and_then(|close| {
            let target = after[close + 1..].strip_prefix('(')?;
            let end = target.find(')')?;
            Some((&after[..close], &target[end + 1..]))
        });
        match link {
            Some((label, tail)) => {
                out.push_str(label);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The verdict of the Reviewer of change `id`, of the project whose state root is `state` (see
/// [`verdict`]): whether it is OK. No file, or no readable verdict, is KO. Logged as a machine
/// step of G3.
pub fn review(state: &Path, id: &str) -> Result<bool, String> {
    let said = verdict(&read(state, &change_file(state, id, "review.md"))?);
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

/// The files the findings of the Reviewer of change `id` name (see [`finding_files`]), of the
/// project whose state root is `state`: none without a review.
pub fn review_files(state: &Path, id: &str) -> Result<Vec<String>, String> {
    let review = change_file(state, id, "review.md");
    Ok(finding_files(&read(state, &review)?))
}

/// Whether the replan of change `id`, of the project whose state root is `state`, sends it back to
/// test mode (see [`sends_to_test`]). No file says build mode.
pub fn replanned_to_test(state: &Path, id: &str) -> Result<bool, String> {
    let replan = change_file(state, id, "replan.md");
    Ok(sends_to_test(&read(state, &replan)?))
}

/// The text of the file `path`, as the project names it (see [`change_file`]), under the state
/// root `state`: empty when there is none. Bytes that are not UTF-8 are replaced.
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
        assert_eq!(review_path("c1"), ".tminus/changes/c1/review.md");
        assert_eq!(replan_path("c1"), ".tminus/changes/c1/replan.md");
    }

    #[test]
    fn a_project_set_up_before_the_rename_keeps_its_answers_in_gdd() {
        let folder = Folder::new();
        folder.write("gdd/project.toml", "");
        folder.write(
            ".gdd/changes/c1/review.md",
            "F1 · a · x.rs:1 · y\nVERDICT: OK\n",
        );
        folder.write(".gdd/changes/c1/replan.md", "NEXT: test\n");
        assert_eq!(review_files(&folder.0, "c1"), Ok(vec!["x.rs".into()]));
        assert_eq!(replanned_to_test(&folder.0, "c1"), Ok(true));
        assert_eq!(review(&folder.0, "c1"), Ok(true));
        fs::remove_file(folder.0.join(".gdd/changes/c1/review.md")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/review.md")).unwrap();
        let err = review_files(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/review.md: ";
        assert!(err.starts_with(start), "{err}");
        assert!(!folder.0.join(".tminus").exists());
    }

    #[test]
    fn only_the_folder_of_a_change_is_named_as_the_project_names_it() {
        let (new, old) = (Folder::new(), Folder::new());
        old.write("gdd/project.toml", "");
        let id = "rename-tminus";
        let in_new = change_file(&new.0, id, "review.md");
        assert_eq!(in_new, ".tminus/changes/rename-tminus/review.md");
        let in_old = change_file(&old.0, id, "g2.json");
        assert_eq!(in_old, ".gdd/changes/rename-tminus/g2.json");
        old.write(".gdd/changes/rename-tminus/replan.md", "NEXT: test\n");
        assert_eq!(replanned_to_test(&old.0, id), Ok(true));
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

    #[test]
    fn the_last_cannot_line_says_why_the_agent_cannot_do_the_task() {
        let said = |reason: &str| Some(reason.to_string());
        for (text, reason) in [
            ("CANNOT: no currency", said("no currency")),
            ("cannot:no currency", said("no currency")),
            ("Cannot: No currency.", said("No currency.")),
            ("**CANNOT:** no currency", said("no currency")),
            ("**CANNOT: no currency**", said("no currency")),
            ("  \t CANNOT:   no currency  ", said("no currency")),
            ("# Summary\nCANNOT: a\nDone.\n", said("a")),
            ("CANNOT: a\nCANNOT: b\n", said("b")),
            ("CANNOT: a\nCANNOT b\n", said("a")),
            ("CANNOT:", said(NO_REASON)),
            ("CANNOT:  ** ** ", said(NO_REASON)),
            ("I say CANNOT: x", None),
            ("`CANNOT: x`", None),
            ("CANNOT x", None),
            ("CANNO", None),
            ("ññññ: x", None),
            ("VERDICT: OK", None),
            ("", None),
        ] {
            assert_eq!(cannot(text), reason, "{text:?}");
        }
        assert_eq!(NO_REASON, "it gave no reason");
    }

    #[test]
    fn a_finding_names_its_file_with_or_without_its_line() {
        for line in [
            "F1 · off by one · src/a.rs:3 · drops a line",
            "F12 · off by one · src/a.rs · drops a line",
            "- F2 · class · src/a.rs:4 · what",
            "* F2 · class · src/a.rs:4 · what",
            "  - **F3** · class · **src/a.rs:5** · what",
            "**F4 · class · `src/a.rs:6` · what**",
            "F5 · class · **`src/a.rs`:7**",
            "F6 · class · src/a.rs:8:2 · what",
            "- **F7 · class · [src/a.rs:9](/abs/wt/src/a.rs:9)** · what",
            "F8 · class · [`src/a.rs`:10](/abs/wt/src/a.rs) · what",
            "F9 · class · [src/a.rs](src/a.rs) · what",
        ] {
            assert_eq!(finding_files(line), ["src/a.rs"], "{line:?}");
        }
        for line in [
            "F1 · class · [](x) · what",
            "F · class · src/a.rs:1 · what",
            "Fix · class · src/a.rs:1 · what",
            "F1x · class · src/a.rs:1 · what",
            "1 · class · src/a.rs:1 · what",
            "F1 · class ·  · what",
            "F1 · class · :3 · what",
            "F1 · src/a.rs:3",
            "-- F1 · class · src/a.rs:1 · what",
            "VERDICT: KO",
            "",
        ] {
            assert!(finding_files(line).is_empty(), "{line:?}");
        }
        let text = "# Review\nF1 · a · x.rs:1 · y\n\nF2 · b · z.rs · w\nVERDICT: KO\n";
        assert_eq!(finding_files(text), ["x.rs", "z.rs"]);
    }

    #[test]
    fn a_link_is_read_as_its_label() {
        for (text, plain) in [
            ("[a](b)", "a"),
            ("x [the guard](a.rs) y [b]()", "x the guard y b"),
            ("[a [b](c)", "a [b"),
            ("[a] [b](c)", "[a] b"),
            ("[](x) [a](b", "[](x) [a](b"),
            ("[a]x(b)", "[a]x(b)"),
            ("no links", "no links"),
        ] {
            assert_eq!(unlinked(text), plain, "{text}");
        }
    }

    #[test]
    fn the_files_of_the_findings_are_read_from_the_review() {
        let folder = Folder::new();
        assert_eq!(review_files(&folder.0, "c1"), Ok(vec![]));
        let review = "F1 · weak step · tests/steps/a.rs:3 · checks nothing\nVERDICT: KO\n";
        folder.write(".tminus/changes/c1/review.md", review);
        let files = review_files(&folder.0, "c1");
        assert_eq!(files, Ok(vec!["tests/steps/a.rs".into()]));
        assert!(!folder.0.join(log::PATH).exists());
        fs::remove_file(folder.0.join(".tminus/changes/c1/review.md")).unwrap();
        fs::create_dir_all(folder.0.join(".tminus/changes/c1/review.md")).unwrap();
        let err = review_files(&folder.0, "c1").unwrap_err();
        let start = "cannot read .tminus/changes/c1/review.md: ";
        assert!(err.starts_with(start), "{err}");
    }

    fn log_lines(folder: &Folder) -> Vec<log::Record> {
        let text = fs::read_to_string(folder.0.join(log::PATH)).unwrap();
        log::parse(&text).unwrap()
    }

    #[test]
    fn the_review_is_read_and_logged() {
        let folder = Folder::new();
        assert_eq!(review(&folder.0, "c1"), Ok(false));
        folder.write(".tminus/changes/c1/review.md", "Fine.\nVERDICT: OK\n");
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
        folder.write(".tminus/changes/c1/review.md", "VERDICT: KO");
        assert_eq!(review(&folder.0, "c1"), Ok(false));
        assert_eq!(log_lines(&folder)[2].text("note"), "");
    }

    #[test]
    fn the_replan_is_read_from_its_file() {
        let folder = Folder::new();
        assert_eq!(replanned_to_test(&folder.0, "c1"), Ok(false));
        folder.write(".tminus/changes/c1/replan.md", "Plan.\nNEXT: test\n");
        assert_eq!(replanned_to_test(&folder.0, "c1"), Ok(true));
        assert!(!folder.0.join(log::PATH).exists());
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error() {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(".tminus/changes/c1/review.md")).unwrap();
        let err = review(&folder.0, "c1").unwrap_err();
        let start = "cannot read .tminus/changes/c1/review.md: ";
        assert!(err.starts_with(start), "{err}");
        fs::create_dir_all(folder.0.join(".tminus/changes/c1/replan.md")).unwrap();
        let err = replanned_to_test(&folder.0, "c1").unwrap_err();
        let start = "cannot read .tminus/changes/c1/replan.md: ";
        assert!(err.starts_with(start), "{err}");
    }
}
