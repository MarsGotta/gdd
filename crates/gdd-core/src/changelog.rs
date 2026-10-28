//! The changelog, for the people who use the project: each change carries one line in its merge
//! commit as git trailers, `Changelog: <section> · <line>` and `Breaking: yes|no`. `CHANGELOG.md`
//! is the project's, in the Keep a Changelog 1.1.0 format: `gdd changelog` compares it with
//! those commits on the main branch and adds only what is missing, each line with a comment that
//! names its commit (see [`missing`] and [`add`]); every word of the file stays. The line of a
//! change is set with `gdd changelog-line` and kept in its
//! record (see [`record`]); without one, its title goes under the section of its kind of work,
//! and Refactors, Spikes and No-code changes are left out (see [`of_change`]).

use std::collections::BTreeMap;
use std::fs::Permissions;
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

/// The heading of what no version released yet.
const UNRELEASED: &str = "## [Unreleased]";

/// What starts the heading of a version, and of a section.
const VERSION_MARK: &str = "## ";
const SECTION_MARK: &str = "### ";

/// What starts a list item, after the spaces.
const BULLETS: [&str; 2] = ["- ", "* "];

/// What starts the comment of a line gdd added: `<!-- gdd: <commit> -->`.
const COMMENT: &str = "gdd:";

/// What a breaking line starts with.
const BREAKING: &str = "**Breaking:**";

/// How many characters of its commit a line gdd adds names, and how few of them are enough to
/// count when a person wrote the comment.
const ID_LEN: usize = 12;
const MIN_ID: usize = 7;

/// What `gdd changelog` says before the list of what is missing, after it, and before and after
/// asking.
const SHOWN: &str = "Missing from CHANGELOG.md:";
const NOT_WRITTEN: &str = "Nothing written: gdd changelog --write adds them.";
const ASK: &str = "Add them to CHANGELOG.md? [y/N] ";
const NOT_WRITTEN_ASKED: &str = "Nothing written.";

/// What `gdd changelog` says when nothing is missing.
pub const COVERED: &str = "CHANGELOG.md covers every change on main.";

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

/// What the changelog of the project lacks, from the commits of its main branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// The file as it is, as text, and its permissions; `None` when there is none.
    existing: Option<(String, Permissions)>,
    /// Every version of git, for a file that does not exist yet.
    all: Vec<Version>,
    /// The versions of git with only the lines the file does not cover; none is empty.
    versions: Vec<Version>,
}

/// What the changelog of the project at `root`, [`PATH`], lacks from the commits of its main
/// branch `main` (see [`versions`]). Nothing is written. It is never read through a link, nor a
/// file with other names (see [`files`]); a file that is not UTF-8 text is an error.
pub fn missing(root: &Path, main: &str) -> Result<Missing, String> {
    let found = files::found(root, PATH).map_err(|e| format!("cannot read {PATH}: {e}"))?;
    let git = Git::new(root);
    let said = |e: git::Error| e.to_string();
    let history = git.first_parents(main, &KEYS).map_err(said)?;
    let tags = git.tags().map_err(said)?;
    let all = versions(&history, &tags);
    let Some(found) = found else {
        let versions = lacking(&all, &Items::default());
        return Ok(Missing {
            existing: None,
            all,
            versions,
        });
    };
    let text = String::from_utf8(found.bytes).map_err(|_| format!("{PATH} is not UTF-8 text"))?;
    let versions = lacking(&all, &Items::of(&text));
    Ok(Missing {
        existing: Some((text, found.permissions)),
        all,
        versions,
    })
}

/// The versions of `all` with only the lines `items` do not cover; those left empty go.
fn lacking(all: &[Version], items: &Items) -> Vec<Version> {
    let uncovered = |version: &Version| Version {
        heading: version.heading.clone(),
        lines: version
            .lines
            .iter()
            .filter(|carried| !items.cover(carried))
            .cloned()
            .collect(),
    };
    all.iter()
        .map(uncovered)
        .filter(|version| !version.lines.is_empty())
        .collect()
}

impl Missing {
    /// How many lines are missing.
    pub fn count(&self) -> usize {
        self.versions
            .iter()
            .map(|version| version.lines.len())
            .sum()
    }

    /// The blocks to add: each version with its sections and its items, with their comments.
    /// What `gdd changelog --check` prints with its failure too.
    pub fn list(&self) -> String {
        let blocks: Vec<String> = self
            .versions
            .iter()
            .map(|version| block(version).join("\n"))
            .collect();
        blocks.join("\n\n")
    }

    /// What `gdd changelog` shows when it does not ask: the list, and that nothing was written.
    pub fn shown(&self) -> String {
        format!("{SHOWN}\n\n{}\n\n{NOT_WRITTEN}", self.list())
    }

    /// What `gdd changelog` shows before it asks (see [`answered`]), without a line break at the
    /// end.
    pub fn offered(&self) -> String {
        format!("{SHOWN}\n\n{}\n\n{ASK}", self.list())
    }

    /// Why `gdd changelog --check` fails.
    pub fn failure(&self) -> String {
        let count = self.count();
        format!(
            "{PATH} misses {count} line(s): gdd changelog shows them, \
             gdd changelog --write adds them"
        )
    }
}

