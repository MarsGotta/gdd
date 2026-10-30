//! Versions of the project: git tags `v<major>.<minor>.<patch>` (Semantic Versioning 2.0.0) on
//! its main branch, from the changelog lines its merge commits carry (see [`changelog`]). The
//! next version is the newest one with the largest bump of the lines since it (see [`Since`]):
//! Fixed and Security, a patch; Added, Changed, Deprecated and Removed, a minor; a breaking line,
//! a major, or a minor while the major is 0. The first version of a project is 0.1.0.
//!
//! By default (`[versions] mode = "auto"` of `gdd/rules.toml`) a change with a changelog line
//! gets its version when it is promoted: the promotion gets it in `GDD_VERSION`, and it is
//! released only once the promotion passed (see [`promote`]). With `mode = "manual"` promotions
//! release nothing, and `gdd release` releases what is merged (see [`by_hand`]). A release turns
//! the Unreleased section of the CHANGELOG.md the main branch keeps into the version, in a commit
//! of its own, and tags it (see [`release`]); a tag is never moved or deleted.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::agents::files;
use crate::change;
use crate::changelog::{self, Line, Pre, Rank};
use crate::gates::{exceptions, release};
use crate::git::{self, Git, Logged};
use crate::guard;
use crate::log;
use crate::recipe::queue;

/// The first version of a project.
pub const FIRST: Version = Version {
    major: 0,
    minor: 1,
    patch: 0,
};

/// What `gdd release` says after what it would release, without releasing it.
const NOT_RELEASED: &str = "Nothing released: gdd release --yes releases it.";

/// What `gdd release` says when a person answers anything but a yes.
const NOT_RELEASED_ASKED: &str = "Nothing released.";

/// What `gdd release` says when the project has no version yet and nothing to release.
const NOTHING_YET: &str = "Nothing to release: no change on main carries a changelog line";

/// Why a version chosen by hand is refused, after what was given.
const NOT_A_VERSION: &str = "is not a version: MAJOR.MINOR.PATCH, such as 1.0.0";

/// How a project releases its versions: `[versions] mode` of `gdd/rules.toml`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Each change with a changelog line gets its version when it is promoted.
    #[default]
    Auto,
    /// Promotions release nothing: `gdd release` does.
    Manual,
}

/// What the rules file of a project says of its releases.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rules {
    /// How versions are released.
    pub mode: Mode,
    /// Whether the main branch and the tags are pushed when there is a remote: `push_main`.
    pub push: bool,
}

impl Rules {
    /// Reads `[versions] mode` and `push_main` of the text of a rules file; the rest is ignored.
    /// Without them, auto and no push. A mode that is neither `auto` nor `manual` is an error
    /// that names the file.
    pub fn from_toml(text: &str) -> Result<Rules, String> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Raw {
            push_main: bool,
            versions: Versions,
        }
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Versions {
            mode: Mode,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| format!("{}: {e}", guard::RULES))?;
        Ok(Rules {
            mode: raw.versions.mode,
            push: raw.push_main,
        })
    }
}

/// A version gdd releases: `major.minor.patch`, without pre-release or build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    /// The version `text` names: `x.y.z`, with or without a `v` first, each number in digits
    /// without a leading zero. `None` for anything else, a pre-release or a build too.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut numbers = text.split('.').map(changelog::number);
        let version = Version {
            major: numbers.next()??,
            minor: numbers.next()??,
            patch: numbers.next()??,
        };
        numbers.next().is_none().then_some(version)
    }

    /// Its tag: `v<version>`.
    pub fn tag(self) -> String {
        format!("v{self}")
    }

    /// The version whose numbers are `numbers`.
    fn of((major, minor, patch): (u64, u64, u64)) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    /// Where it stands among the versions of the tags: after every pre-release of its numbers.
    fn rank(self) -> Rank {
        Rank {
            numbers: (self.major, self.minor, self.patch),
            pre: Pre::Release,
        }
    }

    /// The version after it with `bump`: a patch resets nothing, a minor the patch, a major the
    /// minor and the patch.
    fn bumped(self, bump: Bump) -> Version {
        let next = |number: u64| number.saturating_add(1);
        match bump {
            Bump::Patch => Version {
                patch: next(self.patch),
                ..self
            },
            Bump::Minor => Version {
                minor: next(self.minor),
                patch: 0,
                ..self
            },
            Bump::Major => Version::of((next(self.major), 0, 0)),
        }
    }
}

/// What a line asks of the next version, smallest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bump {
    Patch,
    Minor,
    Major,
}

/// The bump of `line` after a version whose major is `major`: Fixed and Security, a patch; the
/// other sections, a minor; a breaking line, a major, or a minor while the major is 0.
fn bump(line: &Line, major: u64) -> Bump {
    match (line.breaking, line.section.as_str()) {
        (true, _) if major == 0 => Bump::Minor,
        (true, _) => Bump::Major,
        (false, "Fixed" | "Security") => Bump::Patch,
        (false, _) => Bump::Minor,
    }
}

/// What the main branch of a project has since its newest version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Since {
    /// The newest version, by its tag, and where it stands; `None` before the first one.
    newest: Option<(String, Rank)>,
    /// The lines its commits carry since then, newest first.
    lines: Vec<Line>,
}

impl Since {
    /// What the main branch `main` of the project at `root` has since its newest version (see
    /// [`since`]).
    pub fn read(root: &Path, main: &str) -> Result<Since, String> {
        let git = Git::new(root);
        let said = |e: git::Error| e.to_string();
        let history = git.first_parents(main, &changelog::KEYS).map_err(said)?;
        let tags = git.tags().map_err(said)?;
        Ok(since(&history, &tags))
    }

