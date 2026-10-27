//! The security exceptions of a project: the vulnerabilities its scanners are told to ignore,
//! in the scanners' own files. `gdd status` lists them; G3 refuses one without a reason and names
//! the ones a change adds, so the human sees them at G4.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use toml::value::{Date, Datetime};

use crate::config;
use crate::gates::Report;
use crate::git::{self, Git};
use crate::log;

/// The name of the check in a G3 report.
pub const CHECK: &str = "exceptions";

/// The file of osv-scanner, from the project root.
pub const OSV: &str = "osv-scanner.toml";

/// The file of cargo-deny, from the project root.
pub const DENY: &str = "deny.toml";

/// What the line of an exception a change adds starts with.
const ADDED: &str = "exception added: ";

/// The day [`today`] falls back to, as [`log::now`] does.
const EPOCH: Date = Date {
    year: 1970,
    month: 1,
    day: 1,
};

/// Reads the exceptions of a scanner from the text of its file; why it cannot.
type Parse = fn(&str) -> Result<Vec<Exception>, String>;

/// The file of each scanner and how to read it, in the order they are read.
const SCANNERS: [(&str, Parse); 2] = [(OSV, osv), (DENY, deny)];

/// A vulnerability a scanner of the project is told to ignore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exception {
    /// The file that says so.
    pub file: &'static str,
    /// What it ignores: an advisory, or the packages of an override.
    pub name: String,
    /// The day it ends, when the file says.
    pub until: Option<Date>,
    /// Why, when the file says and it is not blank.
    pub reason: Option<String>,
}

impl Exception {
    /// How it shows on `today`: `<name> · <file> · <end> · <reason>`, the end being
    /// `until <date>`, `expired <date>` once that day is past, or `no end date`.
    pub fn line(&self, today: Date) -> String {
        let end = match self.until {
            Some(until) if until < today => format!("expired {until}"),
            Some(until) => format!("until {until}"),
            None => "no end date".to_string(),
        };
        let reason = self.reason.as_deref().unwrap_or("no reason");
        format!("{} · {} · {end} · {reason}", self.name, self.file)
    }
}

/// An exception of `file`; a blank reason is no reason.
fn exception(
    file: &'static str,
    name: String,
    until: Option<Date>,
    reason: Option<String>,
) -> Exception {
    Exception {
        file,
        name,
        until,
        reason: reason.filter(|reason| !reason.trim().is_empty()),
    }
}

/// Today, in UTC: the day of [`log::now`], which always starts with one.
pub fn today() -> Date {
    let now = log::now().parse::<Datetime>().ok();
    now.and_then(|now| now.date).unwrap_or(EPOCH)
}

/// The security exceptions of the project at `root`: those of osv-scanner, then those of
/// cargo-deny, each file read only when it exists. A file that cannot be read, or that is not
/// shaped as its scanner reads it, is an error that names it.
pub fn read(root: &Path) -> Result<Vec<Exception>, String> {
    let mut found = Vec::new();
    for (file, parse) in SCANNERS {
        let text = match fs::read_to_string(root.join(file)) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("{file}: cannot read: {e}")),
        };
        let invalid = |why| format!("{file}: {}", config::Error::Toml(why));
        found.extend(parse(&text).map_err(invalid)?);
    }
    Ok(found)
}

/// G3's check of the exceptions `found` into `report`: it fails on each one without a reason,
/// or on why they could not be read.
pub(crate) fn judge(report: &mut Report, found: &Result<Vec<Exception>, String>) {
    let failures = match found {
        Ok(found) => found
            .iter()
            .filter(|exception| exception.reason.is_none())
            .map(|exception| format!("{}: {} has no reason", exception.file, exception.name))
            .collect(),
        Err(why) => vec![why.clone()],
    };
    report.judge(CHECK, failures);
}

/// The lines G3 prints for the exceptions `found` in the working tree of the project at `root`
/// that the same file does not have at the merge base of `HEAD` with the main branch `main`:
/// `exception added: ` and how each shows on `today`. A changed reason or date counts as added.
/// Outside a git repository there is no merge base, so every exception counts.
pub fn added(
    root: &Path,
    main: &str,
    found: &[Exception],
    today: Date,
) -> Result<Vec<String>, git::Error> {
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let before = at_merge_base(root, main)?;
    let lines = found
        .iter()
        .filter(|exception| !before.contains(exception))
        .map(|exception| format!("{ADDED}{}", exception.line(today)));
    Ok(lines.collect())
}

