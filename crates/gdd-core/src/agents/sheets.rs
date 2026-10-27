//! The agent sheets: one data file per coding agent the tool knows, built into it. A sheet says
//! where the agent keeps its commands and in which format, how one command calls another, and
//! how the recipe starts the agent headless: its program, its arguments for each role and how its
//! output reports what it used. In a sheet, `{name}` is always `gdd-<command>`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::agents::usage::Reader;

/// The sheets, by the name of their agent: the integration of `gdd/models.toml` that names it.
const SHEETS: [(&str, &str); 3] = [
    ("claude", include_str!("../../assets/agents/claude.toml")),
    ("codex", include_str!("../../assets/agents/codex.toml")),
    (
        "opencode",
        include_str!("../../assets/agents/opencode.toml"),
    ),
];

/// The agents with a sheet, as the errors list them.
pub const NAMES: &str = "claude, codex, opencode";

/// The sheet of an agent: what the tool reads of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Sheet {
    /// The name of the agent for people.
    title: String,
    /// Its program, found on the `PATH`.
    binary: String,
    /// How its output reports what it used.
    usage: Reader,
    commands: Commands,
    launch: Launch,
    /// The arguments of each role, by its name (`builder-build`, `reviewer`...).
    roles: BTreeMap<String, Role>,
}

/// Where an agent keeps its commands and how it calls them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Commands {
    /// The file of a command, from the project root.
    path: String,
    format: Format,
    /// How a command is called.
    invoke: String,
}

/// What the file of a command holds before its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// A skill: frontmatter with its name and its description.
    Skill,
    /// A command: frontmatter with its description only.
    Command,
}

/// How the recipe starts an agent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Launch {
    /// The arguments of a headless run, where `{invoke}`, `{name}`, `{args}` and `{model}` are
    /// filled in, and the argument `{role_args}` becomes the arguments of the role.
    args: Vec<String>,
    /// Arguments added to those of the role when its model has an `effort`, where `{effort}` is
    /// filled in.
    #[serde(default)]
    effort_args: Vec<String>,
}

/// What a sheet says of a role.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Role {
    /// Its arguments, as they are.
    args: Vec<String>,
}

/// The argument of `launch.args` that becomes the arguments of the role.
const ROLE_ARGS: &str = "{role_args}";