/// Adds what `missing` lacks to the changelog of the project at `root`, and nothing else; commits
/// nothing. A file that exists is written new with the same permissions, and one that does not is
/// created, never through a link. What it says, in one line.
pub fn add(root: &Path, missing: &Missing) -> Result<String, String> {
    unchanged(root, missing)?;
    let written = match &missing.existing {
        Some((text, permissions)) => {
            let found = files::Found {
                bytes: merged(text, &missing.versions).into_bytes(),
                permissions: permissions.clone(),
            };
            files::put_back(root, PATH, &found)
        }
        None => files::replace(root, PATH, fresh(&missing.all).as_bytes()),
    };
    written.map_err(|e| format!("cannot write {PATH}: {e}"))?;
    let count = missing.count();
    Ok(format!(
        "{PATH}: {count} line(s) added, nothing else changed."
    ))
}

/// Fails when the file is not as [`missing`] read it (or there is one now, where there was none):
/// someone saved it while gdd waited, and writing the old text over it would lose that.
fn unchanged(root: &Path, missing: &Missing) -> Result<(), String> {
    let now = files::read(root, PATH).map_err(|e| format!("cannot write {PATH}: {e}"))?;
    let before = missing.existing.as_ref().map(|(text, _)| text.as_bytes());
    if now.as_deref() == before {
        Ok(())
    } else {
        Err(format!(
            "{PATH} changed since gdd read it: nothing written, run gdd changelog again"
        ))
    }
}

/// What a person's `answer` to [`Missing::offered`] does: `y` or `yes` adds the lines (see
/// [`add`]), anything else writes nothing.
pub fn answered(root: &Path, missing: &Missing, answer: &str) -> Result<String, String> {
    match answer.trim().to_lowercase().as_str() {
        "y" | "yes" => add(root, missing),
        _ => Ok(NOT_WRITTEN_ASKED.to_string()),
    }
}

/// The lines a commit carries that a file has, as its list items say.
#[derive(Debug, Default)]
struct Items {
    /// The commit ids its `<!-- gdd: <id> -->` comments name.
    ids: Vec<String>,
    /// The words of its items, without bullet, comments nor `**Breaking:**`.
    texts: Vec<String>,
}

impl Items {
    /// The list items of `text`: lines that start with `- ` or `* `, after leading spaces.
    fn of(text: &str) -> Items {
        let mut items = Items::default();
        for item in text.lines().filter_map(bullet) {
            let (outside, comments) = split_comments(item);
            let ids = comments.iter().filter_map(|c| c.strip_prefix(COMMENT));
            items.ids.extend(ids.map(|id| id.trim().to_string()));
            let words = outside.trim();
            let words = words.strip_prefix(BREAKING).unwrap_or(words);
            items.texts.push(words.trim().to_string());
        }
        items
    }

    /// Whether `carried` is in the file: a comment names a prefix of its commit of at least
    /// [`MIN_ID`] characters, or an item has its words.
    fn cover(&self, carried: &Carried) -> bool {
        let named = |id: &String| id.len() >= MIN_ID && carried.commit.starts_with(id.as_str());
        self.ids.iter().any(named) || self.texts.contains(&carried.line.text)
    }
}

/// What follows the bullet of the list item `line`, when it is one.
fn bullet(line: &str) -> Option<&str> {
    let line = line.trim_start();
    BULLETS.iter().find_map(|bullet| line.strip_prefix(bullet))
}

/// `item` without its HTML comments, and what each said, without the spaces around it.
fn split_comments(item: &str) -> (String, Vec<&str>) {
    let mut outside = String::new();
    let mut comments = Vec::new();
    let mut rest = item;
    while let Some(start) = rest.find("<!--") {
        outside.push_str(&rest[..start]);
        let after = &rest[start + "<!--".len()..];
        let Some(end) = after.find("-->") else {
            // Never closed: it is words, not a comment.
            outside.push_str(&rest[start..]);
            return (outside, comments);
        };
        comments.push(after[..end].trim());
        rest = &after[end + "-->".len()..];
    }
    outside.push_str(rest);
    (outside, comments)
}

/// A line a commit carries, and the commit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Carried {
    line: Line,
    /// Its full id.
    commit: String,
}

impl Carried {
    /// The item as gdd adds it: the line and a comment with the first [`ID_LEN`] characters of
    /// the commit.
    fn item(&self) -> String {
        let id: String = self.commit.chars().take(ID_LEN).collect();
        format!("{} <!-- gdd: {id} -->", self.line.item())
    }
}

/// A version in the changelog: its heading, and its lines, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    heading: String,
    lines: Vec<Carried>,
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
        let commit = &logged.commit;
        let carry = |line| Carried {
            line,
            commit: commit.clone(),
        };
        current.lines.extend(carried(logged).map(carry));
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
/// is a release, which comes after its pre-releases, or its pre-release identifiers. `None` for
/// anything else.
fn precedence(version: &str) -> Option<Rank> {
    let (version, build) = split(version, '+');
    let (core, pre) = split(version, '-');
    let mut numbers = core.split('.').map(number);
    let known = (numbers.next()??, numbers.next()??, numbers.next()??);
    let pre_ok = pre.is_none_or(|pre| identifiers(pre, true));
    let build_ok = build.is_none_or(|build| identifiers(build, false));
    let valid = numbers.next().is_none() && pre_ok && build_ok;
    let pre = match pre {
        Some(pre) => Pre::Of(pre.split('.').map(Identifier::of).collect()),
        None => Pre::Release,
    };
    valid.then_some(Rank {
        numbers: known,
        pre,
    })
}

/// Where a semantic version stands among others, as semver.org orders them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    numbers: (u64, u64, u64),
    pre: Pre,
}

