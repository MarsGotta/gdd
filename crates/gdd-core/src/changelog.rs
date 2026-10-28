//! The changelog, for the people who use the project: each change carries one line in its merge
//! commit as git trailers, `Changelog: <section> · <line>` and `Breaking: yes|no`, and
//! `gdd changelog` writes `CHANGELOG.md` from those commits on the main branch, in the Keep a
//! Changelog 1.1.0 format. The line of a change is set with `gdd changelog-line` and kept in its
//! record (see [`record`]); without one, its title goes under the section of its kind of work,
//! and Refactors, Spikes and No-code changes are left out (see [`of_change`]).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::agents::files;
use crate::change;
use crate::gates::approve;
use crate::git::{self, Git, Logged};
use crate::log;

/// The sections of Keep a Changelog, in the order a version lists them.
pub const SECTIONS: [&str; 6] = [
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

/// The changelog, from the project root.
pub const PATH: &str = "CHANGELOG.md";

/// The field of the record of a change that keeps its line.
const FIELD: &str = "changelog";

/// The trailers a merge commit carries the line of its change in: the line, then whether it
/// breaks what worked before.
const KEYS: [&str; 2] = ["Changelog", "Breaking"];

/// What separates the section of a line from its words.
const SEPARATOR: &str = " · ";

/// What the record and the log say of a change left out of the changelog.
const LEFT_OUT: &str = "left out";

/// The title of the changelog and what it follows.
const HEADER: &str = "# Changelog\n\n\
                      All notable changes to this project are documented in this file. The \
                      format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), \
                      and this project adheres to \
                      [Semantic Versioning](https://semver.org/spec/v2.0.0.html).";

/// The line that marks a changelog gdd wrote, which it may write again.
const MARKER: &str = "<!-- Written by gdd changelog from the merge commits: change a line with \
                      gdd changelog-line, not here. -->";

/// The heading of what no version released yet.
const UNRELEASED: &str = "## [Unreleased]";

/// Why a changelog gdd did not write stays as it is.
const NOT_OURS: &str =
    "CHANGELOG.md was not written by gdd: move it aside, then run gdd changelog again";

/// Why a line without words is refused.
const EMPTY: &str = "the changelog line is empty: give it its words";

/// The characters that break a line without being control characters: the line and paragraph
/// separators of Unicode.
const BREAKS: [char; 2] = ['\u{2028}', '\u{2029}'];

/// Why a line on more than one line is refused.
const NOT_ONE_LINE: &str = "the changelog line must be one line, without line breaks or other \
                            control characters";

/// Why a record whose line is not one `gdd changelog-line` writes is refused.
const NOT_A_LINE: &str = "the field \"changelog\" of the record is not a line of gdd \
                          changelog-line: a section, the line and whether it is breaking, or \
                          none";

/// The line of a change in the changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// One of [`SECTIONS`].
    pub section: String,
    /// What it says.
    pub text: String,
    /// Whether the change breaks what worked before.
    pub breaking: bool,
}

impl Line {
    /// `Ok` when the line can go in a merge commit and in the changelog: its section is one of
    /// [`SECTIONS`], and it has words, on one line, without control characters nor [`BREAKS`].
    /// Else why not.
    pub fn check(&self) -> Result<(), String> {
        if !SECTIONS.contains(&self.section.as_str()) {
            let sections = SECTIONS.join(", ");
            return Err(format!("the section must be one of {sections}"));
        }
        if self.text.trim().is_empty() {
            return Err(EMPTY.to_string());
        }
        let breaks = |c: char| c.is_control() || BREAKS.contains(&c);
        if self.text.chars().any(breaks) {
            return Err(NOT_ONE_LINE.to_string());
        }
        Ok(())
    }

    /// The line as gdd shows it: `<section> · <text>`.
    pub fn shown(&self) -> String {
        format!("{}{SEPARATOR}{}", self.section, self.text)
    }

    /// The paragraph of trailers of the line in a merge commit: `Changelog: <section> · <text>`,
    /// then `Breaking: yes` or `Breaking: no`.
    pub fn trailers(&self) -> String {
        let [line, breaking] = KEYS;
        let said = if self.breaking { "yes" } else { "no" };
        format!("{line}: {}\n{breaking}: {said}", self.shown())
    }

