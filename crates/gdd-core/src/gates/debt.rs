//! The known debt of a project: the gate commands that failed when `gdd adopt` first ran them on
//! the code the project already had (see [`crate::adopt`]), in `gdd/debt.toml`. A gate still runs
//! a command with known debt, but its failure does not fail the gate: the report keeps it apart.
//! Once a change makes the command pass, the gate names it as paid, and the merge of that change
//! takes it off the list, so from then on it blocks. Only `gdd adopt` writes the list, once; the
//! merge only takes from it.
//!
//! When `[count]` of `gdd/commands.toml` says how to count what a command reports ([`count`]),
//! its debt has a count, a ceiling that turns one way only: a change that reports more fails the
//! gate ([`held`]), and one that reports less lowers it when it merges ([`lowered`]).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agents::files;
use crate::config;
use crate::gates::{Gate, Report};

/// The file of the known debt, from the project root.
pub const PATH: &str = "gdd/debt.toml";

/// The name of the check that fails a gate when the known debt cannot be read.
pub const CHECK: &str = "debt";

/// What stands for a number in a text of `[count]`.
pub const NUMBER: &str = "{n}";

/// What the screens say of known debt without a count.
pub const NO_COUNT: &str = "no count";

/// What the file of the known debt says first: what it is and how it empties.
const HEADER: &str = "# Known debt recorded by gdd adopt. These commands run and warn but do not \
                      block,\n# until a change makes each pass; the merge then takes it off this \
                      list for good.\n# A count is a ceiling: a change that reports more fails \
                      the gate, and one that\n# reports less lowers it when it merges.\n";

/// The kinds of command, riskiest first: the part of a name before its first `-`.
const RISK: [&str; 8] = [
    "security",
    "dependencies",
    "types",
    "unit",
    "architecture",
    "quality",
    "lint",
    "lockfile",
];

/// What the name of a command starts with when it judges the change or its scenarios, not the
/// code that was there before: it never becomes debt.
const NEVER_DEBT: [&str; 3] = ["acceptance", "mutation", "secrets"];

/// The known debt of a project, as `gdd/debt.toml` holds it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Known {
    /// The day `gdd adopt` recorded it.
    pub adopted: String,
    /// The commands with known debt.
    #[serde(default)]
    pub debt: Vec<Debt>,
}

/// A gate command with known debt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Debt {
    /// Its name in `gdd/commands.toml`.
    pub command: String,
    /// The gate whose section has it.
    pub gate: Gate,
    /// The day it was recorded.
    pub since: String,
    /// The count of what it reported (see [`count`]), when `[count]` could count it: its
    /// ceiling.
    #[serde(default)]
    pub count: Option<u64>,
}

/// A command with known debt that failed in a gate: its name and what its output keeps (see
/// [`super::kept`]); when its debt has a count, the count it reported and the one recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owed {
    /// The name of the command.
    pub name: String,
    /// What to read.
    pub tail: String,
    /// The count of what it reported, when its debt has one recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// The count recorded for its debt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known: Option<u64>,
}

/// What a gate holds a command with known debt to: the count recorded for its debt, when there
/// is one, and its texts in `[count]`, when it has them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ceiling<'a> {
    /// The count recorded for its debt.
    pub(crate) known: Option<u64>,
    /// Its texts in `[count]`.
    pub(crate) texts: Option<&'a [String]>,
}

/// A recorded count a change lowers: the command and the gate of the debt, the count recorded
/// and the one the latest report of that gate saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lowered {
    /// The name of the command.
    pub command: String,
    /// The gate whose section has it.
    pub gate: Gate,
    /// The count recorded.
    pub from: u64,
    /// The count the change reported.
    pub to: u64,
}

impl fmt::Display for Lowered {
    /// `<command> from <from> to <to>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} from {} to {}", self.command, self.from, self.to)
    }
}

/// The known debt of one command in each gate that has it, as the screens list it (see
/// [`listed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed<'a> {
    /// The name of the command.
    pub command: &'a str,
    /// The day its debt of the first gate was recorded.
    pub since: &'a str,
    /// The count recorded for its debt in each gate, G2 first.
    pub counts: Vec<(Gate, Option<u64>)>,
}

impl Listed<'_> {
    /// What its counts say: the count, or [`NO_COUNT`], when every gate says the same; else
    /// each with its gate, `5 in G2, no count in G3`.
    pub fn said(&self) -> String {
        let mut alike: Vec<Option<u64>> = self.counts.iter().map(|&(_, count)| count).collect();
        alike.dedup();
        if let [count] = alike.as_slice() {
            return count_said(*count);
        }
        let mut each = Vec::new();
        for (gate, count) in &self.counts {
            each.push(format!("{} in {gate}", count_said(*count)));
        }
        each.join(", ")
    }

    /// Whether some gate holds its debt to no count: a change can add to it unseen there.
    pub fn uncounted(&self) -> bool {
        self.counts.iter().any(|(_, count)| count.is_none())
    }
}

/// What the count of a debt says: the count, or [`NO_COUNT`] without one.
fn count_said(count: Option<u64>) -> String {
    match count {
        Some(count) => count.to_string(),
        None => NO_COUNT.to_string(),
    }
}