/// The pre-release of a version: a release comes after every pre-release of its numbers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Pre {
    /// Its identifiers; a shorter list comes first when the rest is equal.
    Of(Vec<Identifier>),
    Release,
}

/// An identifier of a pre-release: digits only are numbers, which come before the rest, which are
/// compared as text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identifier {
    /// Its digits, no leading zero: more digits is a bigger number.
    Number(usize, String),
    Text(String),
}

impl Identifier {
    fn of(text: &str) -> Identifier {
        if text.bytes().all(|byte| byte.is_ascii_digit()) {
            Identifier::Number(text.len(), text.to_string())
        } else {
            Identifier::Text(text.to_string())
        }
    }
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

/// The sections of `version` that have lines, in the order of [`SECTIONS`], with their items.
fn sections(version: &Version) -> Vec<(&'static str, Vec<String>)> {
    let mut sections = Vec::new();
    for section in SECTIONS {
        let items: Vec<String> = version
            .lines
            .iter()
            .filter(|carried| carried.line.section == section)
            .map(Carried::item)
            .collect();
        if !items.is_empty() {
            sections.push((section, items));
        }
    }
    sections
}

/// The lines of `version` as a block of the changelog: its heading, then each section with its
/// items, one blank line between each two parts.
fn block(version: &Version) -> Vec<String> {
    let mut lines = vec![version.heading.clone()];
    for (section, items) in sections(version) {
        lines.push(String::new());
        lines.push(format!("{SECTION_MARK}{section}"));
        lines.push(String::new());
        lines.extend(items);
    }
    lines
}

/// The text of a changelog that does not exist yet, with `versions`, newest first: the header,
/// then each version, even one without lines.
fn fresh(versions: &[Version]) -> String {
    let mut blocks = vec![HEADER.to_string()];
    blocks.extend(versions.iter().map(|version| block(version).join("\n")));
    format!("{}\n", blocks.join("\n\n"))
}

/// Where a version stands among the headings of a changelog: a release by its precedence (see
/// [`precedence`]) and its name, and Unreleased after every release, as the newest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Release(Rank, String),
    Unreleased,
}

/// The version a heading `## ...` names: `[Unreleased]`, in any case, or `[<semver>]`, with or
/// without a leading `v` and the brackets. `None` for any other heading.
fn heading_key(heading: &str) -> Option<Key> {
    let title = heading.strip_prefix(VERSION_MARK)?.trim();
    if title.to_lowercase().starts_with("[unreleased]") {
        return Some(Key::Unreleased);
    }
    let title = title.strip_prefix('[').unwrap_or(title);
    let title = title.strip_prefix('v').unwrap_or(title);
    let name = title
        .split(|c: char| c == ']' || c.is_whitespace())
        .next()?;
    Some(Key::Release(precedence(name)?, name.to_string()))
}

/// A version of a changelog text: the lines from its heading to the next `## `.
struct Block {
    key: Option<Key>,
    start: usize,
    end: usize,
}

/// The lines of a changelog text, as they are, to add lines to.
struct Doc {
    lines: Vec<String>,
    /// What ends the lines added: `\r` when the text ends its lines with `\r\n`.
    eol: &'static str,
}

impl Doc {
    fn new(text: &str) -> Doc {
        Doc {
            lines: text.split_terminator('\n').map(str::to_string).collect(),
            eol: if text.contains("\r\n") { "\r" } else { "" },
        }
    }

    fn text(&self) -> String {
        format!("{}\n", self.lines.join("\n"))
    }

    fn blank(&self, at: usize) -> bool {
        self.lines[at].trim().is_empty()
    }

    /// The versions of the text, in order: each `## ` heading and what follows it.
    fn blocks(&self) -> Vec<Block> {
        let heads: Vec<usize> = (0..self.lines.len())
            .filter(|&at| self.lines[at].starts_with(VERSION_MARK))
            .collect();
        let ends = heads.iter().skip(1).copied().chain([self.lines.len()]);
        let blocks = heads.iter().zip(ends);
        blocks
            .map(|(&start, end)| Block {
                key: heading_key(&self.lines[start]),
                start,
                end,
            })
            .collect()
    }

    /// The block of the version `key`, if the text has it.
    fn block_of(&self, key: &Key) -> Option<Block> {
        let named = |block: &Block| block.key.as_ref() == Some(key);
        self.blocks().into_iter().find(named)
    }

    /// The `### ` headings in `block`, with their position and name (its own `## ` heading is
    /// not one).
    fn headings(&self, block: &Block) -> Vec<(usize, &str)> {
        (block.start..block.end)
            .filter_map(|at| Some((at, self.lines[at].strip_prefix(SECTION_MARK)?.trim())))
            .collect()
    }

    /// Adds `new` before the line `at`.
    fn insert(&mut self, at: usize, new: Vec<String>) {
        let eol = self.eol;
        let new = new.into_iter().map(|line| line + eol);
        self.lines.splice(at..at, new);
    }

    /// Adds `new` before the line `at`, a blank line apart from what is on each side.
    fn insert_apart(&mut self, at: usize, mut new: Vec<String>) {
        if at < self.lines.len() && !self.blank(at) {
            new.push(String::new());
        }
        if at > 0 && !self.blank(at - 1) {
            new.insert(0, String::new());
        }
        self.insert(at, new);
    }

    /// Adds the version `version` where its place is: before the first version of the text that
    /// is older, else at the end.
    fn add_version(&mut self, key: &Key, version: &Version) {
        let older = |block: &Block| block.key.as_ref().is_some_and(|other| other < key);
        let at = self.blocks().iter().find(|b| older(b)).map(|b| b.start);
        self.insert_apart(at.unwrap_or(self.lines.len()), block(version));
    }

