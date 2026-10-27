//! The stack sheets: one data file per stack the tool knows, built into it. A sheet says how a
//! project of that stack passes the gates: the commands of each gate, its scenario runner, the
//! file of the harness self-test, the paths each Builder mode must not touch, the files of its
//! supply chain and the tool chosen for each need. Each stack also has its instructions for the
//! agents. A project names the stacks it uses in `stack` of `gdd/project.toml` ([`of_project`]);
//! [`compose()`] says what several sheets give together.

mod compose;

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use toml::Spanned;

use crate::config;

pub use compose::compose;

/// The sheets, sorted by the name of their stack: the sheet, then the instructions for the
/// agents (frontmatter with `description: "..."`, then the text).
const SHEETS: [(&str, &str, &str); 3] = [
    (
        "python",
        include_str!("../../assets/stacks/python.toml"),
        include_str!("../../assets/stacks/python.md"),
    ),
    (
        "rust",
        include_str!("../../assets/stacks/rust.toml"),
        include_str!("../../assets/stacks/rust.md"),
    ),
    (
        "typescript",
        include_str!("../../assets/stacks/typescript.toml"),
        include_str!("../../assets/stacks/typescript.md"),
    ),
];

/// The stacks with a sheet, as the errors list them.
pub const NAMES: &str = "python, rust, typescript";

/// Why composing needs a stack, and how to name one.
const NO_NAME: &str = "name at least one stack: gdd stacks compose <name>...";

/// Why `stack` of `gdd/project.toml` names no stack.
const NOT_NAMES: &str = "stack is not a name or a list of names";

/// The sections of commands of a sheet, in the order the tool shows them.
const SECTIONS: [&str; 3] = ["g2", "g3", "learn"];

/// Commands by name, in the order of their sheet. An empty one is not configured.
type Commands = Vec<(String, String)>;

/// The sheet of a stack: what the tool reads of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The name of the stack, as `gdd/project.toml` writes it.
    name: String,
    /// Its name for people.
    title: String,
    /// The tool that runs its Gherkin scenarios.
    scenario_runner: String,
    /// The files that tell a project is of this stack.
    detect: Vec<String>,
    /// The files of its dependencies and toolchain: a red zone from the start.
    supply_chain: Vec<String>,
    /// What installs the dependencies of a project.
    install: String,
    /// The commands of each of [`SECTIONS`], in that order.
    sections: [Commands; 3],
    selftest: Selftest,
    protected: Protected,
    tools: Vec<Tool>,
}

/// A sheet as it is written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    title: String,
    scenario_runner: String,
    detect: Vec<String>,
    supply_chain: Vec<String>,
    install: String,
    // Where each command is written, so they keep the order of the sheet.
    g2: BTreeMap<String, Spanned<String>>,
    g3: BTreeMap<String, Spanned<String>>,
    learn: BTreeMap<String, Spanned<String>>,
    selftest: Selftest,
    protected: Protected,
    tool: Vec<Tool>,
}

/// The deliberately failing test of the harness self-test.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selftest {
    /// Its path, from the project root.
    file: String,
    /// Its text.
    content: String,
}

/// The paths each Builder mode must not touch.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Protected {
    builder_build: Vec<String>,
    builder_test: Vec<String>,
}

/// The tool a sheet chose for a need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    /// What it is for.
    need: String,
    /// Where it works: a gate, Learn...
    gate: String,
    /// The tool chosen.
    default: String,
    /// The one to take in its place, if there is one.
    alternative: Option<String>,
}