/// What the latest G2 and G3 of a change said of the known debt, each with the gate that said it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reported {
    /// The commands with known debt still failing.
    pub owed: Vec<(Gate, Owed)>,
    /// The commands with known debt that passed.
    pub paid: Vec<(Gate, String)>,
}

impl Known {
    /// The text of `gdd/debt.toml` for it: what the file is, the day of the adoption, then each
    /// debt.
    pub fn to_toml(&self) -> String {
        let mut text = format!("{HEADER}adopted = {}\n", quoted(&self.adopted));
        for debt in &self.debt {
            text.push_str(&format!(
                "\n[[debt]]\ncommand = {}\ngate = {}\nsince = {}\n",
                quoted(&debt.command),
                quoted(debt.gate.key()),
                quoted(&debt.since)
            ));
            if let Some(count) = debt.count {
                text.push_str(&format!("count = {count}\n"));
            }
        }
        text
    }
}

/// The count of `output`, what a command reported, by its texts in `[count]`, `texts`: the sum,
/// over the texts, of the numbers at `{n}` where a text with it matches (see [`numbers`]), and
/// of the lines that contain a text without it.
pub fn count(output: &str, texts: &[String]) -> u64 {
    let counted = texts.iter().map(|text| of_text(output, text));
    counted.fold(0, u64::saturating_add)
}

/// The count of `output` by the one text `text` (see [`count`]).
fn of_text(output: &str, text: &str) -> u64 {
    match text.split_once(NUMBER) {
        Some((before, after)) => numbers(output, before, after),
        None => lines_with(output, text),
    }
}

/// The sum of the numbers of each place of `output` where `before`, one or more ASCII digits
/// and `after` come one after the other. Places do not overlap; a number too large for `u64`
/// skips its place.
fn numbers(output: &str, before: &str, after: &str) -> u64 {
    let mut sum: u64 = 0;
    // Where the next place may start: the end of the last one.
    let mut free = 0;
    for (at, _) in output.char_indices() {
        if at < free {
            continue;
        }
        let Some(from) = output[at..].strip_prefix(before) else {
            continue;
        };
        let Some((number, tail)) = number_then(from, after) else {
            continue;
        };
        sum = sum.saturating_add(number.parse().unwrap_or(0));
        free = output.len() - tail.len();
    }
    sum
}

/// The number `text` starts with when `after` follows it, and what comes after `after`: the most
/// ASCII digits, one or more, that leave `after` next, so an `after` that starts with a digit
/// keeps it. `None` without.
fn number_then<'a>(text: &'a str, after: &str) -> Option<(&'a str, &'a str)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    (1..=digits).rev().find_map(|end| {
        let tail = text[end..].strip_prefix(after)?;
        Some((&text[..end], tail))
    })
}

/// How many lines of `output` contain `text`.
fn lines_with(output: &str, text: &str) -> u64 {
    let found = output.lines().filter(|line| line.contains(text)).count();
    u64::try_from(found).unwrap_or(u64::MAX)
}

/// How the command `name`, with known debt held to `ceiling`, stands once it failed with
/// `output`: the count it reported, when its debt has one recorded; `None` without. Reporting
/// more than the count recorded fails the gate, and so does a count that cannot be read:
/// `[count]` has no texts for it, or none matches. Why it fails, then.
pub(crate) fn held(name: &str, ceiling: Ceiling, output: &str) -> Result<Option<u64>, String> {
    let Some(known) = ceiling.known else {
        return Ok(None);
    };
    let now = ceiling.texts.map_or(0, |texts| count(output, texts));
    if now == 0 {
        return Err(format!(
            "{name} failed and gdd cannot count what it reported (its [count] texts match nothing)"
        ));
    }
    if now > known {
        let more = now - known;
        return Err(format!(
            "{name} reports {now}, the known debt is {known}: {more} more"
        ));
    }
    Ok(Some(now))
}

/// `text` as a TOML string.
fn quoted(text: &str) -> String {
    toml::Value::from(text).to_string()
}

/// Whether the gate command `name` can become known debt: it does not judge the change or its
/// scenarios.
pub fn can_owe(name: &str) -> bool {
    !NEVER_DEBT.iter().any(|start| name.starts_with(start))
}

/// The risk of the command `name`, to sort by: the place of its kind in [`RISK`] (a kind not
/// there goes after them), then the name.
pub fn risk(name: &str) -> (usize, &str) {
    let kind = name.split_once('-').map_or(name, |(kind, _)| kind);
    let place = RISK.iter().position(|known| *known == kind);
    (place.unwrap_or(RISK.len()), name)
}

/// `names` riskiest first (see [`risk`]), each once.
pub fn by_risk<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut names: Vec<&str> = names.into_iter().collect();
    names.sort_by_key(|&name| risk(name));
    names.dedup();
    names
}

/// `debts` riskiest first (see [`risk`]), one per command name with the count of each of its
/// gates, G2 first, and the day of the first.
pub fn listed(debts: &[Debt]) -> Vec<Listed<'_>> {
    let mut sorted: Vec<&Debt> = debts.iter().collect();
    sorted.sort_by_key(|debt| (risk(&debt.command), debt.gate.key()));
    let mut listed: Vec<Listed> = Vec::new();
    for debt in sorted {
        let counted = (debt.gate, debt.count);
        match listed.last_mut() {
            Some(last) if last.command == debt.command => last.counts.push(counted),
            _ => listed.push(Listed {
                command: &debt.command,
                since: &debt.since,
                counts: vec![counted],
            }),
        }
    }
    listed
}