    /// Adds the lines of `version` to the block of that version in the text, each under its
    /// section.
    fn add_lines(&mut self, key: &Key, version: &Version) {
        for (section, items) in sections(version) {
            let Some(block) = self.block_of(key) else {
                return;
            };
            let named = |h: &(usize, &str)| h.1 == section;
            let heading = self.headings(&block).into_iter().find(named).map(|h| h.0);
            match heading {
                Some(at) => self.add_items(&block, at, items),
                None => self.add_section(&block, section, items),
            }
        }
    }

    /// Adds `items` to the section whose heading is at `heading`: after its last list item (and
    /// what continues it), or under the heading when it has none.
    fn add_items(&mut self, block: &Block, heading: usize, items: Vec<String>) {
        let headings = self.headings(block);
        let end = headings
            .iter()
            .map(|h| h.0)
            .find(|&at| at > heading)
            .unwrap_or(block.end);
        // The heading itself is no list item, so the search may start at it.
        let last = (heading..end)
            .rev()
            .find(|&at| bullet(&self.lines[at]).is_some());
        match last {
            Some(last) => {
                let at = self.item_end(last, end);
                self.insert(at, items);
            }
            None => self.insert_apart(heading + 1, items),
        }
    }

    /// Where the list item at `item` ends before `end`: it goes on while the lines are text.
    fn item_end(&self, item: usize, end: usize) -> usize {
        let text = |at: usize| {
            let line = &self.lines[at];
            !self.blank(at) && bullet(line).is_none() && !line.starts_with('#')
        };
        (item + 1..end).find(|&at| !text(at)).unwrap_or(end)
    }

    /// Adds the new section `section`, with `items`, to `block`: before the first section of the
    /// version that comes later in [`SECTIONS`], else at the end of the version.
    fn add_section(&mut self, block: &Block, section: &str, items: Vec<String>) {
        let order = |name: &str| SECTIONS.iter().position(|known| *known == name);
        // A name that is no section has no place (`None` is before every place).
        let later = |name: &str| order(name) > order(section);
        let before = self.headings(block).into_iter().find(|h| later(h.1));
        let at = match before {
            Some((at, _)) => at,
            None => (block.start..block.end)
                .rev()
                .find(|&at| !self.blank(at))
                .map_or(block.end, |last| last + 1),
        };
        let mut new = vec![format!("{SECTION_MARK}{section}"), String::new()];
        new.extend(items);
        self.insert_apart(at, new);
    }
}