    /// The line as the changelog lists it: `- <text>`, with `**Breaking:**` first when it is.
    fn item(&self) -> String {
        let mark = if self.breaking { "**Breaking:** " } else { "" };
        format!("- {mark}{}", self.text)
    }
}

/// The line of the change whose record is `record`: the one `gdd changelog-line` gave it, or
/// none when it left the change out. Without either, its title, on one line, under the section
/// of its kind of work: Feature, Added; Hotfix, Fixed; Migration, Changed. The other kinds, and a
/// change without a title, have none. A line the record keeps that is not one
/// `gdd changelog-line` writes is an error.
pub fn of_change(record: &Value) -> Result<Option<Line>, String> {
    let given = &record[FIELD];
    if given.is_null() {
        return Ok(by_kind(record));
    }
    if given["none"] == true {
        return Ok(None);
    }
    let breaking = match &given["breaking"] {
        Value::Null => false,
        Value::Bool(breaking) => *breaking,
        _ => return Err(NOT_A_LINE.to_string()),
    };
    let (Some(section), Some(text)) = (given["section"].as_str(), given["line"].as_str()) else {
        return Err(NOT_A_LINE.to_string());
    };
    let line = Line {
        section: section.to_string(),
        text: text.to_string(),
        breaking,
    };
    line.check()?;
    Ok(Some(line))
}

/// The line of the change whose record is `record` without one of its own (see [`of_change`]).
fn by_kind(record: &Value) -> Option<Line> {
    let kind = record["type"].as_str().unwrap_or(change::DEFAULT_KIND);
    let section = match kind {
        "Feature" => "Added",
        "Hotfix" => "Fixed",
        "Migration" => "Changed",
        _ => return None,
    };
    let title = one_line(record["title"].as_str().unwrap_or_default());
    let line = Line {
        section: section.to_string(),
        text: title,
        breaking: false,
    };
    line.check().ok().map(|()| line)
}

/// The words of `text`, whatever separated them (line breaks and other control characters too),
/// with one space between each two.
fn one_line(text: &str) -> String {
    let words = text.split(|c: char| c.is_whitespace() || c.is_control());
    words
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Records on change `id` of the project whose state root is `state` its line in the changelog,
/// `line`, without the spaces around its words; with `None`, that it is left out. For the role
/// `role` (`None` for a person or the Orchestrator), and logged. A Builder or a Reviewer is
/// refused before anything else (see [`approve::may_record`]), then a line that cannot be one as
/// it was given, a line break at either end too (see [`Line::check`]). What it says, in one line.
pub fn record(
    state: &Path,
    id: &str,
    line: Option<Line>,
    role: Option<&str>,
) -> Result<String, String> {
    approve::may_record(role).map_err(|e| e.to_string())?;
    // Checked before the spaces go: trimming first would take away a line break at either end.
    if let Some(line) = &line {
        line.check()?;
    }
    let line = line.map(|line| Line {
        text: line.text.trim().to_string(),
        ..line
    });
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let (field, said) = match &line {
        Some(line) => {
            let field = serde_json::json!({
                "section": line.section, "line": line.text, "breaking": line.breaking,
            });
            (field, line.shown())
        }
        None => (serde_json::json!({"none": true}), LEFT_OUT.to_string()),
    };
    let recorded = change::with_fields(&text, vec![(FIELD, field)]);
    let recorded = recorded.map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &path, &recorded)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "1-discover".to_string(),
        gate: "-".to_string(),
        role: "orchestrator".to_string(),
        agent: "-".to_string(),
        result: "changelog-line".to_string(),
        duration_s: 0.0,
        note: said.clone(),
    };
    change::logged(state, &entry)?;
    Ok(format!("Changelog line of {id}: {said}"))
}

/// Writes the changelog of the project at `root`, [`PATH`], from the commits of its main branch
/// `main` (see [`versions`]), and commits nothing. A changelog without [`MARKER`] was not written
/// by gdd: it stays as it is, and that is an error. It is never read or written through a link,
/// nor over a file with other names (see [`files`]). What it says, in one line.
pub fn write(root: &Path, main: &str) -> Result<String, String> {
    let found = files::found(root, PATH).map_err(|e| format!("cannot read {PATH}: {e}"))?;
    let ours = |found: &files::Found| String::from_utf8_lossy(&found.bytes).contains(MARKER);
    if found.as_ref().is_some_and(|found| !ours(found)) {
        return Err(NOT_OURS.to_string());
    }
    let git = Git::new(root);
    let said = |e: git::Error| e.to_string();
    let history = git.first_parents(main, &KEYS).map_err(said)?;
    let tags = git.tags().map_err(said)?;
    let versions = versions(&history, &tags);
    let unreleased = versions.first().map_or(0, |version| version.lines.len());
    let text = render(&versions);
    files::replace(root, PATH, text.as_bytes()).map_err(|e| format!("cannot write {PATH}: {e}"))?;
    Ok(format!(
        "{PATH} written: {unreleased} line(s) under Unreleased."
    ))
}