/// The exceptions of the project at `root` at the merge base of `HEAD` with the main branch
/// `main`: a file that was not there, or that its scanner could not read, has none.
fn at_merge_base(root: &Path, main: &str) -> Result<Vec<Exception>, git::Error> {
    if !git::is_repository(root) {
        return Ok(Vec::new());
    }
    let git = Git::new(root);
    let base = git.merge_base(main, "HEAD")?;
    let mut found = Vec::new();
    for (file, parse) in SCANNERS {
        if let Some(bytes) = git.file_at(&base, file)? {
            let text = String::from_utf8_lossy(&bytes);
            found.extend(parse(&text).unwrap_or_default());
        }
    }
    Ok(found)
}

#[derive(Deserialize)]
struct Osv {
    #[serde(default, rename = "IgnoredVulns")]
    ignored: Vec<IgnoredVuln>,
    #[serde(default, rename = "PackageOverrides")]
    overrides: Vec<PackageOverride>,
}

#[derive(Deserialize)]
struct IgnoredVuln {
    id: String,
    #[serde(rename = "ignoreUntil")]
    until: Option<toml::Value>,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct PackageOverride {
    name: Option<String>,
    ecosystem: Option<String>,
    group: Option<String>,
    #[serde(default)]
    ignore: bool,
    #[serde(default)]
    vulnerability: Ignore,
    #[serde(rename = "effectiveUntil")]
    until: Option<toml::Value>,
    reason: Option<String>,
}

#[derive(Default, Deserialize)]
struct Ignore {
    #[serde(default)]
    ignore: bool,
}

impl PackageOverride {
    /// The packages it applies to: `package <name>`, with ` (<ecosystem>)` when it has one;
    /// without a name, `packages <group>`, or `all packages` without a group either.
    fn packages(&self) -> String {
        match (&self.name, &self.ecosystem, &self.group) {
            (Some(name), Some(ecosystem), _) => format!("package {name} ({ecosystem})"),
            (Some(name), None, _) => format!("package {name}"),
            (None, _, Some(group)) => format!("packages {group}"),
            (None, _, None) => "all packages".to_string(),
        }
    }
}

/// The exceptions of osv-scanner: each `[[IgnoredVulns]]`, then each `[[PackageOverrides]]`
/// that ignores the package or its vulnerabilities. Overrides of licenses only are not security
/// exceptions.
fn osv(text: &str) -> Result<Vec<Exception>, String> {
    let osv: Osv = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    for vuln in osv.ignored {
        let until = end_date("ignoreUntil", vuln.until)?;
        found.push(exception(OSV, vuln.id, until, vuln.reason));
    }
    for package in osv.overrides {
        if package.ignore || package.vulnerability.ignore {
            let name = package.packages();
            let until = end_date("effectiveUntil", package.until)?;
            found.push(exception(OSV, name, until, package.reason));
        }
    }
    Ok(found)
}

#[derive(Deserialize)]
struct Deny {
    #[serde(default)]
    advisories: Advisories,
}

#[derive(Default, Deserialize)]
struct Advisories {
    #[serde(default)]
    ignore: Vec<Advisory>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Advisory {
    Id(String),
    Table { id: String, reason: Option<String> },
}

/// The exceptions of cargo-deny: each entry of `[advisories] ignore`, an id alone or a table
/// with its id and reason. None has an end date.
fn deny(text: &str) -> Result<Vec<Exception>, String> {
    let deny: Deny = toml::from_str(text).map_err(|e| e.to_string())?;
    let found = deny
        .advisories
        .ignore
        .into_iter()
        .map(|advisory| match advisory {
            Advisory::Id(id) => exception(DENY, id, None, None),
            Advisory::Table { id, reason } => exception(DENY, id, None, reason),
        });
    Ok(found.collect())
}

/// The day the value of `key` says, when there is one (see [`day_of`]).
fn end_date(key: &str, value: Option<toml::Value>) -> Result<Option<Date>, String> {
    value.map(|value| day_of(key, &value)).transpose()
}

/// The day `value`, the value of `key`, says: a TOML local date, or text `YYYY-MM-DD`.
fn day_of(key: &str, value: &toml::Value) -> Result<Date, String> {
    let datetime = match value {
        toml::Value::Datetime(datetime) => Some(*datetime),
        toml::Value::String(text) => text.parse::<Datetime>().ok(),
        _ => None,
    };
    match datetime {
        Some(Datetime {
            date: Some(date),
            time: None,
            offset: None,
        }) => Ok(date),
        _ => Err(format!("{key} = {value} is not a date (YYYY-MM-DD)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, Repo};

    const TODAY: Date = Date {
        year: 2026,
        month: 10,
        day: 27,
    };

    fn date(year: u16, month: u8, day: u8) -> Option<Date> {
        Some(Date { year, month, day })
    }

    fn found(
        file: &'static str,
        name: &str,
        until: Option<Date>,
        reason: Option<&str>,
    ) -> Exception {
        Exception {
            file,
            name: name.to_string(),
            until,
            reason: reason.map(str::to_string),
        }
    }

    fn lines(found: &[Exception]) -> Vec<String> {
        found
            .iter()
            .map(|exception| exception.line(TODAY))
            .collect()
    }

    #[test]
    fn an_ignored_vulnerability_has_its_id_its_end_and_its_reason() {
        let text = r#"
[[IgnoredVulns]]
id = "GHSA-1"
ignoreUntil = 2099-01-31
reason = "dev only"

[[IgnoredVulns]]
id = "GHSA-2"
ignoreUntil = "2027-02-03"
reason = "  "

[[IgnoredVulns]]
id = "GHSA-3"
"#;
        let expected = [
            found(OSV, "GHSA-1", date(2099, 1, 31), Some("dev only")),
            found(OSV, "GHSA-2", date(2027, 2, 3), None),
            found(OSV, "GHSA-3", None, None),
        ];
        assert_eq!(osv(text).unwrap(), expected);
        assert_eq!(osv("").unwrap(), []);
    }

    #[test]
    fn an_override_that_ignores_vulnerabilities_names_its_packages() {
        let text = r#"
[[PackageOverrides]]
name = "qs"
ecosystem = "npm"
vulnerability.ignore = true
effectiveUntil = 2099-06-30
reason = "pinned"

[[PackageOverrides]]
name = "lodash"
ignore = true

[[PackageOverrides]]
group = "dev"
ecosystem = "npm"
vulnerability.ignore = true
reason = "dev tools"

[[PackageOverrides]]
ignore = true
vulnerability.ignore = false
reason = "everything"

[[PackageOverrides]]
group = "dev"
license.ignore = true
reason = "licenses only"

[[PackageOverrides]]
name = "left-pad"
ignore = false
vulnerability.ignore = false

[[IgnoredVulns]]
id = "GHSA-1"
reason = "listed first"
"#;
        let expected = [
            found(OSV, "GHSA-1", None, Some("listed first")),
            found(OSV, "package qs (npm)", date(2099, 6, 30), Some("pinned")),
            found(OSV, "package lodash", None, None),
            found(OSV, "packages dev", None, Some("dev tools")),
            found(OSV, "all packages", None, Some("everything")),
        ];
        assert_eq!(osv(text).unwrap(), expected);
    }

    #[test]
    fn a_cargo_deny_exception_is_an_id_or_a_table_without_an_end() {
        let text = r#"
[graph]
all-features = true

[advisories]
version = 2
ignore = [
    "RUSTSEC-1",
    { id = "RUSTSEC-2", reason = "not reachable" },
    { id = "RUSTSEC-3" },
    { id = "RUSTSEC-4", reason = "" },
]
"#;
        let expected = [
            found(DENY, "RUSTSEC-1", None, None),
            found(DENY, "RUSTSEC-2", None, Some("not reachable")),
            found(DENY, "RUSTSEC-3", None, None),
            found(DENY, "RUSTSEC-4", None, None),
        ];
        assert_eq!(deny(text).unwrap(), expected);
        assert_eq!(deny("[advisories]\nversion = 2\n").unwrap(), []);
        assert_eq!(deny("").unwrap(), []);
    }

    #[test]
    fn an_exception_shows_its_end_as_of_today() {
        let shown = lines(&[
            found(OSV, "A", date(2026, 10, 28), Some("later")),
            found(OSV, "B", Some(TODAY), Some("today")),
            found(OSV, "C", date(2026, 10, 26), Some("past")),
            found(DENY, "D", None, None),
        ]);
        let expected = [
            "A · osv-scanner.toml · until 2026-10-28 · later",
            "B · osv-scanner.toml · until 2026-10-27 · today",
            "C · osv-scanner.toml · expired 2026-10-26 · past",
            "D · deny.toml · no end date · no reason",
        ];
        assert_eq!(shown, expected);
    }

    #[test]
    fn today_is_the_day_of_the_log() {
        let now = log::now();
        assert_eq!(today().to_string(), now[..10], "{now}");
    }

    #[test]
    fn the_exceptions_are_read_from_osv_scanner_then_cargo_deny() {
        let folder = Folder::new();
        assert_eq!(read(&folder.0).unwrap(), []);
        folder.write(DENY, "[advisories]\nignore = [\"RUSTSEC-1\"]\n");
        let deny_only = [found(DENY, "RUSTSEC-1", None, None)];
        assert_eq!(read(&folder.0).unwrap(), deny_only);
        folder.write(OSV, "[[IgnoredVulns]]\nid = \"GHSA-1\"\nreason = \"r\"\n");
        let both = [
            found(OSV, "GHSA-1", None, Some("r")),
            found(DENY, "RUSTSEC-1", None, None),
        ];
        assert_eq!(read(&folder.0).unwrap(), both);
    }

    #[test]
    fn a_file_not_shaped_as_its_scanner_reads_it_is_invalid() {
        let folder = Folder::new();
        for (file, text) in [
            (OSV, "[[IgnoredVulns]"),
            (OSV, "IgnoredVulns = 1"),
            (OSV, "[[IgnoredVulns]]\nreason = \"no id\""),
            (OSV, "[[IgnoredVulns]]\nid = 1"),
            (OSV, "[[IgnoredVulns]]\nid = \"A\"\nreason = 1"),
            (OSV, "[[PackageOverrides]]\nignore = \"yes\""),
            (OSV, "[[PackageOverrides]]\nvulnerability.ignore = 1"),
            (DENY, "advisories = 1"),
            (DENY, "[advisories]\nignore = \"RUSTSEC-1\""),
            (DENY, "[advisories]\nignore = [{ crate = \"x\" }]"),
            (DENY, "[advisories]\nignore = [1]"),
        ] {
            let other = if file == OSV { DENY } else { OSV };
            let _ = fs::remove_file(folder.0.join(other));
            folder.write(file, text);
            let err = read(&folder.0).unwrap_err();
            let start = format!("{file}: invalid TOML: ");
            assert!(err.starts_with(&start), "{text:?}: {err}");
        }
    }

    #[test]
    fn an_end_date_is_a_local_date_or_its_text() {
        for wrong in [
            "1",
            "true",
            "\"soon\"",
            "\"2099-1-31\"",
            "2099-01-31T10:00:00",
            "2099-01-31T10:00:00Z",
            "\"2099-01-31T10:00:00\"",
            "10:00:00",
        ] {
            let text = format!("[[IgnoredVulns]]\nid = \"A\"\nignoreUntil = {wrong}\n");
            let err = osv(&text).unwrap_err();
            let said = format!("ignoreUntil = {wrong} is not a date (YYYY-MM-DD)");
            assert_eq!(err, said, "{wrong}");
            let ignored = "[[PackageOverrides]]\nignore = true\n";
            let err = osv(&format!("{ignored}effectiveUntil = {wrong}\n")).unwrap_err();
            assert!(err.starts_with("effectiveUntil = "), "{wrong}: {err}");
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_says_so() {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(OSV)).unwrap();
        let err = read(&folder.0).unwrap_err();
        assert!(err.starts_with("osv-scanner.toml: cannot read: "), "{err}");
        // A path under a regular file is not a missing file either.
        folder.write("plain", "x\n");
        let err = read(&folder.0.join("plain")).unwrap_err();
        assert!(err.starts_with("osv-scanner.toml: cannot read: "), "{err}");
    }

    #[test]
    fn g3_refuses_an_exception_without_a_reason_and_a_file_it_cannot_read() {
        let mut report = Report {
            ok: false,
            gate: crate::gates::Gate::G3,
            color: crate::config::Color::Red,
            passed: vec![],
            skipped: vec![],
            failures: vec![],
            out_of_scope: vec![],
            added_exceptions: vec![],
            known_debt: vec![],
            paid_debt: vec![],
            duration_s: 0.0,
        };
        judge(&mut report, &Ok(vec![]));
        judge(&mut report, &Ok(vec![found(OSV, "A", None, Some("r"))]));
        assert_eq!(report.passed, [CHECK, CHECK]);
        assert!(report.failures.is_empty(), "{report:?}");
        let found = vec![
            found(OSV, "A", None, None),
            found(OSV, "B", None, Some("r")),
            found(DENY, "C", None, None),
        ];
        judge(&mut report, &Ok(found));
        judge(&mut report, &Err("deny.toml: cannot read: x".to_string()));
        let tails: Vec<(&str, &str)> = report
            .failures
            .iter()
            .map(|f| (f.name.as_str(), f.tail.as_str()))
            .collect();
        let refused = "osv-scanner.toml: A has no reason\ndeny.toml: C has no reason";
        let expected = [(CHECK, refused), (CHECK, "deny.toml: cannot read: x")];
        assert_eq!(tails, expected);
    }

    #[test]
    fn the_exceptions_added_are_the_ones_the_merge_base_lacks() {
        let repo = Repo::new();
        let on_main = "[[IgnoredVulns]]\nid = \"A\"\nreason = \"r\"\n\
                       [[IgnoredVulns]]\nid = \"B\"\nreason = \"old\"\n";
        repo.write(OSV, on_main);
        repo.commit();
        repo.start("c1");
        let root = repo.root();
        let none = added(root, "main", &read(root).unwrap(), TODAY).unwrap();
        assert_eq!(none, Vec::<String>::new());
        repo.write(
            OSV,
            "[[IgnoredVulns]]\nid = \"A\"\nreason = \"r\"\n\
             [[IgnoredVulns]]\nid = \"B\"\nreason = \"new\"\n\
             [[IgnoredVulns]]\nid = \"C\"\nignoreUntil = 2026-01-01\nreason = \"c\"\n",
        );
        repo.write(DENY, "[advisories]\nignore = [\"A\"]\n");
        // Committed on the branch of the change, they are still not on the main branch.
        repo.commit();
        let found = read(root).unwrap();
        let expected = [
            "exception added: B · osv-scanner.toml · no end date · new",
            "exception added: C · osv-scanner.toml · expired 2026-01-01 · c",
            "exception added: A · deny.toml · no end date · no reason",
        ];
        assert_eq!(added(root, "main", &found, TODAY).unwrap(), expected);
        let err = added(root, "trunk", &found, TODAY).unwrap_err();
        assert!(err.to_string().starts_with("git merge-base "), "{err}");
        let nothing = added(root, "trunk", &[], TODAY).unwrap();
        assert_eq!(nothing, Vec::<String>::new());
    }

    #[test]
    fn a_file_the_merge_base_cannot_read_has_no_exceptions_there() {
        let repo = Repo::new();
        repo.write(OSV, "[[IgnoredVulns]\n");
        repo.commit();
        let root = repo.root();
        repo.write(OSV, "[[IgnoredVulns]]\nid = \"A\"\nreason = \"r\"\n");
        let found = read(root).unwrap();
        let expected = ["exception added: A · osv-scanner.toml · no end date · r"];
        assert_eq!(added(root, "main", &found, TODAY).unwrap(), expected);
    }

    #[test]
    fn outside_a_git_repository_every_exception_is_added() {
        let folder = Folder::new();
        folder.write(OSV, "[[IgnoredVulns]]\nid = \"A\"\nreason = \"r\"\n");
        let found = read(&folder.0).unwrap();
        let expected = ["exception added: A · osv-scanner.toml · no end date · r"];
        assert_eq!(added(&folder.0, "main", &found, TODAY).unwrap(), expected);
    }
}