    /// The next version: the newest one with the largest bump of the lines (see [`bump`]), or
    /// [`FIRST`] when there is none yet, whatever the bump. `None` without lines: nothing to
    /// release.
    pub fn next(&self) -> Option<Version> {
        let major = self.newest.as_ref().map_or(0, |(_, rank)| rank.numbers.0);
        let largest = self.lines.iter().map(|line| bump(line, major)).max()?;
        let Some((_, rank)) = &self.newest else {
            return Some(FIRST);
        };
        Some(Version::of(rank.numbers).bumped(largest))
    }

    /// The version a release takes: `chosen` when given, which must be a version (see
    /// [`Version::parse`]) that comes after the newest one, else the next one (see
    /// [`Since::next`]). `None` when there are no lines to release.
    pub fn version(&self, chosen: Option<&str>) -> Result<Option<Version>, String> {
        let Some(given) = chosen else {
            return Ok(self.next());
        };
        let version = Version::parse(given).ok_or_else(|| format!("{given:?} {NOT_A_VERSION}"))?;
        if let Some((tag, rank)) = &self.newest
            && version.rank() <= *rank
        {
            return Err(format!("the version must come after {tag}"));
        }
        Ok((!self.lines.is_empty()).then_some(version))
    }

    /// What `gdd release` says when there is nothing to release.
    pub fn nothing(&self) -> String {
        match &self.newest {
            Some((tag, _)) => format!("Nothing to release since {tag}"),
            None => NOTHING_YET.to_string(),
        }
    }

    /// What `gdd release` shows of `version` without releasing it: the version and where it
    /// comes from, its lines by section, then how to release it.
    pub fn shown(&self, version: Version) -> String {
        format!("{}\n\n{NOT_RELEASED}", self.listed(version))
    }

    /// What `gdd release` shows of `version` before it asks whether to release it (see [`yes`]),
    /// without a line break at the end.
    pub fn offered(&self, version: Version) -> String {
        let tag = version.tag();
        format!("{}\n\nRelease {tag}? [y/N] ", self.listed(version))
    }

    /// `version` and where it comes from, then the lines as list items under the heading of
    /// their section, in the order of [`changelog::SECTIONS`].
    fn listed(&self, version: Version) -> String {
        let from = match &self.newest {
            Some((tag, _)) => format!("from {tag}"),
            None => "first version".to_string(),
        };
        let mut parts = vec![format!("Next version: {} ({from})", version.tag())];
        for section in changelog::SECTIONS {
            let items: Vec<String> = self
                .lines
                .iter()
                .filter(|line| line.section == section)
                .map(Line::item)
                .collect();
            if !items.is_empty() {
                parts.push(format!("### {section}\n\n{}", items.join("\n")));
            }
        }
        parts.join("\n\n")
    }
}

/// What `history`, the commits of a main branch along its first parents, newest first, has since
/// its newest version among `tags`, each with the commit it names: the highest version of a tag
/// `v<semver>` on one of those commits, pre-releases by their precedence; then the lines of the
/// commits after it (see [`changelog::carried`]), all of them before the first version.
fn since(history: &[Logged], tags: &[(String, String)]) -> Since {
    let commits = history.iter().map(|logged| logged.commit.as_str());
    let on_main: BTreeSet<&str> = commits.collect();
    let newest = tags
        .iter()
        .filter(|(_, commit)| on_main.contains(commit.as_str()))
        .filter_map(|(name, commit)| {
            let rank = changelog::precedence(name.strip_prefix('v')?)?;
            Some((rank, name, commit))
        })
        .max();
    let after = |logged: &&Logged| {
        let newest = newest.as_ref();
        newest.is_none_or(|(_, _, commit)| logged.commit != **commit)
    };
    let lines = history
        .iter()
        .take_while(after)
        .filter_map(changelog::carried);
    Since {
        lines: lines.collect(),
        newest: newest.map(|(rank, name, _)| (name.clone(), rank)),
    }
}

/// Whether a person's `answer` to [`Since::offered`] releases: `y` or `yes`, in any case.
pub fn yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// What a person's `answer` to [`Since::offered`] of `version` does in the main checkout at
/// `root`: a yes (see [`yes`]) releases that version from its main branch `main`, as
/// [`by_hand`] does, with `push` and `waiting`; anything else releases nothing. What it says.
pub fn answered(
    root: &Path,
    main: &str,
    version: Version,
    push: bool,
    answer: &str,
    waiting: impl FnOnce(),
) -> Result<String, String> {
    if !yes(answer) {
        return Ok(NOT_RELEASED_ASKED.to_string());
    }
    by_hand(root, main, Some(&version.to_string()), push, waiting)
}