/// A version in the changelog: its heading, and its lines, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    heading: String,
    lines: Vec<Line>,
}

impl Version {
    fn new(heading: String) -> Version {
        Version {
            heading,
            lines: Vec::new(),
        }
    }
}

/// The versions of `history`, the commits of the main branch along its first parents, newest
/// first: what no tag released yet, under Unreleased, then each version a tag of `tags`, each
/// with the commit it names, released (see [`released`]), with the day of its commit. Each
/// version has the lines its commits carry (see [`carried`]).
fn versions(history: &[Logged], tags: &[(String, String)]) -> Vec<Version> {
    let released = released(tags);
    let mut versions = Vec::new();
    let mut current = Version::new(UNRELEASED.to_string());
    for logged in history {
        if let Some(version) = released.get(logged.commit.as_str()) {
            let heading = format!("## [{version}] - {}", logged.day);
            versions.push(std::mem::replace(&mut current, Version::new(heading)));
        }
        current.lines.extend(carried(logged));
    }
    versions.push(current);
    versions
}

/// The line the commit `logged`, read with the trailers [`KEYS`], carries: its first `Changelog`
/// trailer, when it is a section and words (see [`Line::check`]); breaking when its first
/// `Breaking` trailer says `yes`.
fn carried(logged: &Logged) -> Option<Line> {
    let [lines, breaking] = logged.trailers.as_slice() else {
        return None;
    };
    let (section, text) = lines.first()?.split_once(SEPARATOR)?;
    let line = Line {
        section: section.to_string(),
        text: text.trim().to_string(),
        breaking: breaking.first().is_some_and(|said| said == "yes"),
    };
    line.check().ok().map(|()| line)
}

/// The version each commit released, by commit, from `tags`, each with the commit it names: what
/// follows the `v` of a tag `v<semver>`. Of several on one commit, the highest (see
/// [`precedence`]).
fn released(tags: &[(String, String)]) -> BTreeMap<&str, &str> {
    let mut versions: Vec<_> = tags
        .iter()
        .filter_map(|(name, commit)| {
            let version = name.strip_prefix('v')?;
            Some((precedence(version)?, version, commit.as_str()))
        })
        .collect();
    versions.sort_unstable();
    // The highest of a commit comes last, and stays.
    versions
        .into_iter()
        .map(|(_, version, commit)| (commit, version))
        .collect()
}

/// Where `version` stands among versions, when it is a semantic version 2.0.0:
/// `MAJOR.MINOR.PATCH`, then maybe `-<pre-release>` and `+<build>`. Its numbers, then whether it
/// is a release, which comes after its pre-releases. `None` for anything else.
fn precedence(version: &str) -> Option<(u64, u64, u64, bool)> {
    let (version, build) = split(version, '+');
    let (core, pre) = split(version, '-');
    let mut numbers = core.split('.').map(number);
    let known = (numbers.next()??, numbers.next()??, numbers.next()??);
    let pre_ok = pre.is_none_or(|pre| identifiers(pre, true));
    let build_ok = build.is_none_or(|build| identifiers(build, false));
    let valid = numbers.next().is_none() && pre_ok && build_ok;
    valid.then_some((known.0, known.1, known.2, pre.is_none()))
}

/// `text` before the first `at`, and what follows it, if it has one.
fn split(text: &str, at: char) -> (&str, Option<&str>) {
    match text.split_once(at) {
        Some((before, after)) => (before, Some(after)),
        None => (text, None),
    }
}

/// The number `text` is, in digits, without a leading zero: `None` for anything else, a sign too.
fn number(text: &str) -> Option<u64> {
    let digits = text.bytes().all(|byte| byte.is_ascii_digit());
    let leading_zero = text.len() > 1 && text.starts_with('0');
    if digits && !leading_zero {
        text.parse().ok()
    } else {
        None
    }
}