/// The known debt of the project at `root`; `None` without the file. A file that cannot be
/// read (a link, a folder, a path through a file), that is not shaped as known debt, or that
/// names a command that can never become debt (see [`can_owe`]) is an error that names it.
pub fn read(root: &Path) -> Result<Option<Known>, String> {
    let bytes = files::read(root, PATH).map_err(|e| format!("{PATH}: cannot read: {e}"))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&bytes);
    let known: Known = toml::from_str(&text)
        .map_err(|e| format!("{PATH}: {}", config::Error::Toml(e.to_string())))?;
    if let Some(debt) = known.debt.iter().find(|debt| !can_owe(&debt.command)) {
        return Err(format!(
            "{PATH}: {:?} can never be known debt: it judges the change or its scenarios, so it \
             always blocks. Take it off the list.",
            debt.command
        ));
    }
    Ok(Some(known))
}

/// Writes `bytes` to the file `path` under `root`, a new file (see [`files::replace`]): a link
/// on the way, or a file there with other names, is refused first (see [`files::read`]).
pub(crate) fn write_new(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    let cannot = |e: std::io::Error| format!("cannot write {path}: {e}");
    files::read(root, path).map_err(cannot)?;
    files::replace(root, path, bytes).map_err(cannot)
}

/// The commands of `gate` with known debt in the project at `root`, by name, each with the count
/// recorded for its debt, when there is one. When the file cannot be read there are none, and
/// `report` fails the check `debt` with why.
pub(crate) fn owed(report: &mut Report, root: &Path, gate: Gate) -> BTreeMap<String, Option<u64>> {
    of_gate(root, gate).unwrap_or_else(|why| {
        report.fail(CHECK, why);
        BTreeMap::new()
    })
}

/// The commands of `gate` with known debt in the project at `root`, by name, each with the count
/// recorded for its debt, when there is one; why, when the file cannot be read (see [`read`]).
pub(crate) fn of_gate(root: &Path, gate: Gate) -> Result<BTreeMap<String, Option<u64>>, String> {
    let debts = read(root)?.into_iter().flat_map(|known| known.debt);
    let of_gate = debts.filter(|debt| debt.gate == gate);
    Ok(of_gate.map(|debt| (debt.command, debt.count)).collect())
}