/// `text` with the lines `missing` lacks added, each in its place (see [`Doc`]); every line of
/// `text` stays, as it is and in order. A version the text has gets its lines under their
/// sections, one it lacks a block of its own.
fn merged(text: &str, missing: &[Version]) -> String {
    let mut doc = Doc::new(text);
    for version in missing {
        match heading_key(&version.heading) {
            Some(key) if doc.block_of(&key).is_some() => doc.add_lines(&key, version),
            Some(key) => doc.add_version(&key, version),
            None => {}
        }
    }
    doc.text()
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
            let rank = precedence(version).unwrap();
            let shown = (rank.numbers.0, rank.numbers.1, rank.numbers.2);
            let release = rank.pre == Pre::Release;
            assert_eq!((shown.0, shown.1, shown.2, release), expected, "{version}");
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
    fn versions_are_ordered_as_semver_orders_them() {
        // The example list of semver.org, lowest first, and numbers by value.
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0-rc.2",
            "1.0.0-rc.10",
            "1.0.0",
            "1.0.1",
            "1.10.0",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            let (low, high) = (precedence(pair[0]), precedence(pair[1]));
            assert!(low < high, "{} < {}", pair[0], pair[1]);
        }
        // A build does not count.
        assert_eq!(precedence("1.0.0+a"), precedence("1.0.0+b"));
    }

    #[test]
    fn a_pre_release_goes_in_its_place_among_the_headings() {
        let text = "## [1.0.0-rc.2]\n\n- a\n\n## [0.9.0]\n\n- b\n";
        let version = Version {
            heading: "## [1.0.0-rc.10] - 2026-10-20".to_string(),
            lines: vec![added("Added", "New")],
        };
        let result = merged(text, &[version]);
        let before = result.find("1.0.0-rc.10").unwrap();
        assert!(before < result.find("1.0.0-rc.2").unwrap(), "{result}");
    }

    #[test]
    fn a_file_that_changed_while_gdd_waited_is_not_written_over() {
        let repo = repository();
        repo.write(PATH, "# Mine\n");
        let missing = missing(repo.root(), "main").unwrap();
        repo.write(PATH, "# Mine\n\nAn edit made meanwhile\n");
        let err = add(repo.root(), &missing).unwrap_err();
        assert!(err.starts_with("CHANGELOG.md changed since"), "{err}");
        assert_eq!(read(&repo), "# Mine\n\nAn edit made meanwhile\n");
        // A file that did not exist and does now.
        fs::remove_file(repo.root().join(PATH)).unwrap();
        let missing = super::missing(repo.root(), "main").unwrap();
        repo.write(PATH, "# Theirs\n");
        assert!(add(repo.root(), &missing).is_err());
        assert_eq!(read(&repo), "# Theirs\n");
        // A file that went away.
        repo.write(PATH, "# Mine\n");
        let missing = super::missing(repo.root(), "main").unwrap();
        fs::remove_file(repo.root().join(PATH)).unwrap();
        assert!(add(repo.root(), &missing).is_err());
        assert!(!repo.root().join(PATH).exists());
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
                lines: vec![by("e", line("Added", "Five", false))],
            },
            Version {
                heading: "## [1.1.0] - 2026-10-27".to_string(),
                lines: vec![by("c", line("Fixed", "Three", false))],
            },
            Version {
                heading: "## [1.0.0] - 2026-10-25".to_string(),
                lines: vec![
                    by("b", line("Changed", "Two", true)),
                    by("a", line("Added", "One", false)),
                ],
            },
        ];
        assert_eq!(split, expected);
        let fresh = versions(&history[..1], &[tag("v1.0.0", "e")]);
        let empty = Version::new(UNRELEASED.to_string());
        assert_eq!(fresh[0], empty);
        assert_eq!(fresh[1].heading, "## [1.0.0] - 2026-10-28");
        assert_eq!(versions(&[], &[]), [empty]);
    }

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";

    fn by(commit: &str, line: Line) -> Carried {
        Carried {
            line,
            commit: commit.to_string(),
        }
    }

    fn version(heading: &str, lines: Vec<Carried>) -> Version {
        Version {
            heading: heading.to_string(),
            lines,
        }
    }

    #[test]
    fn a_new_changelog_lists_each_version_by_sections_in_their_order() {
        let versions = [
            version(
                UNRELEASED,
                vec![
                    by(ID, line("Security", "Locks", false)),
                    by(ID, line("Added", "Export", false)),
                    by(ID, line("Fixed", "Crash", false)),
                    by(ID, line("Added", "Import", true)),
                ],
            ),
            Version::new("## [1.0.0] - 2026-10-20".to_string()),
        ];
        let expected = format!(
            "{HEADER}\n\n## [Unreleased]\n\n\
             ### Added\n\n- Export <!-- gdd: 0123456789ab -->\n\
             - **Breaking:** Import <!-- gdd: 0123456789ab -->\n\n\
             ### Fixed\n\n- Crash <!-- gdd: 0123456789ab -->\n\n\
             ### Security\n\n- Locks <!-- gdd: 0123456789ab -->\n\n\
             ## [1.0.0] - 2026-10-20\n"
        );
        assert_eq!(fresh(&versions), expected);
        let header = "# Changelog\n\nAll notable changes to this project are documented in this \
                      file. The format is based on \
                      [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project \
                      adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).";
        assert_eq!(HEADER, header);
    }

    #[test]
    fn a_line_is_covered_by_the_commit_a_comment_names_or_by_its_words() {
        let text = "# Notes\n\n\
                    ## [Unreleased]\n\n\
                    - Reworded <!-- gdd: 0123456 -->\n\
                    \x20 * **Breaking:** Same words <!-- a note --> <!-- other -->\n\
                    - Short <!-- gdd: 012345 -->\n\
                    Export is not a list item\n\
                    - <!-- gdd: 9999999 -->\n";
        let items = Items::of(text);
        let covers =
            |commit: &str, said: &str| items.cover(&by(commit, line("Added", said, false)));
        // Seven characters of the commit name it, whatever the words.
        assert!(covers(ID, "Nobody"));
        assert!(covers("0123456", "Nobody"));
        assert!(covers("9999999a", "Nobody"));
        // Not a prefix of the commit, or too short.
        assert!(!covers("012345a", "Nobody"));
        assert!(!covers("0123450", "Nobody"));
        // The words count in any item, all of them.
        assert!(covers("ffffff0", "Same words"));
        assert!(!covers("ffffff0", "Same"));
        assert!(!covers("ffffff0", "Export is not a list item"));
        let short = Items::of("- Short <!-- gdd: 012345 -->\n");
        assert!(!short.cover(&by(ID, line("Added", "Other", false))));
        let bare = "- A <!-- never closed\n";
        assert_eq!(Items::of(bare).texts, ["A <!-- never closed"]);
    }

    #[test]
    fn only_the_lines_a_file_does_not_cover_are_missing() {
        let all = [
            version(
                UNRELEASED,
                vec![by("aaaaaaaaaaaa1", line("Added", "One", false))],
            ),
            version(
                "## [1.0.0] - 2026-10-20",
                vec![
                    by("bbbbbbbbbbbb2", line("Fixed", "Two", false)),
                    by("cccccccccccc3", line("Fixed", "Three", false)),
                ],
            ),
        ];
        let items = Items::of("- Two\n- Zero <!-- gdd: cccccccccccc -->\n");
        let missing = lacking(&all, &items);
        assert_eq!(missing, [all[0].clone()]);
        let items = Items::of("- One\n- Two\n- Three\n");
        assert!(lacking(&all, &items).is_empty());
        assert_eq!(lacking(&all, &Items::default()), all);
    }

    #[test]
    fn headings_name_their_version_in_any_of_the_ways_people_write_it() {
        let release = |name: &str| Some(Key::Release(precedence(name)?, name.to_string()));
        for heading in [
            "## [1.0.0] - 2026-10-20",
            "## 1.0.0",
            "## v1.0.0 (2026-10-20)",
            "## [v1.0.0]",
            "##   [1.0.0]  ",
        ] {
            assert_eq!(heading_key(heading), release("1.0.0"), "{heading}");
        }
        for heading in [
            "## [Unreleased]",
            "## [unreleased] - soon",
            "## [UNRELEASED]",
        ] {
            assert_eq!(heading_key(heading), Some(Key::Unreleased), "{heading}");
        }
        let pre = release("1.0.0-rc.1");
        assert_eq!(heading_key("## [1.0.0-rc.1]"), pre);
        for heading in [
            "## Notes",
            "# [1.0.0]",
            "### [1.0.0]",
            "##[1.0.0]",
            "## [1.0]",
            "## ",
        ] {
            assert_eq!(heading_key(heading), None, "{heading:?}");
        }
        let mut keys = [
            Key::Unreleased,
            release("1.0.0").unwrap(),
            release("0.9.0").unwrap(),
        ];
        keys.sort();
        assert_eq!(keys[0], release("0.9.0").unwrap());
        assert_eq!(keys[2], Key::Unreleased);
    }

    /// The lines of `text`, as `merged` keeps them.
    fn lines_of(text: &str) -> Vec<&str> {
        text.split_terminator('\n').collect()
    }

    /// Whether the lines of `original` are in `result`, in order.
    fn keeps(original: &str, result: &str) -> bool {
        let mut rest = lines_of(result).into_iter();
        lines_of(original)
            .iter()
            .all(|line| rest.any(|l| l == *line))
    }

    fn added(section: &str, text: &str) -> Carried {
        by(ID, line(section, text, false))
    }

    #[test]
    fn a_missing_line_goes_after_the_last_item_of_its_section() {
        let text = "# Notes\n\n## [Unreleased]\n\n### Added\n\n- Dark theme\n  more words\n\n\
                    ### Fixed\n\n- Crash\n\n## [1.0.0] - 2026-10-20\n\n### Added\n\n- Old\n";
        let missing = [version(UNRELEASED, vec![added("Added", "Export")])];
        let expected = "# Notes\n\n## [Unreleased]\n\n### Added\n\n- Dark theme\n  more words\n\
                        - Export <!-- gdd: 0123456789ab -->\n\n\
                        ### Fixed\n\n- Crash\n\n## [1.0.0] - 2026-10-20\n\n### Added\n\n- Old\n";
        assert_eq!(merged(text, &missing), expected);
        let two = [version(
            "## [1.0.0] - 2026-10-20",
            vec![added("Added", "A"), added("Added", "B")],
        )];
        let result = merged(text, &two);
        let tail = "### Added\n\n- Old\n- A <!-- gdd: 0123456789ab -->\n\
                    - B <!-- gdd: 0123456789ab -->\n";
        assert!(result.ends_with(tail), "{result}");
        assert!(keeps(text, &result));
    }

    #[test]
    fn a_missing_section_goes_before_the_first_later_one_else_at_the_end_of_its_version() {
        let text = "## [Unreleased]\n\n### Removed\n\n- Old\n\n### Notes\n\n- x\n\n\
                    ## [1.0.0] - 2026-10-20\n\n### Added\n\n- A\n";
        let one = |section| [version(UNRELEASED, vec![added(section, "New")])];
        let before = merged(text, &one("Added"));
        let expected = "## [Unreleased]\n\n### Added\n\n- New <!-- gdd: 0123456789ab -->\n\n\
                        ### Removed\n\n- Old\n\n### Notes\n\n- x\n\n\
                        ## [1.0.0] - 2026-10-20\n\n### Added\n\n- A\n";
        assert_eq!(before, expected);
        let end = merged(text, &one("Security"));
        let expected = "## [Unreleased]\n\n### Removed\n\n- Old\n\n### Notes\n\n- x\n\n\
                        ### Security\n\n- New <!-- gdd: 0123456789ab -->\n\n\
                        ## [1.0.0] - 2026-10-20\n\n### Added\n\n- A\n";
        assert_eq!(end, expected);
        let bare = "## [Unreleased]\n## [1.0.0]\n- A\n";
        let result = merged(bare, &one("Fixed"));
        let expected = "## [Unreleased]\n\n### Fixed\n\n- New <!-- gdd: 0123456789ab -->\n\n\
                        ## [1.0.0]\n- A\n";
        assert_eq!(result, expected);
        let empty = "## [Unreleased]\n\n### Fixed\n\n## [1.0.0]\n";
        let result = merged(empty, &one("Fixed"));
        let expected = "## [Unreleased]\n\n### Fixed\n\n- New <!-- gdd: 0123456789ab -->\n\n\
                        ## [1.0.0]\n";
        assert_eq!(result, expected);
        let tight = "## [Unreleased]\n### Fixed\n## [1.0.0]\n";
        let result = merged(tight, &one("Fixed"));
        assert!(keeps(tight, &result), "{result}");
        assert!(result.contains("### Fixed\n\n- New <!-- gdd: 0123456789ab -->\n\n## [1.0.0]\n"));
    }

    #[test]
    fn a_missing_version_goes_before_the_first_older_one_else_at_the_end() {
        let old = "## [1.0.0] - 2026-10-20\n\n### Added\n\n- A\n";
        let both = [
            version(UNRELEASED, vec![added("Fixed", "Now")]),
            version("## [1.5.0] - 2026-10-25", vec![added("Added", "Mid")]),
        ];
        let text = format!("# Notes\n\nSome words.\n\n## Upcoming\n\n{old}\n## [0.1.0] - 2026\n");
        let result = merged(&text, &both);
        let expected = format!(
            "# Notes\n\nSome words.\n\n## Upcoming\n\n\
             ## [Unreleased]\n\n### Fixed\n\n- Now <!-- gdd: 0123456789ab -->\n\n\
             ## [1.5.0] - 2026-10-25\n\n### Added\n\n- Mid <!-- gdd: 0123456789ab -->\n\n\
             {old}\n## [0.1.0] - 2026\n"
        );
        assert_eq!(result, expected);
        assert!(keeps(&text, &result));
        let oldest = [version(
            "## [0.0.1] - 2026-01-01",
            vec![added("Added", "First")],
        )];
        let result = merged(&text, &oldest);
        let tail = "## [0.1.0] - 2026\n\n## [0.0.1] - 2026-01-01\n\n### Added\n\n\
                    - First <!-- gdd: 0123456789ab -->\n";
        assert!(result.ends_with(tail), "{result}");
        let no_heading = merged("Words\n", &oldest);
        let expected = "Words\n\n## [0.0.1] - 2026-01-01\n\n### Added\n\n\
                        - First <!-- gdd: 0123456789ab -->\n";
        assert_eq!(no_heading, expected);
        let from_nothing = merged("", &oldest);
        assert!(
            from_nothing.starts_with("## [0.0.1] - 2026-01-01\n"),
            "{from_nothing}"
        );
    }

    #[test]
    fn a_new_section_goes_after_the_last_line_of_the_version_whatever_blank_lines_end_it() {
        let text = "## [Unreleased]\n\n### Added\n\n- A\n\n\n## [1.0.0]\n";
        let missing = [version(UNRELEASED, vec![added("Security", "New")])];
        let expected = "## [Unreleased]\n\n### Added\n\n- A\n\n\
                        ### Security\n\n- New <!-- gdd: 0123456789ab -->\n\n\n## [1.0.0]\n";
        assert_eq!(merged(text, &missing), expected);
    }

    #[test]
    fn a_section_the_version_has_is_never_a_later_one_and_nor_is_a_version_an_older_one() {
        let mut doc = Doc::new("## [Unreleased]\n\n### Added\n\n- A\n");
        let block = doc.block_of(&Key::Unreleased).unwrap();
        doc.add_section(&block, "Added", vec!["- B".to_string()]);
        let twice = "## [Unreleased]\n\n### Added\n\n- A\n\n### Added\n\n- B\n";
        assert_eq!(doc.text(), twice);
        let mut doc = Doc::new("## [1.0.0]\n\n- A\n");
        let key = heading_key("## [1.0.0]").unwrap();
        doc.add_version(&key, &Version::new("## [1.0.0] - 2026-10-20".to_string()));
        assert_eq!(doc.text(), "## [1.0.0]\n\n- A\n\n## [1.0.0] - 2026-10-20\n");
    }

    #[test]
    fn the_original_lines_stay_in_order_on_every_shape_of_file() {
        let missing = [
            version(
                UNRELEASED,
                vec![added("Added", "New"), added("Security", "Safe")],
            ),
            version("## [1.0.0] - 2026-10-20", vec![added("Fixed", "Old")]),
            version("## [0.5.0] - 2026-10-10", vec![added("Changed", "Older")]),
        ];
        let texts = [
            "",
            "\n",
            "Just words",
            "# Changelog\n",
            "# Changelog\n\n\n\n",
            "## [Unreleased]\n",
            "## [Unreleased]\n### Added\n- A\n",
            "# C\n\n## [1.0.0] - 2026-10-20\n\n### Fixed\n\n- X\n\n\n## [0.1.0]\n\n- Y\n",
            "# C\r\n\r\n## [Unreleased]\r\n\r\n### Added\r\n\r\n- A\r\n\r\n## [1.0.0]\r\n",
            "## [Unreleased]\n\n```\n## [1.0.0]\n```\n\n* star item\n   - indented\n",
            "## v0.5.0\n\n### Changed\n\n- Older\n\n## Notes\n\nSome words\n",
        ];
        for text in texts {
            let result = merged(text, &missing);
            assert!(keeps(text, &result), "{text:?} -> {result:?}");
            assert!(
                result.ends_with('\n') && !result.ends_with("\n\n\n"),
                "{result:?}"
            );
            for item in ["- New", "- Safe", "- Old", "- Older"] {
                let count = result.matches(item).count();
                assert!(count >= 1, "{item} in {result:?}");
            }
        }
        let crlf = merged(texts[8], &missing);
        assert!(crlf.lines().all(|l| !l.contains('\r')), "{crlf:?}");
        assert!(
            !crlf.contains("\n\n") && !crlf.replace("\r\n", "").contains('\n'),
            "{crlf:?}"
        );
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

    fn read(repo: &Repo) -> String {
        fs::read_to_string(repo.root().join(PATH)).unwrap()
    }

    #[test]
    fn without_a_file_everything_is_missing_and_adding_creates_it() {
        let repo = repository();
        let missing = missing(repo.root(), "main").unwrap();
        assert_eq!(missing.count(), 2);
        let day = repo.git(&["log", "-1", "--format=%cs", "v1.0.0"]);
        let list = missing.list();
        assert!(list.starts_with("## [Unreleased]\n\n### Changed\n\n- **Breaking:** One file"));
        assert!(list.contains(&format!(
            "\n\n## [1.0.0] - {day}\n\n### Added\n\n- Export <!-- gdd: "
        )));
        assert!(
            missing
                .shown()
                .starts_with("Missing from CHANGELOG.md:\n\n## [Unreleased]")
        );
        assert!(
            missing
                .shown()
                .ends_with("\n\nNothing written: gdd changelog --write adds them.")
        );
        assert!(
            missing
                .offered()
                .ends_with("\n\nAdd them to CHANGELOG.md? [y/N] ")
        );
        let failure = "CHANGELOG.md misses 2 line(s): gdd changelog shows them, \
                       gdd changelog --write adds them";
        assert_eq!(missing.failure(), failure);
        assert!(!repo.root().join(PATH).exists());
        let said = add(repo.root(), &missing).unwrap();
        assert_eq!(said, "CHANGELOG.md: 2 line(s) added, nothing else changed.");
        let text = read(&repo);
        assert!(text.starts_with(HEADER), "{text}");
        assert!(text.contains(
            "## [Unreleased]\n\n### Changed\n\n- **Breaking:** One file per folder <!-- gdd: "
        ));
        assert!(!text.contains("Inside the branch"), "{text}");
        assert_eq!(repo.git(&["status", "--porcelain"]), "?? CHANGELOG.md");
        // What it wrote covers everything.
        let again = super::missing(repo.root(), "main").unwrap();
        assert_eq!(again.count(), 0);
    }

    #[test]
    fn a_file_of_the_project_keeps_its_words_and_gets_only_what_is_missing() {
        let repo = repository();
        let mine = "# Release notes\n\n## [Unreleased]\n\n### Changed\n\n- One file per folder\n";
        repo.write(PATH, mine);
        let missing = missing(repo.root(), "main").unwrap();
        assert_eq!(missing.count(), 1);
        add(repo.root(), &missing).unwrap();
        let text = read(&repo);
        assert!(text.starts_with(mine), "{text}");
        assert!(text.contains("## [1.0.0] - "), "{text}");
        assert!(keeps(mine, &text));
        assert_eq!(super::missing(repo.root(), "main").unwrap().count(), 0);
    }

    #[test]
    fn adding_keeps_the_permissions_of_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let repo = repository();
        repo.write(PATH, "# Mine\n");
        let path = repo.root().join(PATH);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let missing = missing(repo.root(), "main").unwrap();
        add(repo.root(), &missing).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn only_a_yes_adds_the_lines() {
        let repo = repository();
        let missing = missing(repo.root(), "main").unwrap();
        for no in ["", "n", "no", "N", "yep", "yes please", "\n"] {
            let said = answered(repo.root(), &missing, no).unwrap();
            assert_eq!(said, "Nothing written.", "{no:?}");
            assert!(!repo.root().join(PATH).exists());
        }
        for yes in ["y\n", "Y", "yes", " YES \n"] {
            let said = answered(repo.root(), &missing, yes).unwrap();
            assert!(
                said.starts_with("CHANGELOG.md: 2 line(s) added"),
                "{yes:?}: {said}"
            );
            fs::remove_file(repo.root().join(PATH)).unwrap();
        }
    }

    #[test]
    fn a_file_gdd_cannot_read_or_write_is_an_error_and_stays() {
        let repo = repository();
        repo.write(PATH, "# My own notes\n");
        // Never through a link, nor over a file with other names.
        fs::remove_file(repo.root().join(PATH)).unwrap();
        let outside = Folder::new();
        outside.write("notes.md", "# Outside\n");
        symlink(outside.0.join("notes.md"), repo.root().join(PATH)).unwrap();
        let err = missing(repo.root(), "main").unwrap_err();
        let start = "cannot read CHANGELOG.md: not a file of its own";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(repo.root().join(PATH)).unwrap();
        fs::hard_link(outside.0.join("notes.md"), repo.root().join(PATH)).unwrap();
        assert!(missing(repo.root(), "main").is_err());
        let notes = fs::read_to_string(outside.0.join("notes.md")).unwrap();
        assert_eq!(notes, "# Outside\n");
        fs::remove_file(repo.root().join(PATH)).unwrap();
        fs::write(repo.root().join(PATH), [0xff, 0xfe, b'\n']).unwrap();
        let err = missing(repo.root(), "main").unwrap_err();
        assert_eq!(err, "CHANGELOG.md is not UTF-8 text");
        // A link that appears after reading: nothing is written, through it or over it.
        fs::remove_file(repo.root().join(PATH)).unwrap();
        let ready = missing(repo.root(), "main").unwrap();
        symlink(outside.0.join("notes.md"), repo.root().join(PATH)).unwrap();
        let err = add(repo.root(), &ready).unwrap_err();
        let start = "cannot write CHANGELOG.md: not a file of its own";
        assert!(err.starts_with(start), "{err}");
        let notes = fs::read_to_string(outside.0.join("notes.md")).unwrap();
        assert_eq!(notes, "# Outside\n");
        fs::remove_file(repo.root().join(PATH)).unwrap();
        // A project root that is a file cannot be written to either.
        let file = repo.root().join("a-file");
        fs::write(&file, "x\n").unwrap();
        let err = add(&file, &ready).unwrap_err();
        assert!(err.starts_with("cannot write CHANGELOG.md: "), "{err}");
    }

    #[test]
    fn what_the_changelog_cannot_read_is_an_error() {
        let repo = repository();
        let err = missing(repo.root(), "trunk").unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
        assert!(!repo.root().join(PATH).exists());
        let err = missing(repo.root(), "-x").unwrap_err();
        assert_eq!(err, "\"-x\" is not a revision git can take");
        // A project root that is a file: not a folder to read from.
        let file = repo.root().join("a-file");
        fs::write(&file, "x\n").unwrap();
        let err = missing(&file, "main").unwrap_err();
        assert!(err.starts_with("cannot read CHANGELOG.md: "), "{err}");
        let plain = Folder::new();
        let err = missing(&plain.0, "main").unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
    }
}