/// Why a stack has no sheet to work with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No sheet has that name.
    Unknown(String),
    /// Composing named no stack.
    NoName,
    /// The sheet built into the tool cannot be read: its name, and why.
    Invalid(String, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unknown(name) => write!(f, "unknown stack {name} (known: {NAMES})"),
            Error::NoName => f.write_str(NO_NAME),
            Error::Invalid(name, why) => write!(f, "the {name} stack sheet is not valid: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The sheet and the instructions of the stack `name`, as they are built in.
fn built_in(name: &str) -> Result<(&'static str, &'static str), Error> {
    let (_, sheet, instructions) = SHEETS
        .iter()
        .find(|(stack, ..)| *stack == name)
        .ok_or_else(|| Error::Unknown(name.to_string()))?;
    Ok((sheet, instructions))
}

/// The sheet of the stack `name`.
pub fn sheet(name: &str) -> Result<Sheet, Error> {
    let (text, _) = built_in(name)?;
    parse(text).map_err(|why| Error::Invalid(name.to_string(), why))
}

/// The instructions of the stack `name` for the agents: frontmatter with `description: "..."`,
/// then the text.
pub fn instructions(name: &str) -> Result<&'static str, Error> {
    built_in(name).map(|(_, instructions)| instructions)
}

/// One line per sheet, sorted by the name of its stack: `<name> · <title> · <scenario_runner>`,
/// without a line break at the end.
pub fn list() -> Result<String, Error> {
    let mut lines = Vec::new();
    for (name, ..) in SHEETS {
        let sheet = sheet(name)?;
        let (title, runner) = (&sheet.title, &sheet.scenario_runner);
        lines.push(format!("{} · {title} · {runner}", sheet.name));
    }
    Ok(lines.join("\n"))
}

/// Reads the text of a sheet, or says why it cannot.
fn parse(text: &str) -> Result<Sheet, String> {
    let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
    Ok(Sheet {
        name: raw.name,
        title: raw.title,
        scenario_runner: raw.scenario_runner,
        detect: raw.detect,
        supply_chain: raw.supply_chain,
        install: raw.install,
        sections: [ordered(raw.g2), ordered(raw.g3), ordered(raw.learn)],
        selftest: raw.selftest,
        protected: raw.protected,
        tools: raw.tool,
    })
}

/// The commands of a section in the order they are written.
fn ordered(section: BTreeMap<String, Spanned<String>>) -> Commands {
    let mut commands: Vec<(String, Spanned<String>)> = section.into_iter().collect();
    commands.sort_by_key(|(_, command)| command.span().start);
    let text = |(name, command): (String, Spanned<String>)| (name, command.into_inner());
    commands.into_iter().map(text).collect()
}

impl Sheet {
    /// The name of the stack for people.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The tool that runs its Gherkin scenarios.
    pub fn scenario_runner(&self) -> &str {
        &self.scenario_runner
    }

    /// The files of its dependencies and toolchain.
    pub fn supply_chain(&self) -> &[String] {
        &self.supply_chain
    }

    /// The paths the Builder in build mode must not touch.
    pub fn builder_build(&self) -> &[String] {
        &self.protected.builder_build
    }

    /// The paths the Builder in test mode must not touch.
    pub fn builder_test(&self) -> &[String] {
        &self.protected.builder_test
    }
}

impl fmt::Display for Sheet {
    /// As `gdd stacks show` prints it: the stack, its tools, its commands that are not empty and
    /// its protected paths, without a line break at the end.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (title, name, runner) = (&self.title, &self.name, &self.scenario_runner);
        write!(f, "{title} ({name}) · scenarios: {runner}")?;
        writeln!(f, " · detected by: {}", self.detect.join(", "))?;
        writeln!(f, "Tools:")?;
        for tool in &self.tools {
            writeln!(f, "{tool}")?;
        }
        writeln!(f, "Commands:")?;
        writeln!(f, "install: {}", self.install)?;
        for (section, commands) in SECTIONS.iter().zip(&self.sections) {
            for (name, command) in commands.iter().filter(|(_, c)| !c.is_empty()) {
                writeln!(f, "{section}.{name}: {command}")?;
            }
        }
        writeln!(f, "Protected:")?;
        let protected = &self.protected;
        writeln!(f, "builder_build: {}", protected.builder_build.join(", "))?;
        write!(f, "builder_test: {}", protected.builder_test.join(", "))
    }
}

impl fmt::Display for Tool {
    /// `<need> (<gate>): <default>`, then ` · alternative: <alternative>` when it has one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): {}", self.need, self.gate, self.default)?;
        match &self.alternative {
            Some(alternative) => write!(f, " · alternative: {alternative}"),
            None => Ok(()),
        }
    }
}