/// Whether the main checkout at `root` can release `version` on the main branch `main` now:
/// `true` when main tracks a CHANGELOG.md, which the release commits. An error when the tag of
/// the version exists already, as gdd never moves or deletes one; and, with a CHANGELOG.md, when
/// the checkout is not on main or its CHANGELOG.md is not the one main has, in the working tree
/// or in the index, as the release commit would carry what nobody committed, or overwrite what
/// was added to the index. The file is read with the rules of [`files`].
pub fn ready(root: &Path, main: &str, version: Version) -> Result<bool, String> {
    let git = Git::new(root);
    let said = |e: git::Error| e.to_string();
    let (tag, path) = (version.tag(), changelog::PATH);
    if git.exists(&format!("refs/tags/{tag}")).map_err(said)? {
        return Err(format!(
            "the tag {tag} exists already: gdd never moves or deletes a tag"
        ));
    }
    let Some(committed) = git.file_at(main, path).map_err(said)? else {
        return Ok(false);
    };
    let current = git.current_branch().map_err(said)?;
    if current != main {
        let checkout = root.display();
        return Err(format!(
            "the release commits {path} on {main}, and {checkout} is on {current}: check out \
             {main} there first"
        ));
    }
    let here = files::read(root, path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let staged = git.has_staged(path).map_err(said)?;
    if staged || here.as_deref() != Some(committed.as_slice()) {
        return Err(format!(
            "{path} has changes {main} does not have: commit them, or put them aside, before the \
             release"
        ));
    }
    Ok(true)
}

/// Releases `version` from the main checkout at `root`, on its main branch `main`, once it may
/// (see [`ready`]). With a CHANGELOG.md that main tracks, its release (see
/// [`changelog::release`]), on the day of today, committed alone on main: `Release v<version>`.
/// Then the annotated tag of the version, with that same message, on the head of main. With
/// `push` and a remote, main and the tag are pushed.
pub fn release(root: &Path, main: &str, version: Version, push: bool) -> Result<(), String> {
    let tracked = ready(root, main, version)?;
    let git = Git::new(root);
    let said = |e: git::Error| e.to_string();
    let (tag, message) = (version.tag(), format!("Release {}", version.tag()));
    if tracked {
        let day = exceptions::today().to_string();
        changelog::release(root, main, &version.to_string(), &day)?;
        let path = [changelog::PATH.to_string()];
        git.commit_files(&path, &message).map_err(said)?;
    }
    git.tag(&tag, &message, &format!("refs/heads/{main}"))
        .map_err(said)?;
    if push && git.has_remote().map_err(said)? {
        git.push(main).map_err(said)?;
        git.push(&format!("refs/tags/{tag}")).map_err(said)?;
    }
    Ok(())
}

/// What the promotion of a change reads from the project.
#[derive(Debug, Clone, Copy)]
pub struct Promotion<'a> {
    /// The project root, where the promote step runs.
    pub root: &'a Path,
    /// The state root: its `.gdd/` holds the record of the change, and its main checkout takes
    /// the release.
    pub state: &'a Path,
    /// The main branch.
    pub main: &'a str,
    /// The id of the change.
    pub id: &'a str,
    /// The release commands of the project.
    pub steps: &'a release::Steps,
    /// Whether the change needs the human G4, as it is classified now.
    pub needs_g4: bool,
    /// What the rules file of the main checkout says of releases.
    pub rules: Rules,
}

/// `gdd release promote` (see [`release::promote`]) under the merge lock, which the merge or the
/// promotion of another change makes it wait for, saying so with `say` (see [`queue::lock`]): two
/// promotions never take the same version. In auto mode, a change with a changelog line (see
/// [`changelog::of_change`]) gets the next version of the main branch in `GDD_VERSION`, once it
/// may be released (see [`ready`]); only when the promotion passed is it released (see
/// [`release`]), kept in the record of the change (`version`) and logged, and `say` says
/// `Released v<version>`. What the promotion printed.
pub fn promote(at: &Promotion, mut say: impl FnMut(&str)) -> Result<String, String> {
    let _lock = queue::lock(at.state, || say(queue::WAITING))?;
    let version = due(at)?;
    if let Some(version) = version {
        ready(at.state, at.main, version)?;
    }
    let given = version.map(|version| version.to_string());
    let (root, state, id) = (at.root, at.state, at.id);
    let promoted = release::promote(root, state, id, at.steps, at.needs_g4, given.as_deref())?;
    let Some(version) = version else {
        return Ok(promoted);
    };
    let tag = version.tag();
    let released = release(state, at.main, version, at.rules.push);
    released.map_err(|e| format!("Promoted, but {tag} was not released: {e}"))?;
    recorded(state, id, version)?;
    say(&format!("Released {tag}"));
    Ok(promoted)
}

/// The version change `at` gets when it is promoted: in auto mode, when it has a changelog line,
/// the next version of the main branch (see [`Since::next`]). None in manual mode, for a change
/// left out of the changelog, or when nothing is new since the newest version.
fn due(at: &Promotion) -> Result<Option<Version>, String> {
    if at.rules.mode == Mode::Manual {
        return Ok(None);
    }
    let (_, record) = release::read(at.state, at.id)?;
    if changelog::of_change(&record)?.is_none() {
        return Ok(None);
    }
    Ok(Since::read(at.state, at.main)?.next())
}

/// Keeps `version` in the record of change `id`, under the state root `state`, and logs that it
/// was released.
fn recorded(state: &Path, id: &str, version: Version) -> Result<(), String> {
    let text = change::read_record(state, id)?;
    let path = change::record_path(id);
    let fields = vec![("version", Value::from(version.to_string()))];
    let kept = change::with_fields(&text, fields).map_err(|e| format!("{path}: {e}"))?;
    change::write(state, &path, &kept)?;
    let entry = log::Entry {
        change: id.to_string(),
        phase: "4-release".to_string(),
        gate: "-".to_string(),
        role: "machine".to_string(),
        agent: "-".to_string(),
        result: "released".to_string(),
        duration_s: 0.0,
        note: version.tag(),
    };
    change::logged(state, &entry)
}