/// What the latest G2 and G3 of change `id`, under the state root `state`, said of the known
/// debt: nothing from a gate that has not run.
pub fn reported(state: &Path, id: &str) -> Result<Reported, String> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        known_debt: Vec<Owed>,
        #[serde(default)]
        paid_debt: Vec<String>,
    }
    let mut reported = Reported::default();
    for gate in [Gate::G2, Gate::G3] {
        let path = gate.report_path(id);
        let bytes = files::read(state, &path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let Some(bytes) = bytes else {
            continue;
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let owed = saved.known_debt.into_iter().map(|owed| (gate, owed));
        reported.owed.extend(owed);
        let paid = saved.paid_debt.into_iter().map(|name| (gate, name));
        reported.paid.extend(paid);
    }
    Ok(reported)
}

/// The recorded counts of the known debt of the project at `root` that `owed`, what the latest
/// reports of a change said of the known debt still failing, lowers: each debt whose gate's
/// report saved a count below the one recorded, in the order of the reports. Without a count in
/// them, the file is not read.
pub fn lowered(root: &Path, owed: &[(Gate, Owed)]) -> Result<Vec<Lowered>, String> {
    if owed.iter().all(|(_, owed)| owed.count.is_none()) {
        return Ok(Vec::new());
    }
    let debts = read(root)?.map(|known| known.debt).unwrap_or_default();
    let lowered = owed.iter().filter_map(|(gate, owed)| {
        let to = owed.count?;
        let same = |debt: &&Debt| debt.gate == *gate && debt.command == owed.name;
        let from = debts.iter().find(same)?.count.filter(|&from| to < from)?;
        Some(Lowered {
            command: owed.name.clone(),
            gate: *gate,
            from,
            to,
        })
    });
    Ok(lowered.collect())
}

/// Takes the commands `paid`, each with its gate, off the known debt of the project at `root`,
/// and lowers the counts `lowered` names, never raising one: the file is written again, or
/// removed when no debt is left. Nothing without the file.
pub fn pay(root: &Path, paid: &[(Gate, String)], lowered: &[Lowered]) -> Result<(), String> {
    let Some(mut known) = read(root)? else {
        return Ok(());
    };
    let owed = |debt: &Debt| !paid.contains(&(debt.gate, debt.command.clone()));
    known.debt.retain(owed);
    for debt in &mut known.debt {
        let same = |lower: &&Lowered| lower.gate == debt.gate && lower.command == debt.command;
        if let Some(lower) = lowered.iter().find(same) {
            debt.count = debt.count.map(|count| count.min(lower.to));
        }
    }
    if known.debt.is_empty() {
        files::remove(root, PATH).map_err(|e| format!("cannot remove {PATH}: {e}"))
    } else {
        write_new(root, PATH, known.to_toml().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Color;
    use crate::git::tests::Folder;
    use std::fs;
    use std::os::unix::fs::symlink;

    const KNOWN: &str = "adopted = \"2026-10-27\"\n\n\
                         [[debt]]\ncommand = \"security\"\ngate = \"g3\"\nsince = \"2026-10-27\"\n\n\
                         [[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"2026-10-28\"\n";

    fn debt(command: &str, gate: Gate, since: &str) -> Debt {
        Debt {
            command: command.to_string(),
            gate,
            since: since.to_string(),
            count: None,
        }
    }

    fn counted(command: &str, gate: Gate, count: u64) -> Debt {
        Debt {
            count: Some(count),
            ..debt(command, gate, "2026-10-27")
        }
    }

    fn owing(name: &str, count: Option<u64>) -> Owed {
        Owed {
            name: name.to_string(),
            tail: String::new(),
            count,
            known: None,
        }
    }

    fn texts(list: &[&str]) -> Vec<String> {
        list.iter().map(|text| text.to_string()).collect()
    }

    fn report() -> Report {
        Report::new(Gate::G2, Color::Yellow)
    }

    fn paid(list: &[(Gate, &str)]) -> Vec<(Gate, String)> {
        list.iter().map(|(g, n)| (*g, n.to_string())).collect()
    }

    #[test]
    fn the_known_debt_is_read_from_its_file() {
        let folder = Folder::new();
        assert_eq!(read(&folder.0), Ok(None));
        folder.write(PATH, KNOWN);
        let expected = Known {
            adopted: "2026-10-27".into(),
            debt: vec![
                debt("security", Gate::G3, "2026-10-27"),
                debt("lint", Gate::G2, "2026-10-28"),
            ],
        };
        assert_eq!(read(&folder.0), Ok(Some(expected)));
        folder.write(PATH, "adopted = \"2026-10-27\"\n");
        let empty = Known {
            adopted: "2026-10-27".into(),
            debt: vec![],
        };
        assert_eq!(read(&folder.0), Ok(Some(empty)));
    }

    #[test]
    fn a_file_not_shaped_as_known_debt_is_invalid() {
        let folder = Folder::new();
        for text in [
            "[[debt]",
            "[[debt]]\ncommand = \"lint\"\ngate = \"g2\"\nsince = \"x\"\n",
            "adopted = \"x\"\n[[debt]]\ncommand = \"lint\"\ngate = \"g4\"\nsince = \"x\"\n",
            "adopted = \"x\"\n[[debt]]\ngate = \"g2\"\nsince = \"x\"\n",
            "adopted = 1\n",
        ] {
            folder.write(PATH, text);
            let err = read(&folder.0).unwrap_err();
            let start = "gdd/debt.toml: invalid TOML: ";
            assert!(err.starts_with(start), "{text:?}: {err}");
        }
    }

    #[test]
    fn a_debt_may_have_a_count_that_is_a_whole_number() {
        let folder = Folder::new();
        folder.write(PATH, &format!("{KNOWN}count = 5\n"));
        let debts = read(&folder.0).unwrap().unwrap().debt;
        assert_eq!(debts[0].count, None);
        let lint = Debt {
            count: Some(5),
            ..debt("lint", Gate::G2, "2026-10-28")
        };
        assert_eq!(debts[1], lint);
        for count in ["count = -1\n", "count = \"5\"\n", "count = 1.5\n"] {
            folder.write(PATH, &format!("{KNOWN}{count}"));
            let err = read(&folder.0).unwrap_err();
            let start = "gdd/debt.toml: invalid TOML: ";
            assert!(err.starts_with(start), "{count:?}: {err}");
        }
    }

    #[test]
    fn a_file_with_a_command_that_never_becomes_debt_is_invalid() {
        let folder = Folder::new();
        for (name, gate) in [
            ("secrets", "g3"),
            ("acceptance", "g2"),
            ("acceptance_full", "g3"),
            ("mutation-slow", "g3"),
        ] {
            let entry = format!("[[debt]]\ncommand = {name:?}\ngate = {gate:?}\nsince = \"x\"\n");
            folder.write(PATH, &format!("{KNOWN}\n{entry}"));
            let said = format!(
                "gdd/debt.toml: {name:?} can never be known debt: it judges the change or its \
                 scenarios, so it always blocks. Take it off the list."
            );
            assert_eq!(read(&folder.0), Err(said.clone()));
            // A gate owes nothing then, and fails the check `debt` with why.
            let mut report = report();
            let owed = owed(&mut report, &folder.0, Gate::G3);
            assert_eq!(owed, BTreeMap::new());
            let [failure] = report.failures.as_slice() else {
                panic!("one failure: {report:?}");
            };
            assert_eq!((failure.name.as_str(), &failure.tail), (CHECK, &said));
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_not_no_debt() {
        let (folder, outside) = (Folder::new(), Folder::new());
        outside.write("debt.toml", KNOWN);
        fs::create_dir_all(folder.0.join("gdd")).unwrap();
        symlink(outside.0.join("debt.toml"), folder.0.join(PATH)).unwrap();
        let err = read(&folder.0).unwrap_err();
        let start = "gdd/debt.toml: cannot read: not a file";
        assert!(err.starts_with(start), "{err}");
        // A path under a regular file: not a folder on the way, then NotADirectory.
        let plain = Folder::new();
        plain.write("gdd", "a file\n");
        let err = read(&plain.0).unwrap_err();
        assert_eq!(err, "gdd/debt.toml: cannot read: gdd is not a folder");
        let err = read(&plain.0.join("gdd")).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: cannot read: "), "{err}");
        assert!(err.contains("Not a directory"), "{err}");
    }

    #[test]
    fn the_file_is_written_as_it_is_read() {
        let folder = Folder::new();
        let known = Known {
            adopted: "2026-10-27".into(),
            debt: vec![
                debt("security", Gate::G3, "2026-10-27"),
                counted("lint \"x\"", Gate::G2, 7),
            ],
        };
        let text = known.to_toml();
        let start = "# Known debt recorded by gdd adopt. These commands run and warn but do not \
                     block,\n# until a change makes each pass; the merge then takes it off this \
                     list for good.\n# A count is a ceiling: a change that reports more fails \
                     the gate, and one that\n# reports less lowers it when it merges.\n\
                     adopted = \"2026-10-27\"\n\n[[debt]]\ncommand = \"security\"\n\
                     gate = \"g3\"\nsince = \"2026-10-27\"\n\n[[debt]]\n";
        assert!(text.starts_with(start), "{text}");
        let end = "\nsince = \"2026-10-27\"\ncount = 7\n";
        assert!(text.ends_with(end), "{text}");
        write_new(&folder.0, PATH, text.as_bytes()).unwrap();
        assert_eq!(read(&folder.0), Ok(Some(known)));
    }

    #[test]
    fn a_file_is_never_written_through_a_link_or_another_name() {
        let (folder, outside) = (Folder::new(), Folder::new());
        outside.write("debt.toml", "outside\n");
        write_new(&folder.0, PATH, b"one").unwrap();
        write_new(&folder.0, PATH, b"two").unwrap();
        assert_eq!(fs::read_to_string(folder.0.join(PATH)).unwrap(), "two");
        fs::remove_file(folder.0.join(PATH)).unwrap();
        fs::hard_link(outside.0.join("debt.toml"), folder.0.join(PATH)).unwrap();
        let err = write_new(&folder.0, PATH, b"x").unwrap_err();
        let start = "cannot write gdd/debt.toml: not a file";
        assert!(err.starts_with(start), "{err}");
        fs::remove_file(folder.0.join(PATH)).unwrap();
        fs::remove_dir(folder.0.join("gdd")).unwrap();
        symlink(&outside.0, folder.0.join("gdd")).unwrap();
        let err = write_new(&folder.0, PATH, b"x").unwrap_err();
        assert_eq!(err, "cannot write gdd/debt.toml: gdd is not a folder");
        let text = fs::read_to_string(outside.0.join("debt.toml")).unwrap();
        assert_eq!(text, "outside\n");
    }

    #[test]
    fn mutation_secrets_and_the_scenarios_never_become_debt() {
        for name in ["lint", "unit", "security", "types-ts", "coverage"] {
            assert!(can_owe(name), "{name}");
        }
        for name in [
            "acceptance",
            "acceptance_full",
            "mutation",
            "mutation-slow",
            "secrets",
        ] {
            assert!(!can_owe(name), "{name}");
        }
    }

    #[test]
    fn the_riskiest_kind_comes_first_and_ties_go_by_name() {
        let names = [
            "lockfile",
            "lint-typescript",
            "zeta",
            "lint",
            "quality",
            "architecture",
            "unit",
            "types",
            "dependencies",
            "alpha",
            "security",
            "lint-rust",
            "lint",
        ];
        let expected = [
            "security",
            "dependencies",
            "types",
            "unit",
            "architecture",
            "quality",
            "lint",
            "lint-rust",
            "lint-typescript",
            "lockfile",
            "alpha",
            "zeta",
        ];
        assert_eq!(by_risk(names), expected);
        assert_eq!(risk("security-extra"), (0, "security-extra"));
        assert_eq!(risk("other"), (8, "other"));
    }

    #[test]
    fn the_listed_debt_has_one_line_per_command_with_the_count_of_each_gate() {
        let debts = [
            debt("lint", Gate::G3, "c"),
            debt("security", Gate::G3, "b"),
            counted("lint", Gate::G2, 5),
            debt("unit", Gate::G2, "d"),
        ];
        let listed = |command, since, counts: &[(Gate, Option<u64>)]| Listed {
            command,
            since,
            counts: counts.to_vec(),
        };
        let lint = [(Gate::G2, Some(5)), (Gate::G3, None)];
        let expected = [
            listed("security", "b", &[(Gate::G3, None)]),
            listed("unit", "d", &[(Gate::G2, None)]),
            listed("lint", "2026-10-27", &lint),
        ];
        assert_eq!(super::listed(&debts), expected);
    }

    #[test]
    fn the_counts_of_a_command_say_each_gate_when_they_differ() {
        let said = |counts: &[(Gate, Option<u64>)]| {
            let listed = Listed {
                command: "lint",
                since: "a",
                counts: counts.to_vec(),
            };
            (listed.said(), listed.uncounted())
        };
        let alike = |said: &str, uncounted| (said.to_string(), uncounted);
        assert_eq!(said(&[(Gate::G2, Some(5))]), alike("5", false));
        assert_eq!(said(&[(Gate::G3, None)]), alike("no count", true));
        let same = [(Gate::G2, Some(5)), (Gate::G3, Some(5))];
        assert_eq!(said(&same), alike("5", false));
        let none = [(Gate::G2, None), (Gate::G3, None)];
        assert_eq!(said(&none), alike("no count", true));
        let other = [(Gate::G2, Some(5)), (Gate::G3, Some(3))];
        assert_eq!(said(&other), alike("5 in G2, 3 in G3", false));
        let some = [(Gate::G2, Some(5)), (Gate::G3, None)];
        assert_eq!(said(&some), alike("5 in G2, no count in G3", true));
        let first = [(Gate::G2, None), (Gate::G3, Some(4))];
        assert_eq!(said(&first), alike("no count in G2, 4 in G3", true));
    }

    #[test]
    fn a_gate_owes_only_its_own_commands_and_fails_on_a_file_it_cannot_read() {
        let folder = Folder::new();
        let mut report = report();
        assert_eq!(owed(&mut report, &folder.0, Gate::G2), BTreeMap::new());
        folder.write(PATH, KNOWN);
        let g2 = owed(&mut report, &folder.0, Gate::G2);
        assert_eq!(g2, BTreeMap::from([("lint".to_string(), None)]));
        let g3 = owed(&mut report, &folder.0, Gate::G3);
        assert_eq!(g3, BTreeMap::from([("security".to_string(), None)]));
        // With the count recorded for its debt.
        folder.write(PATH, &format!("{KNOWN}count = 5\n"));
        let g2 = owed(&mut report, &folder.0, Gate::G2);
        assert_eq!(g2, BTreeMap::from([("lint".to_string(), Some(5))]));
        assert!(report.failures.is_empty(), "{report:?}");
        assert!(report.passed.is_empty(), "{report:?}");
        folder.write(PATH, "[[debt]");
        assert_eq!(owed(&mut report, &folder.0, Gate::G2), BTreeMap::new());
        let [failure] = report.failures.as_slice() else {
            panic!("one failure: {report:?}");
        };
        assert_eq!(failure.name, CHECK);
        let start = "gdd/debt.toml: invalid TOML";
        assert!(failure.tail.starts_with(start), "{failure:?}");
        // Without a report, why is the error.
        assert_eq!(of_gate(&folder.0, Gate::G2), Err(failure.tail.clone()));
    }

    #[test]
    fn the_latest_reports_say_what_is_owed_and_what_is_paid() {
        let folder = Folder::new();
        assert_eq!(reported(&folder.0, "c1"), Ok(Reported::default()));
        let g2 = r#"{"ok": true, "known_debt": [{"name": "lint", "tail": "x"},
                     {"name": "types", "tail": "y", "count": 3, "known": 5}],
                     "paid_debt": ["unit"]}"#;
        folder.write(".gdd/changes/c1/g2.json", g2);
        folder.write(".gdd/changes/c1/g3.json", r#"{"paid_debt": ["security"]}"#);
        let lint = Owed {
            tail: "x".into(),
            ..owing("lint", None)
        };
        let types = Owed {
            tail: "y".into(),
            known: Some(5),
            ..owing("types", Some(3))
        };
        let expected = Reported {
            owed: vec![(Gate::G2, lint), (Gate::G2, types)],
            paid: paid(&[(Gate::G2, "unit"), (Gate::G3, "security")]),
        };
        assert_eq!(reported(&folder.0, "c1"), Ok(expected));
        folder.write(".gdd/changes/c1/g3.json", "{}");
        assert_eq!(reported(&folder.0, "c1").unwrap().paid.len(), 1);
        folder.write(".gdd/changes/c1/g3.json", "{not json");
        let err = reported(&folder.0, "c1").unwrap_err();
        assert!(err.starts_with(".gdd/changes/c1/g3.json: "), "{err}");
        fs::remove_file(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        fs::create_dir_all(folder.0.join(".gdd/changes/c1/g3.json")).unwrap();
        let err = reported(&folder.0, "c1").unwrap_err();
        let start = "cannot read .gdd/changes/c1/g3.json: ";
        assert!(err.starts_with(start), "{err}");
    }

    #[test]
    fn paying_takes_the_commands_of_their_gate_off_and_the_last_removes_the_file() {
        let folder = Folder::new();
        assert_eq!(pay(&folder.0, &paid(&[(Gate::G2, "lint")]), &[]), Ok(()));
        assert!(!folder.0.join(PATH).exists());
        folder.write(PATH, KNOWN);
        // A name of the other gate is not paid.
        let other_gate = paid(&[(Gate::G3, "lint"), (Gate::G2, "security")]);
        pay(&folder.0, &other_gate, &[]).unwrap();
        assert_eq!(read(&folder.0).unwrap().unwrap().debt.len(), 2);
        pay(&folder.0, &paid(&[(Gate::G2, "lint")]), &[]).unwrap();
        let left = read(&folder.0).unwrap().unwrap();
        assert_eq!(left.adopted, "2026-10-27");
        assert_eq!(left.debt, [debt("security", Gate::G3, "2026-10-27")]);
        pay(&folder.0, &paid(&[(Gate::G3, "security")]), &[]).unwrap();
        assert!(!folder.0.join(PATH).exists());
    }

    fn lower(command: &str, gate: Gate, from: u64, to: u64) -> Lowered {
        Lowered {
            command: command.to_string(),
            gate,
            from,
            to,
        }
    }

    /// The debt of `debts`, written as the file of the known debt of `folder`.
    fn known_in(folder: &Folder, debts: Vec<Debt>) {
        let known = Known {
            adopted: "2026-10-27".into(),
            debt: debts,
        };
        folder.write(PATH, &known.to_toml());
    }

    #[test]
    fn a_change_lowers_the_counts_its_reports_saved_below_the_ones_recorded() {
        let folder = Folder::new();
        let owed = [
            (Gate::G2, owing("lint", Some(3))),
            (Gate::G2, owing("types", Some(9))),
            (Gate::G2, owing("unit", Some(4))),
            (Gate::G3, owing("security", Some(1))),
            (Gate::G3, owing("lint", Some(2))),
            (Gate::G2, owing("quality", None)),
        ];
        // Without the file, nothing is lowered.
        assert_eq!(lowered(&folder.0, &owed), Ok(vec![]));
        known_in(
            &folder,
            vec![
                counted("security", Gate::G3, 2),
                counted("types", Gate::G2, 9),
                counted("unit", Gate::G2, 2),
                counted("lint", Gate::G2, 5),
                debt("lint", Gate::G3, "2026-10-27"),
                counted("quality", Gate::G2, 4),
            ],
        );
        // Equal or higher, without a count recorded or reported: not lowered.
        let expected = [
            lower("lint", Gate::G2, 5, 3),
            lower("security", Gate::G3, 2, 1),
        ];
        assert_eq!(lowered(&folder.0, &owed), Ok(expected.to_vec()));
        let line: Vec<String> = expected.iter().map(Lowered::to_string).collect();
        assert_eq!(line, ["lint from 5 to 3", "security from 2 to 1"]);
        // A count of another gate does not lower it.
        let other = [(Gate::G3, owing("types", Some(1)))];
        assert_eq!(lowered(&folder.0, &other), Ok(vec![]));
    }

    #[test]
    fn without_a_count_reported_the_file_is_not_read() {
        let folder = Folder::new();
        folder.write(PATH, "[[debt]");
        let uncounted = [(Gate::G2, owing("lint", None))];
        assert_eq!(lowered(&folder.0, &uncounted), Ok(vec![]));
        assert_eq!(lowered(&folder.0, &[]), Ok(vec![]));
        let counted = [(Gate::G2, owing("lint", Some(1)))];
        let err = lowered(&folder.0, &counted).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML"), "{err}");
    }

    #[test]
    fn paying_lowers_the_counts_and_never_raises_one() {
        let folder = Folder::new();
        known_in(
            &folder,
            vec![
                counted("security", Gate::G3, 2),
                counted("lint", Gate::G2, 5),
                counted("lint", Gate::G3, 4),
                debt("unit", Gate::G2, "2026-10-27"),
            ],
        );
        let lowering = [
            lower("lint", Gate::G2, 5, 3),
            lower("security", Gate::G3, 9, 7),
            lower("unit", Gate::G2, 9, 1),
        ];
        pay(&folder.0, &[], &lowering).unwrap();
        let expected = vec![
            counted("security", Gate::G3, 2),
            counted("lint", Gate::G2, 3),
            counted("lint", Gate::G3, 4),
            debt("unit", Gate::G2, "2026-10-27"),
        ];
        assert_eq!(read(&folder.0).unwrap().unwrap().debt, expected);
        // Paid and lowered in one go.
        let paid = paid(&[(Gate::G3, "security")]);
        pay(&folder.0, &paid, &[lower("lint", Gate::G3, 4, 1)]).unwrap();
        let expected = vec![
            counted("lint", Gate::G2, 3),
            counted("lint", Gate::G3, 1),
            debt("unit", Gate::G2, "2026-10-27"),
        ];
        assert_eq!(read(&folder.0).unwrap().unwrap().debt, expected);
    }

    #[test]
    fn paying_a_file_that_cannot_be_read_is_an_error() {
        let folder = Folder::new();
        folder.write(PATH, "[[debt]");
        let err = pay(&folder.0, &paid(&[(Gate::G2, "lint")]), &[]).unwrap_err();
        assert!(err.starts_with("gdd/debt.toml: invalid TOML"), "{err}");
        assert_eq!(fs::read_to_string(folder.0.join(PATH)).unwrap(), "[[debt]");
    }

    /// The count of `output` by the one text `text`.
    fn by(text: &str, output: &str) -> u64 {
        count(output, &texts(&[text]))
    }

    #[test]
    fn a_text_with_a_number_adds_up_the_number_of_each_place_it_matches() {
        // Literals before and after the number.
        assert_eq!(by("Found {n} error", "Found 5 errors.\n"), 5);
        assert_eq!(by("Found {n} error", "Found 1 error in 1 file\n"), 1);
        assert_eq!(by("Found {n} error", "All checks passed!\n"), 0);
        let summary = "test result: FAILED. 3 passed; 2 failed; 0 ignored\n";
        assert_eq!(by("{n} failed;", summary), 2);
        assert_eq!(by("{n} failed;", "2 failed, 1 passed\n"), 0);
        // Nothing before: the number is all its digits.
        assert_eq!(by("{n} failed", "12 failed, 1 passed\n"), 12);
        assert_eq!(by("{n} failed", "a12 failed\n"), 12);
        // Nothing after.
        assert_eq!(by("Found {n}", "Found 5 errors\nFound 12"), 17);
        assert_eq!(by("{n}", "1 a 22 b 333"), 356);
        // Several on one line, and on several lines.
        let clippy = "error: could not compile `a` (lib) due to 3 previous errors; \
                      error: could not compile `a` (test) due to 4 previous errors\n\
                      error: could not compile `b` due to 1 previous error\n";
        assert_eq!(by("due to {n} previous error", clippy), 8);
        // The digits right after the literal before them.
        assert_eq!(by("error[E{n}]", "error[E0425]: cannot find value\n"), 425);
        let vitest = " Tests  2 failed | 5 passed (7)\n";
        assert_eq!(by("Tests  {n} failed", vitest), 2);
        // A place without digits, or without the literal after them, is no place.
        assert_eq!(by("Found {n} error", "Found Found 2 errors\n"), 2);
        let ruff = "Found 3 warnings, Found 2 errors\n";
        assert_eq!(by("Found {n} error", ruff), 2);
        let eslint = "✖ ✖ 4 problems (4 errors, 0 warnings)\n";
        assert_eq!(by("✖ {n} problem", eslint), 4);
        // Places do not overlap, but one may start where the last one ended.
        assert_eq!(by("a{n}a", "a1a2a"), 1);
        assert_eq!(by("a{n}a", "aa5a"), 5);
        assert_eq!(by("Found {n} error", "Found 1 errorFound 2 errors"), 3);
        // A literal after that starts with a digit keeps it: the number takes the most digits
        // that leave it next.
        assert_eq!(by("x{n}2", "x52"), 5);
        assert_eq!(by("x{n}2", "x522"), 52);
        assert_eq!(by("x{n}2", "x2 x52 x9"), 5);
        assert_eq!(by("{n}0 failed", "120 failed\n"), 12);
        assert_eq!(by("x{n}2", "x2"), 0);
    }

    #[test]
    fn a_number_too_large_skips_its_place() {
        let output = "99999999999999999999 failed, 3 failed\n";
        assert_eq!(by("{n} failed", output), 3);
        assert_eq!(by("{n} failed", "18446744073709551615 failed\n"), u64::MAX);
        let texts = texts(&["{n} failed", "{n} error"]);
        let both = "18446744073709551615 failed, 1 error\n";
        assert_eq!(count(both, &texts), u64::MAX);
    }

    #[test]
    fn a_text_without_a_number_counts_the_lines_that_contain_it() {
        let diff = "Diff in /a/src/lib.rs:1:\n x\nDiff in /a/src/main.rs:3:\n";
        assert_eq!(by("Diff in ", diff), 2);
        let tsc = "a.ts(1,1): error TS2322: x error TS7006\nb.ts(2,2): error TS2304: y\n";
        assert_eq!(by("error TS", tsc), 2);
        assert_eq!(by("error TS", "no errors\n"), 0);
        assert_eq!(by("error TS", ""), 0);
    }

    #[test]
    fn the_count_adds_up_every_text() {
        let python = texts(&["{n} failed", "{n} error"]);
        assert_eq!(count("2 failed, 1 error in 0.10s\n", &python), 3);
        let rust = texts(&["Diff in ", "due to {n} previous error"]);
        let output = "Diff in /a.rs:1:\nerror: could not compile `a` due to 3 previous errors\n";
        assert_eq!(count(output, &rust), 4);
        assert_eq!(count(output, &[]), 0);
    }

    fn ceiling(known: Option<u64>, texts: Option<&[String]>) -> Ceiling<'_> {
        Ceiling { known, texts }
    }

    #[test]
    fn a_count_above_the_known_debt_fails_and_one_at_or_below_it_warns() {
        let lint = texts(&["Found {n} error"]);
        let judged = |known: Option<u64>, output: &str| {
            held("lint", ceiling(known, Some(lint.as_slice())), output)
        };
        let more = "lint reports 7, the known debt is 5: 2 more";
        assert_eq!(judged(Some(5), "Found 7 errors.\n"), Err(more.to_string()));
        assert_eq!(judged(Some(5), "Found 5 errors.\n"), Ok(Some(5)));
        assert_eq!(judged(Some(5), "Found 3 errors.\n"), Ok(Some(3)));
        // Debt without a count recorded is held to nothing.
        assert_eq!(judged(None, "Found 7 errors.\n"), Ok(None));
        assert_eq!(judged(None, "panicked\n"), Ok(None));
    }

    #[test]
    fn a_count_that_cannot_be_read_fails() {
        let (lint, none) = (texts(&["Found {n} error"]), texts(&[]));
        let cannot = "security failed and gdd cannot count what it reported (its [count] texts \
                      match nothing)";
        let judged =
            |texts: Option<&[String]>| held("security", ceiling(Some(2), texts), "panicked\n");
        assert_eq!(judged(Some(lint.as_slice())), Err(cannot.to_string()));
        assert_eq!(judged(Some(none.as_slice())), Err(cannot.to_string()));
        // Without [count] for it any more.
        assert_eq!(judged(None), Err(cannot.to_string()));
    }
}