/// Why an agent has no sheet to work with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No sheet has that name.
    Missing(String),
    /// The sheet built into the tool cannot be read: its name, and why.
    Invalid(String, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Missing(name) => write!(f, "no agent sheet for {name:?} (sheets: {NAMES})"),
            Error::Invalid(name, why) => {
                write!(f, "the agent sheet of {name:?} is not valid: {why}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// The sheet of the agent `name`.
pub fn sheet(name: &str) -> Result<Sheet, Error> {
    let (_, text) = SHEETS
        .iter()
        .find(|(agent, _)| *agent == name)
        .ok_or_else(|| Error::Missing(name.to_string()))?;
    toml::from_str(text).map_err(|e| Error::Invalid(name.to_string(), e.to_string()))
}

/// The name of `command` in every agent: `gdd-<command>`.
pub fn name(command: &str) -> String {
    format!("gdd-{command}")
}

impl Sheet {
    /// The file of `command` for this agent, from the project root.
    pub fn path(&self, command: &str) -> String {
        fill(&self.commands.path, &[("name", &name(command))])
    }

    /// What the file of a command holds before its text.
    pub fn format(&self) -> Format {
        self.commands.format
    }

    /// How this agent calls `command`.
    pub fn invoke(&self, command: &str) -> String {
        fill(&self.commands.invoke, &[("name", &name(command))])
    }

    /// The name of this agent for people.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The program of this agent, found on the `PATH`.
    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// How the output of this agent reports what it used.
    pub fn usage(&self) -> Reader {
        self.usage
    }

    /// The arguments that start this agent headless on `command`, with the words `args`, the
    /// model `model` and the arguments of `role`, then, when the model has an `effort`, the
    /// effort arguments. A role the sheet does not name has no arguments.
    pub fn launch(
        &self,
        command: &str,
        args: &str,
        model: &str,
        role: &str,
        effort: Option<&str>,
    ) -> Vec<String> {
        let (name, invoke) = (name(command), self.invoke(command));
        let values = [
            ("invoke", invoke.as_str()),
            ("name", name.as_str()),
            ("args", args),
            ("model", model),
        ];
        let mut launch = Vec::new();
        for arg in &self.launch.args {
            if arg == ROLE_ARGS {
                launch.extend(self.role_args(role, effort));
            } else {
                launch.push(fill(arg, &values));
            }
        }
        launch
    }

    /// The arguments of `role`, then the effort arguments for `effort`, if there is one.
    fn role_args(&self, role: &str, effort: Option<&str>) -> Vec<String> {
        let role = self.roles.get(role);
        let mut args = role.map(|role| role.args.clone()).unwrap_or_default();
        if let Some(effort) = effort {
            let values = [("effort", effort)];
            let effort_args = self.launch.effort_args.iter();
            args.extend(effort_args.map(|arg| fill(arg, &values)));
        }
        args
    }
}

/// `template` with each `{key}` of `values` replaced by its value, in one pass: what a value
/// brings in is never filled again. Any other brace stays as it is.
pub(crate) fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut filled = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        filled.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        let found = values.iter().find_map(|(key, value)| {
            let after = rest.strip_prefix(key)?.strip_prefix('}')?;
            Some((value, after))
        });
        match found {
            Some((value, after)) => {
                filled.push_str(value);
                rest = after;
            }
            None => filled.push('{'),
        }
    }
    filled.push_str(rest);
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(name: &str) -> Sheet {
        sheet(name).unwrap()
    }

    /// The roles the recipe starts.
    const ROLES: [&str; 4] = ["builder-build", "builder-test", "reviewer", "orchestrator"];

    #[test]
    fn every_sheet_is_valid_and_names_its_own_agent_and_every_role() {
        let names: Vec<&str> = SHEETS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.join(", "), NAMES);
        for (name, text) in SHEETS {
            let sheet = sheet(name).unwrap();
            let table: toml::Table = toml::from_str(text).unwrap();
            assert_eq!(table["name"].as_str(), Some(name));
            let roles: Vec<&str> = sheet.roles.keys().map(String::as_str).collect();
            let mut expected = ROLES;
            expected.sort_unstable();
            assert_eq!(roles, expected, "{name}");
            let role_args = sheet.launch.args.iter().filter(|a| *a == ROLE_ARGS);
            assert_eq!(role_args.count(), 1, "{name}");
        }
    }

    #[test]
    fn each_sheet_names_its_program_and_how_it_reports_what_it_used() {
        let claude = of("claude");
        let named = (claude.title(), claude.binary(), claude.usage());
        assert_eq!(named, ("Claude Code", "claude", Reader::ClaudeJson));
        let codex = of("codex");
        let named = (codex.title(), codex.binary(), codex.usage());
        assert_eq!(named, ("Codex", "codex", Reader::CodexJsonl));
        let opencode = of("opencode");
        let named = (opencode.title(), opencode.binary(), opencode.usage());
        assert_eq!(named, ("OpenCode", "opencode", Reader::OpencodeJsonl));
    }

    #[test]
    fn an_agent_without_a_sheet_is_an_error() {
        let said = "no agent sheet for \"gemini\" (sheets: claude, codex, opencode)";
        assert_eq!(sheet("gemini").unwrap_err().to_string(), said);
        assert_eq!(sheet(""), Err(Error::Missing(String::new())));
        assert_eq!(sheet("Claude"), Err(Error::Missing("Claude".into())));
        let invalid = Error::Invalid("x".into(), "bad".into()).to_string();
        assert_eq!(invalid, "the agent sheet of \"x\" is not valid: bad");
    }

    #[test]
    fn each_agent_keeps_and_calls_its_commands_its_own_way() {
        let claude = of("claude");
        assert_eq!(claude.path("build"), ".claude/skills/gdd-build/SKILL.md");
        assert_eq!(claude.invoke("build-test"), "/gdd-build-test");
        assert_eq!(claude.format(), Format::Skill);
        let codex = of("codex");
        assert_eq!(codex.path("review"), ".agents/skills/gdd-review/SKILL.md");
        assert_eq!(codex.invoke("discover"), "$gdd-discover");
        assert_eq!(codex.format(), Format::Skill);
        let opencode = of("opencode");
        assert_eq!(opencode.path("status"), ".opencode/commands/gdd-status.md");
        assert_eq!(opencode.invoke("setup"), "/gdd-setup");
        assert_eq!(opencode.format(), Format::Command);
    }

    #[test]
    fn each_agent_is_started_as_its_sheet_says() {
        let args = "change=c1 mode=build";
        let claude = of("claude").launch("build", args, "sonnet", "builder-build", None);
        let start = [
            "-p",
            "/gdd-build change=c1 mode=build",
            "--model",
            "sonnet",
            "--output-format",
            "json",
            "--strict-mcp-config",
            "--permission-mode",
            "acceptEdits",
        ];
        assert_eq!(claude[..9], start);
        assert!(claude.contains(&"Write(.gdd/**)".to_string()));
        let codex = of("codex").launch("review", "change=c1 round=1", "gpt-6", "reviewer", None);
        let expected = [
            "exec",
            "--sandbox",
            "read-only",
            "-c",
            "mcp_servers={}",
            "$gdd-review change=c1 round=1",
            "--model",
            "gpt-6",
            "--json",
        ];
        assert_eq!(codex, expected);
        let words = "change=c1 r=2";
        let opencode = of("opencode").launch("replan", words, "kimi", "orchestrator", None);
        let expected = [
            "run",
            "--command",
            "gdd-replan",
            "-m",
            "kimi",
            "--format",
            "json",
            "change=c1 r=2",
        ];
        assert_eq!(opencode, expected);
    }

    #[test]
    fn each_role_gets_its_own_arguments() {
        let claude = of("claude");
        let mode = |role: &str| {
            let args = claude.launch("x", "", "m", role, None);
            let at = args.iter().position(|a| a == "--permission-mode").unwrap();
            args[at + 1].clone()
        };
        assert_eq!(mode("reviewer"), "plan");
        assert_eq!(mode("builder-test"), "acceptEdits");
        let orchestrator = claude.launch("replan", "", "m", "orchestrator", None);
        assert!(orchestrator.contains(&"Write(.gdd/log.jsonl)".to_string()));
        assert!(!orchestrator.contains(&"Write(.gdd/**)".to_string()));
        let codex = of("codex");
        let sandbox = |role: &str| codex.launch("x", "", "m", role, None)[2].clone();
        assert_eq!(sandbox("reviewer"), "read-only");
        assert_eq!(sandbox("builder-build"), "workspace-write");
        // A role the sheet does not name has no arguments.
        let none = codex.launch("x", "w", "m", "human", None);
        assert_eq!(none, ["exec", "$gdd-x w", "--model", "m", "--json"]);
    }

    #[test]
    fn the_effort_of_a_model_goes_after_its_role_when_the_sheet_takes_it() {
        let codex = of("codex").launch("review", "w", "m", "reviewer", Some("high"));
        let expected = [
            "exec",
            "--sandbox",
            "read-only",
            "-c",
            "mcp_servers={}",
            "-c",
            "model_reasoning_effort=high",
            "$gdd-review w",
            "--model",
            "m",
            "--json",
        ];
        assert_eq!(codex, expected);
        let unnamed = of("codex").launch("review", "w", "m", "human", Some("{model}"));
        assert_eq!(unnamed[1..3], ["-c", "model_reasoning_effort={model}"]);
        // A sheet without effort arguments takes no effort.
        let claude = of("claude");
        let with = claude.launch("build", "w", "m", "reviewer", Some("high"));
        assert_eq!(with, claude.launch("build", "w", "m", "reviewer", None));
        let opencode = of("opencode").launch("build", "w", "m", "reviewer", Some("high"));
        assert_eq!(opencode.len(), 8);
    }

    #[test]
    fn filling_replaces_each_key_once_and_leaves_other_braces() {
        let values = [("name", "gdd-x"), ("model", "{name}")];
        assert_eq!(fill("${name}/{name}", &values), "$gdd-x/gdd-x");
        // What a value brings in is not filled again.
        assert_eq!(fill("{model} {name}", &values), "{name} gdd-x");
        for kept in ["{other}", "{name", "{{name}", "name}", "{", "}{", ""] {
            let expected = kept.replace("{name}", "gdd-x");
            assert_eq!(fill(kept, &values), expected, "{kept:?}");
        }
        assert_eq!(fill("a{namex}b{name}", &values), "a{namex}bgdd-x");
        assert_eq!(fill("ñ{name}ñ", &values), "ñgdd-xñ");
    }
}