/// `gdd release --yes`: releases what the main branch `main` of the project whose main checkout
/// is at `root` has since its newest version (see [`release`]), as `chosen` asks (see
/// [`Since::version`]), with `push`. It runs no promote step. All of it under the merge lock;
/// `waiting` is called while the merge or the promotion of a change holds it. What it says.
pub fn by_hand(
    root: &Path,
    main: &str,
    chosen: Option<&str>,
    push: bool,
    waiting: impl FnOnce(),
) -> Result<String, String> {
    let _lock = queue::lock(root, waiting)?;
    let since = Since::read(root, main)?;
    let Some(version) = since.version(chosen)? else {
        return Ok(since.nothing());
    };
    release(root, main, version, push)?;
    Ok(format!("Released {}", version.tag()))
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

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn the_rules_say_how_versions_are_released_and_whether_main_is_pushed() {
        assert_eq!(Rules::from_toml(""), Ok(Rules::default()));
        assert_eq!(Rules::default().mode, Mode::Auto);
        assert!(!Rules::default().push);
        let manual = "push_main = true\n[versions]\nmode = \"manual\"\n[limits]\nx = 1\n";
        let rules = Rules::from_toml(manual).unwrap();
        assert_eq!((rules.mode, rules.push), (Mode::Manual, true));
        let auto = Rules::from_toml("[versions]\nmode = \"auto\"\n").unwrap();
        assert_eq!(auto.mode, Mode::Auto);
        for wrong in [
            "[versions]\nmode = \"weekly\"\n",
            "[versions]\nmode = \"Manual\"\n",
            "[versions]\nmode = 1\n",
            "push_main = \"yes\"\n",
            "[versions",
        ] {
            let err = Rules::from_toml(wrong).unwrap_err();
            assert!(err.starts_with("gdd/rules.toml: "), "{wrong:?}: {err}");
        }
        let err = Rules::from_toml("[versions]\nmode = \"weekly\"\n").unwrap_err();
        assert!(err.contains("weekly") && err.contains("manual"), "{err}");
    }

    #[test]
    fn a_version_is_three_numbers_with_or_without_a_v() {
        for (text, expected) in [
            ("1.2.3", version(1, 2, 3)),
            ("v0.1.0", version(0, 1, 0)),
            ("10.20.30", version(10, 20, 30)),
        ] {
            assert_eq!(Version::parse(text), Some(expected), "{text}");
        }
        for wrong in [
            "",
            "v",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.03",
            "1.2.3-rc.1",
            "1.2.3+build",
            "vv1.2.3",
            "V1.2.3",
            " 1.2.3",
            "1.+2.3",
        ] {
            assert_eq!(Version::parse(wrong), None, "{wrong:?}");
        }
        assert_eq!(version(1, 2, 3).to_string(), "1.2.3");
        assert_eq!(version(1, 2, 3).tag(), "v1.2.3");
        assert_eq!(FIRST, version(0, 1, 0));
    }

    #[test]
    fn each_line_bumps_its_part_and_a_breaking_one_the_major_once_it_is_not_0() {
        let of = |section, breaking, major| bump(&line(section, "x", breaking), major);
        for section in ["Fixed", "Security"] {
            assert_eq!(of(section, false, 1), Bump::Patch, "{section}");
        }
        for section in ["Added", "Changed", "Deprecated", "Removed"] {
            assert_eq!(of(section, false, 1), Bump::Minor, "{section}");
        }
        for section in changelog::SECTIONS {
            assert_eq!(of(section, true, 1), Bump::Major, "{section}");
            assert_eq!(of(section, true, 0), Bump::Minor, "{section}");
            assert_eq!(of(section, true, 2), Bump::Major, "{section}");
        }
        assert!(Bump::Patch < Bump::Minor && Bump::Minor < Bump::Major);
    }

    #[test]
    fn a_bump_resets_what_follows_its_part() {
        let current = version(1, 2, 3);
        assert_eq!(current.bumped(Bump::Patch), version(1, 2, 4));
        assert_eq!(current.bumped(Bump::Minor), version(1, 3, 0));
        assert_eq!(current.bumped(Bump::Major), version(2, 0, 0));
        // The last number there is stays: its tag exists, and the release says so.
        let top = version(u64::MAX, u64::MAX, u64::MAX);
        assert_eq!(top.bumped(Bump::Patch), top);
        assert_eq!(top.bumped(Bump::Major), version(u64::MAX, 0, 0));
    }

    fn logged(commit: &str, lines: &[&str], breaking: &[&str]) -> Logged {
        let owned = |said: &[&str]| said.iter().map(|s| s.to_string()).collect();
        Logged {
            commit: commit.to_string(),
            day: "2026-10-28".to_string(),
            trailers: vec![owned(lines), owned(breaking)],
        }
    }

    fn tag(name: &str, commit: &str) -> (String, String) {
        (name.to_string(), commit.to_string())
    }

    /// A first-parent history, newest first: two lines since `c`, then one before it.
    fn history() -> [Logged; 5] {
        [
            logged("e", &["Fixed · Crash"], &["no"]),
            logged("d", &["Added · Export"], &["no"]),
            logged("c", &[], &[]),
            logged("b", &["Changed · Old"], &["yes"]),
            logged("a", &[], &[]),
        ]
    }

    fn next(history: &[Logged], tags: &[(String, String)]) -> Option<String> {
        Some(since(history, tags).next()?.to_string())
    }

    /// The tag of the newest version `found` names.
    fn newest(found: &Since) -> Option<&str> {
        found.newest.as_ref().map(|(tag, _)| tag.as_str())
    }

    #[test]
    fn the_next_version_takes_the_largest_bump_of_the_lines_since_the_newest_one() {
        let history = history();
        let tags = [tag("v1.2.3", "c"), tag("v1.1.0", "a")];
        let found = since(&history, &tags);
        assert_eq!(newest(&found), Some("v1.2.3"));
        let crash = line("Fixed", "Crash", false);
        assert_eq!(found.lines, [crash, line("Added", "Export", false)]);
        assert_eq!(next(&history, &tags).as_deref(), Some("1.3.0"));
        assert_eq!(next(&history[..1], &[tag("v1.2.3", "e")]), None);
        let fix_only = [history[0].clone(), logged("c", &[], &[])];
        assert_eq!(next(&fix_only, &tags).as_deref(), Some("1.2.4"));
        // A breaking line since: major, or minor while the major is 0.
        let after_a = |name| next(&history[2..], &[tag(name, "a")]);
        assert_eq!(after_a("v1.1.0").as_deref(), Some("2.0.0"));
        assert_eq!(after_a("v0.4.2").as_deref(), Some("0.5.0"));
    }

    #[test]
    fn before_the_first_version_every_line_counts_and_the_next_is_0_1_0() {
        let history = history();
        let found = since(&history, &[]);
        assert_eq!(found.newest, None);
        assert_eq!(found.lines.len(), 3);
        assert_eq!(found.next(), Some(FIRST));
        assert_eq!(next(&history[2..], &[]).as_deref(), Some("0.1.0"));
        assert_eq!(next(&[logged("a", &[], &[])], &[]), None);
        assert_eq!(next(&[], &[]), None);
    }

    #[test]
    fn the_newest_version_is_the_highest_tag_on_the_first_parents_of_main() {
        let history = history();
        // Pre-releases count by their precedence; a release comes after them.
        let tags = [tag("v1.0.0-rc.2", "c"), tag("v1.0.0-rc.10", "a")];
        let found = since(&history, &tags);
        assert_eq!(newest(&found), Some("v1.0.0-rc.10"));
        assert_eq!(found.lines.len(), 3);
        let tags = [tag("v1.0.0", "a"), tag("v1.0.0-rc.1", "c")];
        assert_eq!(next(&history, &tags).as_deref(), Some("2.0.0"));
        let found = since(&history, &[tag("v1.0.0-rc.1", "c")]);
        assert_eq!(found.next(), Some(version(1, 1, 0)));
        // Tags off the first parents, and tags that name no version, do not count.
        for other in ["v9.0.0", "release", "1.5.0", "v1.5", "vx.1.0"] {
            let tags = [tag("v1.2.3", "c"), tag(other, "z"), tag(other, "e")];
            let found = since(&history, &tags);
            if other == "v9.0.0" {
                assert_eq!(newest(&found), Some("v9.0.0"), "on e");
                assert!(found.lines.is_empty());
            } else {
                assert_eq!(newest(&found), Some("v1.2.3"), "{other}");
                assert_eq!(found.lines.len(), 2, "{other}");
            }
        }
        let off_main = since(&history, &[tag("v9.0.0", "z")]);
        assert_eq!(off_main.newest, None);
    }

    #[test]
    fn a_version_chosen_by_hand_must_be_one_and_come_after_the_newest() {
        let history = history();
        let found = since(&history, &[tag("v0.4.0", "c")]);
        let refused = Err("the version must come after v0.4.0".to_string());
        for early in ["0.3.9", "0.4.0", "v0.4.0"] {
            assert_eq!(found.version(Some(early)), refused, "{early}");
        }
        assert_eq!(found.version(Some("v1.0.0")), Ok(Some(version(1, 0, 0))));
        assert_eq!(found.version(Some("0.4.1")), Ok(Some(version(0, 4, 1))));
        assert_eq!(found.version(None), Ok(Some(version(0, 5, 0))));
        for wrong in ["1.0", "1.0.0-rc.1", "one"] {
            let err = found.version(Some(wrong)).unwrap_err();
            assert_eq!(err, format!("{wrong:?} {NOT_A_VERSION}"));
        }
        let not_a_version = "is not a version: MAJOR.MINOR.PATCH, such as 1.0.0";
        assert_eq!(NOT_A_VERSION, not_a_version);
        // A release comes after its own pre-release.
        let pre = since(&history, &[tag("v1.0.0-rc.1", "c")]);
        assert_eq!(pre.version(Some("1.0.0")), Ok(Some(version(1, 0, 0))));
        // Nothing to release: none, but a version that is not after the newest is still wrong.
        let nothing = since(&history[..1], &[tag("v0.4.0", "e")]);
        assert_eq!(nothing.version(Some("1.0.0")), Ok(None));
        assert_eq!(nothing.version(None), Ok(None));
        assert!(nothing.version(Some("0.1.0")).is_err());
        let first = since(&history, &[]);
        assert_eq!(first.version(Some("0.0.1")), Ok(Some(version(0, 0, 1))));
    }

    #[test]
    fn gdd_release_shows_the_next_version_and_its_lines_by_section() {
        let history = [
            logged("e", &["Fixed · Crash"], &["no"]),
            logged("d", &["Changed · One file"], &["yes"]),
            logged("c", &["Added · Export"], &["no"]),
            logged("b", &["Added · Import"], &["no"]),
            logged("a", &[], &[]),
        ];
        let found = since(&history, &[tag("v0.1.0", "a")]);
        let next = found.next().unwrap();
        let expected = "Next version: v0.2.0 (from v0.1.0)\n\n### Added\n\n- Export\n- Import\n\n\
                        ### Changed\n\n- **Breaking:** One file\n\n### Fixed\n\n- Crash\n\n\
                        Nothing released: gdd release --yes releases it.";
        assert_eq!(found.shown(next), expected);
        let offered = found.offered(next);
        let start = "Next version: v0.2.0 (from v0.1.0)\n\n### Added\n\n";
        assert!(offered.starts_with(start), "{offered}");
        let end = "- Crash\n\nRelease v0.2.0? [y/N] ";
        assert!(offered.ends_with(end), "{offered}");
        let first = since(&history[..1], &[]);
        let shown = first.shown(FIRST);
        let start = "Next version: v0.1.0 (first version)\n\n### Fixed\n\n- Crash\n\n";
        assert!(shown.starts_with(start), "{shown}");
        assert_eq!(found.nothing(), "Nothing to release since v0.1.0");
        assert_eq!(first.nothing(), NOTHING_YET);
        let nothing_yet = "Nothing to release: no change on main carries a changelog line";
        assert_eq!(NOTHING_YET, nothing_yet);
    }

    #[test]
    fn only_a_yes_releases() {
        for answer in ["y", "Y\n", "yes", " YES \n"] {
            assert!(yes(answer), "{answer:?}");
        }
        for answer in ["", "\n", "n", "no", "yep", "yes please", "N"] {
            assert!(!yes(answer), "{answer:?}");
        }
    }

    /// A repository with an author of its own and the record of change `c1`, a Feature, with
    /// `field` among its fields when it is not empty.
    fn with_change(field: &str) -> Repo {
        let repo = Repo::new();
        repo.identify();
        let field = if field.is_empty() {
            String::new()
        } else {
            format!(", {field}")
        };
        let record = format!(
            r#"{{"id": "c1", "title": "App", "type": "Feature", "state": "in-pre", "gates": {{}},
                "inbox": []{field}}}"#
        );
        repo.write(".gdd/changes/c1/change.json", &record);
        repo.write(".gitignore", ".gdd/\n");
        repo.commit();
        repo
    }

    /// The merge commit of a change on main, carrying `trailers`.
    fn merge(repo: &Repo, trailers: &str) {
        let message = format!("gdd(c1): G3 Merge Gate · App\n\nRecord: x\n\n{trailers}");
        repo.git(&["commit", "-q", "--allow-empty", "-m", &message]);
    }

    fn has_tag(repo: &Repo, tag: &str) -> bool {
        repo.git(&["tag", "--list", tag]) == tag
    }

    fn named(repo: &Repo, tag: &str) -> String {
        repo.git(&["rev-parse", &format!("{tag}^{{commit}}")])
    }

    fn head(repo: &Repo) -> String {
        repo.git(&["rev-parse", "main"])
    }

    const FIXED: &str = "Changelog: Fixed · Crash\nBreaking: no";

    #[test]
    fn without_a_changelog_the_tag_names_the_head_of_main() {
        let repo = with_change("");
        merge(&repo, FIXED);
        // Pushing without a remote pushes nothing.
        release(repo.root(), "main", FIRST, true).unwrap();
        assert_eq!(named(&repo, "v0.1.0"), head(&repo));
        assert_eq!(repo.git(&["cat-file", "-t", "v0.1.0"]), "tag");
        let subject = ["tag", "-l", "--format=%(contents:subject)", "v0.1.0"];
        assert_eq!(repo.git(&subject), "Release v0.1.0");
        let last = repo.git(&["log", "-1", "--format=%s", "main"]);
        assert_eq!(last, "gdd(c1): G3 Merge Gate · App");
        // Off main, without a changelog, the tag still names the head of main.
        let main = head(&repo);
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.commit();
        release(repo.root(), "main", version(0, 2, 0), false).unwrap();
        assert_eq!(named(&repo, "v0.2.0"), main);
    }

    #[test]
    fn a_changelog_of_main_is_released_in_a_commit_of_its_own_which_the_tag_names() {
        let repo = with_change("");
        let ours = "# Changelog\n\nOur words.\n\n## [Unreleased]\n";
        repo.write(changelog::PATH, ours);
        repo.commit();
        merge(&repo, FIXED);
        repo.write("loose.txt", "not released\n");
        release(repo.root(), "main", FIRST, false).unwrap();
        assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Release v0.1.0");
        let files = repo.git(&["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(files, "CHANGELOG.md");
        assert_eq!(named(&repo, "v0.1.0"), head(&repo));
        assert_eq!(repo.git(&["status", "--porcelain"]), "?? loose.txt");
        let text = fs::read_to_string(repo.root().join(changelog::PATH)).unwrap();
        let day = exceptions::today();
        let start = format!(
            "# Changelog\n\nOur words.\n\n## [Unreleased]\n\n## [0.1.0] - {day}\n\n### Fixed\n\n\
             - Crash <!-- gdd: "
        );
        assert!(text.starts_with(&start), "{text}");
    }

    #[test]
    fn a_release_never_moves_a_tag_nor_commits_what_main_does_not_have() {
        let repo = with_change("");
        repo.git(&["tag", "v0.1.0"]);
        let before = head(&repo);
        let err = release(repo.root(), "main", FIRST, false).unwrap_err();
        assert_eq!(
            err,
            "the tag v0.1.0 exists already: gdd never moves or deletes a tag"
        );
        assert_eq!(named(&repo, "v0.1.0"), before);
        repo.write(changelog::PATH, "# Changelog\n");
        repo.commit();
        merge(&repo, FIXED);
        let main = head(&repo);
        let next = version(0, 2, 0);
        let edited = "# Changelog\n\nAn edit nobody committed\n";
        repo.write(changelog::PATH, edited);
        let err = release(repo.root(), "main", next, false).unwrap_err();
        let said = "CHANGELOG.md has changes main does not have: commit them, or put them aside, \
                    before the release";
        assert_eq!(err, said);
        let file = repo.root().join(changelog::PATH);
        fs::remove_file(&file).unwrap();
        let err = release(repo.root(), "main", next, false).unwrap_err();
        assert_eq!(err, said);
        let outside = Folder::new();
        outside.write("notes.md", "# Changelog\n");
        symlink(outside.0.join("notes.md"), &file).unwrap();
        let err = release(repo.root(), "main", next, false).unwrap_err();
        let start = "cannot read CHANGELOG.md: not a file of its own";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(&file).unwrap();
        repo.git(&["checkout", "-q", "--", changelog::PATH]);
        repo.git(&["checkout", "-q", "-b", "other"]);
        let err = release(repo.root(), "main", next, false).unwrap_err();
        let start = "the release commits CHANGELOG.md on main, and ";
        assert!(err.starts_with(start), "{err}");
        let end = " is on other: check out main there first";
        assert!(err.ends_with(end), "{err}");
        assert!(!has_tag(&repo, "v0.2.0"));
        assert_eq!(head(&repo), main);
        let err = release(repo.root(), "trunk", next, false).unwrap_err();
        assert!(err.starts_with("git ls-tree "), "{err}");
    }

    #[test]
    fn a_release_never_overwrites_what_the_index_holds_of_the_changelog() {
        let repo = with_change("");
        let ours = "# Changelog\n\n## [Unreleased]\n";
        repo.write(changelog::PATH, ours);
        repo.commit();
        merge(&repo, FIXED);
        let main = head(&repo);
        // An edit added to the index, then the file put back as main has it.
        let edited = "# Changelog\n\nAn edit in the index\n";
        repo.write(changelog::PATH, edited);
        repo.git(&["add", "--", changelog::PATH]);
        repo.write(changelog::PATH, ours);
        let err = release(repo.root(), "main", FIRST, false).unwrap_err();
        let said = "CHANGELOG.md has changes main does not have: commit them, or put them aside, \
                    before the release";
        assert_eq!(err, said);
        assert!(!has_tag(&repo, "v0.1.0"));
        assert_eq!(head(&repo), main);
        let index = repo.git(&["show", &format!(":{}", changelog::PATH)]);
        assert_eq!(index, edited.trim_end());
        // Once the index holds what main has again, it is released.
        repo.git(&["reset", "-q", "--", changelog::PATH]);
        release(repo.root(), "main", FIRST, false).unwrap();
        assert_eq!(named(&repo, "v0.1.0"), head(&repo));
    }

    #[test]
    fn with_push_main_and_the_tag_go_to_the_remote() {
        let repo = with_change("");
        repo.write(changelog::PATH, "# Changelog\n\n## [Unreleased]\n");
        repo.commit();
        merge(&repo, FIXED);
        let remote = Folder::new();
        let path = remote.0.to_string_lossy().into_owned();
        repo.git(&["init", "-q", "--bare", &path]);
        repo.git(&["remote", "add", "origin", &path]);
        let pushed = |name: &str| Git::new(&remote.0).exists(name).unwrap();
        release(repo.root(), "main", FIRST, false).unwrap();
        assert!(!pushed("main") && !pushed("refs/tags/v0.1.0"));
        merge(&repo, FIXED);
        release(repo.root(), "main", version(0, 1, 1), true).unwrap();
        assert!(pushed("refs/tags/v0.1.1"));
        let remote_main = Git::new(&remote.0).merge_base("main", "main").unwrap();
        assert_eq!(remote_main, head(&repo));
        assert_eq!(named(&repo, "v0.1.1"), remote_main);
    }

    fn steps(promote: &str) -> release::Steps {
        let text = format!("[release]\nsimulated = true\npromote = {promote:?}\n");
        release::Steps::from_toml(&text, crate::exec::TIMEOUT).unwrap()
    }

    fn promotion<'a>(repo: &'a Repo, steps: &'a release::Steps, rules: Rules) -> Promotion<'a> {
        Promotion {
            root: repo.root(),
            state: repo.root(),
            main: "main",
            id: "c1",
            steps,
            needs_g4: false,
            rules,
        }
    }

    fn record(repo: &Repo) -> Value {
        let text = change::read_record(repo.root(), "c1").unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn log_lines(repo: &Repo) -> Vec<log::Record> {
        let text = fs::read_to_string(repo.root().join(log::PATH)).unwrap_or_default();
        log::parse(&text).unwrap()
    }

    const FIX_LINE: &str = r#""changelog": {"section": "Fixed", "line": "Crash"}"#;

    #[test]
    fn a_promotion_gets_its_version_and_releases_it_once_it_passed() {
        let repo = with_change(FIX_LINE);
        repo.git(&["tag", "v1.2.3"]);
        merge(&repo, FIXED);
        let mut said = Vec::new();
        let failing = steps("exit 3");
        let promoted = promote(&promotion(&repo, &failing, Rules::default()), |line| {
            said.push(line.to_string())
        });
        let err = promoted.unwrap_err();
        assert_eq!(err, "release.promote failed (exit code 3)");
        assert!(!has_tag(&repo, "v1.2.4"));
        assert_eq!(record(&repo)["version"], Value::Null);
        // The retry takes the same version: nothing was tagged.
        let seen = steps("test \"$GDD_VERSION\" = 1.2.4");
        let promoted = promote(&promotion(&repo, &seen, Rules::default()), |line| {
            said.push(line.to_string())
        });
        assert!(promoted.unwrap().contains(r#""name":"promote""#));
        assert_eq!(said, ["Released v1.2.4"]);
        assert_eq!(named(&repo, "v1.2.4"), head(&repo));
        let record = record(&repo);
        assert_eq!(record["version"], "1.2.4");
        assert_eq!(record["state"], "in-production");
        let log = log_lines(&repo);
        let last = log.last().unwrap();
        let fields = ["change", "phase", "gate", "role", "agent", "result", "note"];
        let expected = ["c1", "4-release", "-", "machine", "-", "released", "v1.2.4"];
        let values = fields.map(|field| last.text(field));
        assert_eq!(values, expected.map(String::from));
        assert_eq!(last.duration_s, 0.0);
    }

    #[test]
    fn manual_mode_a_change_left_out_and_nothing_new_promote_without_a_version() {
        let unversioned = steps("test -z \"${GDD_VERSION+set}\"");
        let manual = Rules {
            mode: Mode::Manual,
            push: false,
        };
        let none = r#""changelog": {"none": true}"#;
        for (field, rules) in [(FIX_LINE, manual), (none, Rules::default())] {
            let repo = with_change(field);
            merge(&repo, FIXED);
            let mut said = Vec::new();
            let at = promotion(&repo, &unversioned, rules);
            promote(&at, |line| said.push(line.to_string())).unwrap();
            assert!(said.is_empty(), "{said:?}");
            assert_eq!(repo.git(&["tag", "--list"]), "", "{field}");
            assert_eq!(record(&repo)["version"], Value::Null);
            assert_eq!(record(&repo)["state"], "in-production");
        }
        // Its line was released already, with the newest version.
        let repo = with_change(FIX_LINE);
        merge(&repo, FIXED);
        repo.git(&["tag", "v0.1.0"]);
        let at = promotion(&repo, &unversioned, Rules::default());
        promote(&at, |_| {}).unwrap();
        assert_eq!(repo.git(&["tag", "--list"]), "v0.1.0");
    }

    #[test]
    fn a_version_that_cannot_be_released_is_not_promoted() {
        let repo = with_change(FIX_LINE);
        merge(&repo, FIXED);
        // The tag of the next version, on a commit off main.
        repo.git(&["checkout", "-q", "-b", "other"]);
        repo.commit();
        repo.git(&["tag", "v0.1.0"]);
        repo.git(&["checkout", "-q", "main"]);
        let touching = steps("touch promoted");
        let at = promotion(&repo, &touching, Rules::default());
        let err = promote(&at, |_| {}).unwrap_err();
        assert_eq!(
            err,
            "the tag v0.1.0 exists already: gdd never moves or deletes a tag"
        );
        assert!(!repo.root().join("promoted").exists());
        assert_eq!(record(&repo)["state"], "in-pre");
        // A record that cannot be read, nor a changelog line in it: nothing runs either.
        repo.write(".gdd/changes/c1/change.json", "[]");
        let err = promote(&at, |_| {}).unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/change.json: "), "{err}");
        repo.write(".gdd/changes/c1/change.json", r#"{"changelog": "Fixed"}"#);
        assert!(promote(&at, |_| {}).is_err());
        assert!(!repo.root().join("promoted").exists());
    }

    #[test]
    fn a_release_that_fails_after_the_promotion_says_production_has_it() {
        let repo = with_change(FIX_LINE);
        repo.write(changelog::PATH, "# Changelog\n");
        repo.commit();
        merge(&repo, FIXED);
        // Someone edits the changelog while the promotion runs.
        let editing = steps(&format!("echo edit >> {}", changelog::PATH));
        let at = promotion(&repo, &editing, Rules::default());
        let mut said = Vec::new();
        let err = promote(&at, |line| said.push(line.to_string())).unwrap_err();
        let start = "Promoted, but v0.1.0 was not released: CHANGELOG.md has changes";
        assert!(err.starts_with(start), "{err}");
        assert!(said.is_empty());
        assert_eq!(record(&repo)["state"], "in-production");
        assert!(!has_tag(&repo, "v0.1.0"));
    }

    #[test]
    fn a_promotion_waits_for_the_merge_lock_and_says_so() {
        let repo = with_change(FIX_LINE);
        merge(&repo, FIXED);
        let held = queue::lock(repo.root(), || {}).unwrap();
        let root = repo.root().to_path_buf();
        let waited = std::thread::spawn(move || {
            let steps = steps("true");
            let at = Promotion {
                root: &root,
                state: &root,
                main: "main",
                id: "c1",
                steps: &steps,
                needs_g4: false,
                rules: Rules::default(),
            };
            let mut said = Vec::new();
            promote(&at, |line| said.push(line.to_string())).map(|_| said)
        });
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!has_tag(&repo, "v0.1.0"));
        drop(held);
        let said = waited.join().unwrap().unwrap();
        assert_eq!(said, [queue::WAITING, "Released v0.1.0"]);
    }

    #[test]
    fn by_hand_releases_what_is_merged_or_says_nothing_is_new() {
        let repo = with_change("");
        let mut waited = false;
        let nothing = by_hand(repo.root(), "main", None, false, || waited = true);
        assert_eq!(nothing.unwrap(), NOTHING_YET);
        assert!(!waited);
        merge(&repo, FIXED);
        let err = by_hand(repo.root(), "main", Some("1.0"), false, || {}).unwrap_err();
        assert!(err.starts_with("\"1.0\" is not a version"), "{err}");
        let released = by_hand(repo.root(), "main", None, false, || {});
        assert_eq!(released.unwrap(), "Released v0.1.0");
        assert_eq!(named(&repo, "v0.1.0"), head(&repo));
        let again = by_hand(repo.root(), "main", None, false, || {});
        assert_eq!(again.unwrap(), "Nothing to release since v0.1.0");
        merge(&repo, "Changelog: Added · Export\nBreaking: no");
        let err = by_hand(repo.root(), "main", Some("0.1.0"), false, || {}).unwrap_err();
        assert_eq!(err, "the version must come after v0.1.0");
        let chosen = by_hand(repo.root(), "main", Some("v1.0.0"), false, || {});
        assert_eq!(chosen.unwrap(), "Released v1.0.0");
        assert!(has_tag(&repo, "v1.0.0") && !has_tag(&repo, "v0.2.0"));
        let err = by_hand(repo.root(), "trunk", None, false, || {}).unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
    }

    #[test]
    fn only_a_yes_to_the_question_releases_the_version_it_offered() {
        let repo = with_change("");
        merge(&repo, FIXED);
        for answer in ["n\n", "", "yes please\n"] {
            let said = answered(repo.root(), "main", FIRST, false, answer, || {});
            assert_eq!(said.unwrap(), "Nothing released.", "{answer:?}");
        }
        assert_eq!(repo.git(&["tag", "--list"]), "");
        let said = answered(repo.root(), "main", version(0, 2, 0), false, "Y\n", || {});
        assert_eq!(said.unwrap(), "Released v0.2.0");
        assert_eq!(named(&repo, "v0.2.0"), head(&repo));
    }

    #[test]
    fn what_cannot_be_read_from_git_is_an_error() {
        let plain = Folder::new();
        let err = Since::read(&plain.0, "main").unwrap_err();
        assert!(err.starts_with("git log "), "{err}");
        let err = release(&plain.0, "main", FIRST, false).unwrap_err();
        assert!(err.starts_with("git rev-parse "), "{err}");
    }
}