/// The stacks a project uses, from the text of its `gdd/project.toml`: `stack`, one name or a
/// list of names, each once. A missing or empty `stack` names none.
pub fn of_project(text: &str) -> Result<Vec<String>, config::Error> {
    let table: toml::Table =
        toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
    let not_names = || config::Error::Toml(NOT_NAMES.to_string());
    let names = match table.get("stack") {
        None => Vec::new(),
        Some(toml::Value::String(name)) if name.is_empty() => Vec::new(),
        Some(toml::Value::String(name)) => vec![name.clone()],
        Some(toml::Value::Array(list)) => {
            let names = list.iter().map(|name| name.as_str().map(str::to_string));
            names.collect::<Option<_>>().ok_or_else(not_names)?
        }
        Some(_) => return Err(not_names()),
    };
    Ok(once(&names))
}

/// `items` in their order, each once.
pub(crate) fn once<'a, T: PartialEq + Clone + 'a>(
    items: impl IntoIterator<Item = &'a T>,
) -> Vec<T> {
    let mut kept = Vec::new();
    for item in items {
        if !kept.contains(item) {
            kept.push(item.clone());
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(name: &str) -> Sheet {
        sheet(name).unwrap()
    }

    #[test]
    fn every_sheet_is_valid_names_its_own_stack_and_has_its_instructions() {
        let names: Vec<&str> = SHEETS.iter().map(|(name, ..)| *name).collect();
        assert_eq!(names.join(", "), NAMES);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        for name in names {
            let sheet = of(name);
            assert_eq!(sheet.name, name);
            assert!(!sheet.tools.is_empty(), "{name}");
            let text = instructions(name).unwrap();
            assert!(text.starts_with("---\ndescription: \""), "{name}");
        }
    }

    #[test]
    fn a_stack_without_a_sheet_is_an_error() {
        let said = "unknown stack cobol (known: python, rust, typescript)";
        assert_eq!(sheet("cobol").unwrap_err().to_string(), said);
        let cobol = Error::Unknown("cobol".into());
        assert_eq!(instructions("cobol"), Err(cobol));
        for unknown in ["", "Rust", "rust "] {
            assert_eq!(sheet(unknown), Err(Error::Unknown(unknown.into())));
        }
        let invalid = Error::Invalid("x".into(), "bad".into()).to_string();
        assert_eq!(invalid, "the x stack sheet is not valid: bad");
        let none = "name at least one stack: gdd stacks compose <name>...";
        assert_eq!(Error::NoName.to_string(), none);
    }

    #[test]
    fn the_list_has_one_line_per_sheet_sorted_by_name() {
        let expected = "python · Python · pytest-bdd\nrust · Rust · cucumber-rs\n\
                        typescript · TypeScript · cucumber-js";
        assert_eq!(list().unwrap(), expected);
    }

    /// A sheet with every field, its commands written out of the order of their names.
    const SHEET: &str = r#"
name = "demo"
title = "Demo"
scenario_runner = "behave"
detect = ["a.toml", "b.json"]
supply_chain = ["a.lock", "toolchain"]
install = "make deps"

[g2]
unit = "make unit"
lint = ""
acceptance = """make \
  acceptance"""

[g3]
mutation = "make mutants"

[learn]

[selftest]
file = "t/fail.x"
content = "fail\n"

[protected]
builder_build = ["features/**", "t/steps/**"]
builder_test = ["src/**"]

[[tool]]
need = "Unit tests"
gate = "G2"
default = "unit (1k stars)"

[[tool]]
need = "Lint"
gate = "G2"
default = "lint"
alternative = "other lint"
"#;

    #[test]
    fn a_sheet_says_its_title_runner_supply_chain_and_protected_paths() {
        let sheet = parse(SHEET).unwrap();
        assert_eq!((sheet.title(), sheet.scenario_runner()), ("Demo", "behave"));
        assert_eq!(sheet.supply_chain(), ["a.lock", "toolchain"]);
        assert_eq!(sheet.builder_build(), ["features/**", "t/steps/**"]);
        assert_eq!(sheet.builder_test(), ["src/**"]);
        let rust = of("rust");
        let supply_chain = ["Cargo.lock", "deny.toml", "rust-toolchain.toml"];
        assert_eq!(rust.supply_chain(), supply_chain);
        for name in NAMES.split(", ") {
            assert!(!of(name).supply_chain().is_empty(), "{name}");
        }
    }

    #[test]
    fn a_sheet_keeps_the_order_of_its_commands() {
        let sheet = parse(SHEET).unwrap();
        let g2: Vec<&String> = sheet.sections[0].iter().map(|(name, _)| name).collect();
        assert_eq!(g2, ["unit", "lint", "acceptance"]);
        assert!(sheet.sections[2].is_empty());
    }

    #[test]
    fn show_says_the_tools_the_commands_not_empty_and_the_protected_paths() {
        let expected = "Demo (demo) · scenarios: behave · detected by: a.toml, b.json\n\
                        Tools:\n\
                        Unit tests (G2): unit (1k stars)\n\
                        Lint (G2): lint · alternative: other lint\n\
                        Commands:\n\
                        install: make deps\n\
                        g2.unit: make unit\n\
                        g2.acceptance: make acceptance\n\
                        g3.mutation: make mutants\n\
                        Protected:\n\
                        builder_build: features/**, t/steps/**\n\
                        builder_test: src/**";
        assert_eq!(parse(SHEET).unwrap().to_string(), expected);
        let multi = SHEET.replace("make \\\n  acceptance", "make\nacceptance");
        let shown = parse(&multi).unwrap().to_string();
        let acceptance = "\ng2.acceptance: make\nacceptance\ng3.";
        assert!(shown.contains(acceptance), "{shown}");
        let python = of("python").to_string();
        let start = "Python (python) · scenarios: pytest-bdd · detected by: ";
        assert!(python.starts_with(start), "{python}");
        assert!(python.contains("\ng2.types: mypy src\n"), "{python}");
    }

    #[test]
    fn a_sheet_that_does_not_read_says_why() {
        assert!(parse(SHEET).is_ok());
        for wrong in [
            SHEET.replace("name = \"demo\"\n", ""),
            SHEET.replace("[learn]\n", "[learn]\nunused = 3\n"),
            SHEET.replace("need = \"Lint\"", "needs = \"Lint\""),
            SHEET.replace("content", "text"),
            SHEET.replace("supply_chain = [\"a.lock\", \"toolchain\"]\n", ""),
            format!("{SHEET}extra = 1\n"),
            "[g2".to_string(),
        ] {
            let why = parse(&wrong).unwrap_err();
            assert!(!why.is_empty(), "{wrong}");
        }
    }

    #[test]
    fn a_project_names_one_stack_several_or_none() {
        let named = |text: &str| of_project(text).unwrap();
        assert_eq!(named("name = \"x\"\n"), Vec::<String>::new());
        assert_eq!(named("stack = \"\"\n"), Vec::<String>::new());
        assert_eq!(named("stack = \"rust\"\n"), ["rust"]);
        assert_eq!(named("stack = []\n"), Vec::<String>::new());
        let several = "stack = [\"rust\", \"python\", \"rust\"]\nname = \"x\"\n";
        assert_eq!(named(several), ["rust", "python"]);
        // An unknown name is for whoever uses it to say.
        assert_eq!(named("stack = \"cobol\"\n"), ["cobol"]);
        let not_names = config::Error::Toml(NOT_NAMES.into());
        for wrong in [
            "stack = 3\n",
            "stack = [\"rust\", 3]\n",
            "[stack]\nrust = true\n",
        ] {
            assert_eq!(of_project(wrong), Err(not_names.clone()), "{wrong:?}");
        }
        let wrong = of_project("stack = ");
        assert!(matches!(wrong, Err(config::Error::Toml(_))));
    }

    #[test]
    fn once_keeps_the_first_of_each() {
        let items = ["b", "a", "b", "c", "a"].map(String::from);
        assert_eq!(once(&items), ["b", "a", "c"]);
        assert_eq!(once::<String>(&[]), Vec::<String>::new());
    }
}