/// Whether `text` is identifiers of a semantic version, separated by dots: letters, digits and
/// hyphens, never empty; those of a pre-release, `pre_release`, that are digits only have no
/// leading zero.
fn identifiers(text: &str, pre_release: bool) -> bool {
    text.split('.').all(|id| {
        let allowed = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-';
        let well_formed = !id.is_empty() && id.bytes().all(allowed);
        let numeric = id.bytes().all(|byte| byte.is_ascii_digit());
        well_formed && !(pre_release && numeric && number(id).is_none())
    })
}

/// The text of the changelog with `versions`, newest first: the header and the marker, then each
/// version with each section of [`SECTIONS`] it has lines in, in that order.
fn render(versions: &[Version]) -> String {
    let mut blocks = vec![HEADER.to_string(), MARKER.to_string()];
    for version in versions {
        blocks.push(version.heading.clone());
        for section in SECTIONS {
            let items: Vec<String> = version
                .lines
                .iter()
                .filter(|line| line.section == section)
                .map(Line::item)
                .collect();
            if !items.is_empty() {
                blocks.push(format!("### {section}"));
                blocks.push(items.join("\n"));
            }
        }
    }
    format!("{}\n", blocks.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};
    use std::fs;
    use std::os::unix::fs::symlink;

    fn line(section: &str, text: &str, breaking: bool) -> Line {
        Line {
            section: section.to_string(),
            text: text.to_string(),
            breaking,
        }
    }

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_line_has_a_section_of_keep_a_changelog_and_words_on_one_line() {
        for section in SECTIONS {
            assert_eq!(line(section, "Export notes", false).check(), Ok(()));
        }
        let sections = "the section must be one of Added, Changed, Deprecated, Removed, Fixed, \
                        Security";
        for wrong in ["Improved", "added", "", "Added "] {
            let checked = line(wrong, "Faster", false).check();
            assert_eq!(checked, Err(sections.to_string()), "{wrong:?}");
        }
        for empty in ["", "   "] {
            let checked = line("Added", empty, false).check();
            assert_eq!(checked, Err(EMPTY.to_string()), "{empty:?}");
        }
        for broken in [
            "two\nlines",
            "Export\n",
            "\nExport",
            "a\rb",
            "a\tb",
            "a\u{1f}b",
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
        ] {
            let checked = line("Fixed", broken, true).check();
            assert_eq!(checked, Err(NOT_ONE_LINE.to_string()), "{broken:?}");
        }
    }

    #[test]
    fn a_line_goes_in_trailers_and_in_a_list() {
        let added = line("Added", "Export notes as Markdown", false);
        assert_eq!(added.shown(), "Added · Export notes as Markdown");
        let trailers = "Changelog: Added · Export notes as Markdown\nBreaking: no";
        assert_eq!(added.trailers(), trailers);
        assert_eq!(added.item(), "- Export notes as Markdown");
        let breaking = line("Changed", "One file per folder", true);
        let trailers = "Changelog: Changed · One file per folder\nBreaking: yes";
        assert_eq!(breaking.trailers(), trailers);
        assert_eq!(breaking.item(), "- **Breaking:** One file per folder");
    }

    #[test]
    fn without_a_line_the_title_goes_under_the_section_of_its_kind() {
        let by = |kind: &str| {
            let title = " Crash \n on\tstart\u{1f}\u{2028}";
            let record = serde_json::json!({"title": title, "type": kind});
            of_change(&record).unwrap()
        };
        let title = "Crash on start";
        assert_eq!(by("Feature"), Some(line("Added", title, false)));
        assert_eq!(by("Hotfix"), Some(line("Fixed", title, false)));
        assert_eq!(by("Migration"), Some(line("Changed", title, false)));
        for kind in ["Refactor", "Spike", "No-code", "Other"] {
            assert_eq!(by(kind), None, "{kind}");
        }
        let untyped = of_change(&json(r#"{"title": "App"}"#)).unwrap();
        assert_eq!(untyped, Some(line("Added", "App", false)));
        for untitled in [r#"{"type": "Feature"}"#, r#"{"title": " \n "}"#, "{}"] {
            assert_eq!(of_change(&json(untitled)).unwrap(), None, "{untitled}");
        }
    }

    #[test]
    fn the_line_of_the_record_comes_first_and_none_leaves_the_change_out() {
        let given = r#"{"title": "App", "type": "Refactor", "changelog":
                        {"section": "Removed", "line": "Old export", "breaking": true}}"#;
        let removed = line("Removed", "Old export", true);
        assert_eq!(of_change(&json(given)).unwrap(), Some(removed));
        let plain = r#"{"changelog": {"section": "Fixed", "line": "Crash"}}"#;
        let fixed = line("Fixed", "Crash", false);
        assert_eq!(of_change(&json(plain)).unwrap(), Some(fixed));
        let none = r#"{"title": "App", "type": "Feature", "changelog": {"none": true}}"#;
        assert_eq!(of_change(&json(none)).unwrap(), None);
        let null = r#"{"title": "App", "changelog": null}"#;
        let app = Some(line("Added", "App", false));
        assert_eq!(of_change(&json(null)).unwrap(), app);
    }

    #[test]
    fn a_line_of_the_record_that_gdd_does_not_write_is_an_error() {
        for wrong in [
            r#"{"changelog": {"section": "Added", "line": "x", "breaking": "yes"}}"#,
            r#"{"changelog": {"section": "Added"}}"#,
            r#"{"changelog": {"line": "x"}}"#,
            r#"{"changelog": {"none": false}}"#,
            r#"{"changelog": "Added"}"#,
        ] {
            let err = of_change(&json(wrong));
            assert_eq!(err, Err(NOT_A_LINE.to_string()), "{wrong}");
        }
        let bad = r#"{"changelog": {"section": "Improved", "line": "x"}}"#;
        let err = of_change(&json(bad)).unwrap_err();
        assert!(err.starts_with("the section must be one of "), "{err}");
        let broken = r#"{"changelog": {"section": "Added", "line": "a\nb"}}"#;
        assert_eq!(of_change(&json(broken)), Err(NOT_ONE_LINE.to_string()));
    }

    /// A state root with the record of change `c1`.
    fn with_change() -> Folder {
        let folder = Folder::new();
        let record = r#"{"id": "c1", "title": "App", "type": "Feature", "x": 1}"#;
        folder.write(".gdd/changes/c1/change.json", record);
        folder
    }

    fn record_of(folder: &Folder) -> Value {
        let text = change::read_record(&folder.0, "c1").unwrap();
        json(&text)
    }

    fn logged(folder: &Folder) -> Vec<log::Record> {
        let text = fs::read_to_string(folder.0.join(log::PATH)).unwrap_or_default();
        log::parse(&text).unwrap()
    }

    #[test]
    fn the_line_of_a_change_is_recorded_and_logged() {
        let folder = with_change();
        let given = line("Changed", "  Notes in one file  ", true);
        let said = record(&folder.0, "c1", Some(given), None).unwrap();
        assert_eq!(said, "Changelog line of c1: Changed · Notes in one file");
        let record_now = record_of(&folder);
        let field = serde_json::json!({
            "section": "Changed", "line": "Notes in one file", "breaking": true,
        });
        assert_eq!(record_now[FIELD], field);
        assert_eq!(record_now["x"], 1);
        let read = of_change(&record_now).unwrap();
        assert_eq!(read, Some(line("Changed", "Notes in one file", true)));
        let entries = logged(&folder);
        let [entry] = entries.as_slice() else {
            panic!("one log line: {entries:?}");
        };
        for (key, value) in [
            ("change", "c1"),
            ("phase", "1-discover"),
            ("gate", "-"),
            ("role", "orchestrator"),
            ("agent", "-"),
            ("result", "changelog-line"),
            ("note", "Changed · Notes in one file"),
        ] {
            assert_eq!(entry.text(key), value, "{key}");
        }
        let said = record(&folder.0, "c1", None, Some("orchestrator")).unwrap();
        assert_eq!(said, "Changelog line of c1: left out");
        let none = serde_json::json!({"none": true});
        assert_eq!(record_of(&folder)[FIELD], none);
        assert_eq!(of_change(&record_of(&folder)).unwrap(), None);
        assert_eq!(logged(&folder)[1].text("note"), "left out");
    }

    #[test]
    fn agents_wrong_lines_and_unknown_changes_record_nothing() {
        let folder = with_change();
        let before = record_of(&folder);
        let refused = approve::may_record(Some("reviewer")).unwrap_err();
        let refused = refused.to_string();
        let start = "A Builder or Reviewer cannot";
        assert!(refused.starts_with(start), "{refused}");
        for role in ["builder-build", "builder-test", "reviewer"] {
            let err = record(&folder.0, "c1", None, Some(role)).unwrap_err();
            assert_eq!(err, refused, "{role}");
        }
        let wrong = line("Improved", "Faster", false);
        let err = record(&folder.0, "c1", Some(wrong), None).unwrap_err();
        assert!(err.starts_with("the section must be one of "), "{err}");
        let blank = line("Added", "  ", false);
        let err = record(&folder.0, "c1", Some(blank), None).unwrap_err();
        assert_eq!(err, EMPTY);
        // The spaces around the words go, but only after the check: a break at either end stays.
        for broken in [
            "Export notes\n",
            "\nExport notes",
            "Export notes\r\n",
            " \tExport notes ",
            "Export notes\u{2028}",
        ] {
            let given = line("Added", broken, false);
            let err = record(&folder.0, "c1", Some(given), None).unwrap_err();
            assert_eq!(err, NOT_ONE_LINE, "{broken:?}");
        }
        assert_eq!(record_of(&folder), before);
        assert!(logged(&folder).is_empty());
        let err = record(&folder.0, "c9", None, None).unwrap_err();
        assert!(err.starts_with("change c9 does not exist"), "{err}");
        folder.write(".gdd/changes/c2/change.json", "[]");
        let err = record(&folder.0, "c2", None, None).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c2/change.json: "), "{err}");
        fs::create_dir_all(folder.0.join(log::PATH)).unwrap();
        let err = record(&folder.0, "c1", None, None).unwrap_err();
        assert!(err.starts_with("cannot write .gdd/log.jsonl: "), "{err}");
    }

    #[test]
    fn only_semantic_versions_count() {
        for (version, expected) in [
            ("0.0.0", (0, 0, 0, true)),
            ("1.2.3", (1, 2, 3, true)),
            ("10.20.30", (10, 20, 30, true)),
            ("1.0.0-0", (1, 0, 0, false)),
            ("1.0.0-rc.1", (1, 0, 0, false)),
            ("1.0.0-0a.x-y", (1, 0, 0, false)),
            ("1.0.0+001", (1, 0, 0, true)),
            ("1.0.0-alpha+build.5", (1, 0, 0, false)),
        ] {
            assert_eq!(precedence(version), Some(expected), "{version}");
        }
        for wrong in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.a.3",
            "+1.2.3",
            "1.2.+3",
            " 1.2.3",
            "1.2.3-",
            "1.2.3+",
            "1.2.3-01",
            "1.2.3-rc.01",
            "1.2.3-a..b",
            "1.2.3-a_b",
            "1.2.3+a_b",
            "99999999999999999999.0.0",
        ] {
            assert_eq!(precedence(wrong), None, "{wrong:?}");
        }
    }

    #[test]
    fn a_number_is_digits_only_without_a_leading_zero() {
        for (text, expected) in [("0", 0), ("7", 7), ("10", 10)] {
            assert_eq!(number(text), Some(expected), "{text:?}");
        }
        // `u64::from_str` alone takes a `+` sign; a version never does.
        for text in ["", "+5", "-5", "05", "00", " 5", "5a"] {
            assert_eq!(number(text), None, "{text:?}");
        }
        assert_eq!(number("99999999999999999999"), None);
    }

    fn tag(name: &str, commit: &str) -> (String, String) {
        (name.to_string(), commit.to_string())
    }

    #[test]
    fn a_commit_releases_the_highest_version_it_is_tagged_with() {
        let tags = [
            tag("v1.0.0", "a"),
            tag("v1.0.0-rc.1", "a"),
            tag("v0.9.0", "b"),
            tag("v0.10.0", "b"),
            tag("v2.0.0-beta", "c"),
            tag("v2.0.0-alpha", "c"),
            tag("1.0.0", "d"),
            tag("vacation", "d"),
            tag("v1.0", "d"),
        ];
        let expected = BTreeMap::from([("a", "1.0.0"), ("b", "0.10.0"), ("c", "2.0.0-beta")]);
        assert_eq!(released(&tags), expected);
    }

    fn commit(id: &str, day: &str, lines: &[&str], breaking: &[&str]) -> Logged {
        let owned = |said: &[&str]| said.iter().map(|s| s.to_string()).collect();
        Logged {
            commit: id.to_string(),
            day: day.to_string(),
            trailers: vec![owned(lines), owned(breaking)],
        }
    }

    #[test]
    fn a_commit_carries_its_first_line_when_it_is_one() {
        let carried_by = |lines: &[&str], breaking: &[&str]| {
            carried(&commit("a", "2026-10-28", lines, breaking))
        };
        let added = Some(line("Added", "Export", false));
        assert_eq!(carried_by(&["Added · Export "], &[]), added);
        let two = ["Added · Export", "Fixed · Other"];
        assert_eq!(carried_by(&two, &["no"]), added);
        let breaking = Some(line("Changed", "A · B", true));
        assert_eq!(carried_by(&["Changed · A · B"], &["yes", "no"]), breaking);
        let changed = Some(line("Changed", "A · B", false));
        assert_eq!(carried_by(&["Changed · A · B"], &["no", "yes"]), changed);
        assert_eq!(carried_by(&["Changed · A · B"], &["Yes"]), changed);
        let wrong: [&[&str]; 4] = [&[], &["Added"], &["Improved · Faster"], &["Added · "]];
        for lines in wrong {
            assert_eq!(carried_by(lines, &["yes"]), None, "{lines:?}");
        }
        let mut only_lines = commit("a", "2026-10-28", &["Added · Export"], &[]);
        only_lines.trailers.pop();
        assert_eq!(carried(&only_lines), None);
    }

    #[test]
    fn tags_split_the_history_into_versions_newest_first() {
        let history = [
            commit("e", "2026-10-28", &["Added · Five"], &["no"]),
            commit("d", "2026-10-27", &[], &[]),
            commit("c", "2026-10-26", &["Fixed · Three"], &["no"]),
            commit("b", "2026-10-25", &["Changed · Two"], &["yes"]),
            commit("a", "2026-10-24", &["Added · One"], &["no"]),
        ];
        let tags = [tag("v1.1.0", "d"), tag("v1.0.0", "b"), tag("v9.0.0", "z")];
        let split = versions(&history, &tags);
        let expected = [
            Version {
                heading: UNRELEASED.to_string(),
                lines: vec![line("Added", "Five", false)],
            },
            Version {
                heading: "## [1.1.0] - 2026-10-27".to_string(),
                lines: vec![line("Fixed", "Three", false)],
            },
            Version {
                heading: "## [1.0.0] - 2026-10-25".to_string(),
                lines: vec![line("Changed", "Two", true), line("Added", "One", false)],
            },
        ];
        assert_eq!(split, expected);
        let fresh = versions(&history[..1], &[tag("v1.0.0", "e")]);
        let empty = Version::new(UNRELEASED.to_string());
        assert_eq!(fresh[0], empty);
        assert_eq!(fresh[1].heading, "## [1.0.0] - 2026-10-28");
        assert_eq!(versions(&[], &[]), [empty]);
    }

    #[test]
    fn the_changelog_lists_each_version_by_sections_in_their_order() {
        let versions = [
            Version {
                heading: UNRELEASED.to_string(),
                lines: vec![
                    line("Security", "Locks", false),
                    line("Added", "Export", false),
                    line("Fixed", "Crash", false),
                    line("Added", "Import", true),
                ],
            },
            Version::new("## [1.0.0] - 2026-10-20".to_string()),
        ];
        let expected = format!(
            "{HEADER}\n\n{MARKER}\n\n## [Unreleased]\n\n\
             ### Added\n\n- Export\n- **Breaking:** Import\n\n\
             ### Fixed\n\n- Crash\n\n\
             ### Security\n\n- Locks\n\n\
             ## [1.0.0] - 2026-10-20\n"
        );
        assert_eq!(render(&versions), expected);
        let header = "# Changelog\n\nAll notable changes to this project are documented in this \
                      file. The format is based on \
                      [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project \
                      adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).";
        assert_eq!(HEADER, header);
        let marker = "<!-- Written by gdd changelog from the merge commits: change a line with \
                      gdd changelog-line, not here. -->";
        assert_eq!(MARKER, marker);
    }

    /// A repository whose `main` has, along its first parents: a commit without a line, the
    /// merge of a branch whose own commit has a line too, tagged `v1.0.0`, then a commit with a
    /// breaking line and one with a line that is not one. Tags that are no version, or that name
    /// a commit off the first parents, release nothing.
    fn repository() -> Repo {
        let repo = Repo::new();
        repo.identify();
        repo.start("c1");
        repo.write("a.txt", "a\n");
        repo.git(&["add", "-A"]);
        let inside = "c1\n\nChangelog: Removed · Inside the branch\nBreaking: no";
        repo.git(&["commit", "-q", "-m", inside]);
        repo.git(&["checkout", "-q", "main"]);
        let merge = "Merge c1\n\nRecord: x\n\nChangelog: Added · Export\nBreaking: no";
        repo.git(&["merge", "-q", "--no-ff", "-m", merge, "change/c1"]);
        let annotated = [
            "-c",
            "tag.gpgsign=false",
            "tag",
            "-a",
            "-m",
            "Release",
            "v1.0.0",
        ];
        repo.git(&annotated);
        repo.git(&["tag", "v1.0", "HEAD~1"]);
        repo.git(&["tag", "v0.1.0", "change/c1"]);
        let breaking = "Store\n\nChangelog: Changed · One file per folder\nBreaking: yes";
        repo.git(&["commit", "-q", "--allow-empty", "-m", breaking]);
        let wrong = "Odd\n\nChangelog: Improved · Faster";
        repo.git(&["commit", "-q", "--allow-empty", "-m", wrong]);
        repo
    }

    #[test]
    fn the_changelog_is_written_from_the_commits_of_main() {
        let repo = repository();
        let said = write(repo.root(), "main").unwrap();
        assert_eq!(said, "CHANGELOG.md written: 1 line(s) under Unreleased.");
        let text = fs::read_to_string(repo.root().join(PATH)).unwrap();
        let day = repo.git(&["log", "-1", "--format=%cs", "v1.0.0"]);
        let versions = format!(
            "{MARKER}\n\n## [Unreleased]\n\n### Changed\n\n- **Breaking:** One file per folder\n\n\
             ## [1.0.0] - {day}\n\n### Added\n\n- Export\n"
        );
        assert!(text.starts_with(HEADER), "{text}");
        assert!(text.ends_with(&versions), "{text}");
        assert!(!text.contains("Inside the branch"), "{text}");
        assert_eq!(repo.git(&["status", "--porcelain"]), "?? CHANGELOG.md");
        // Written by gdd, it is written again.
        let fix = "Fix\n\nChangelog: Fixed · Crash";
        repo.git(&["commit", "-q", "--allow-empty", "-m", fix]);
        let said = write(repo.root(), "main").unwrap();
        assert_eq!(said, "CHANGELOG.md written: 2 line(s) under Unreleased.");
        let text = fs::read_to_string(repo.root().join(PATH)).unwrap();
        assert!(text.contains("### Fixed\n\n- Crash\n"), "{text}");
    }

    #[test]
    fn a_changelog_gdd_did_not_write_is_left_alone() {
        let repo = repository();
        repo.write(PATH, "# My own notes\n");
        let err = write(repo.root(), "main").unwrap_err();
        assert_eq!(err, NOT_OURS);
        let text = fs::read_to_string(repo.root().join(PATH)).unwrap();
        assert_eq!(text, "# My own notes\n");
        // Never through a link, nor over a file with other names.
        fs::remove_file(repo.root().join(PATH)).unwrap();
        let outside = Folder::new();
        outside.write("notes.md", &format!("{MARKER}\n"));
        symlink(outside.0.join("notes.md"), repo.root().join(PATH)).unwrap();
        let err = write(repo.root(), "main").unwrap_err();
        let start = "cannot read CHANGELOG.md: not a file of its own";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(repo.root().join(PATH)).unwrap();
        fs::hard_link(outside.0.join("notes.md"), repo.root().join(PATH)).unwrap();
        assert!(write(repo.root(), "main").is_err());
        let notes = fs::read_to_string(outside.0.join("notes.md")).unwrap();
        assert_eq!(notes, format!("{MARKER}\n"));
    }

    #[test]
    fn what_the_changelog_cannot_read_is_an_error() {
        let repo = repository();
        let err = write(repo.root(), "trunk").unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
        assert!(!repo.root().join(PATH).exists());
        let err = write(repo.root(), "-x").unwrap_err();
        assert_eq!(err, "\"-x\" is not a revision git can take");
        // A project root that is a file: not a folder to read from.
        let file = repo.root().join("a-file");
        fs::write(&file, "x\n").unwrap();
        let err = write(&file, "main").unwrap_err();
        assert!(err.starts_with("cannot read CHANGELOG.md: "), "{err}");
        let plain = Folder::new();
        let err = write(&plain.0, "main").unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
    }
}
