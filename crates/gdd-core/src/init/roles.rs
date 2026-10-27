//! Who plays each role in a new project: the agents named, or the ones whose program is on the
//! `PATH`, with the defaults gdd init has for each of them (`assets/init/agents.toml`). The first
//! agent builds and orchestrates; the Reviewer is the first other one of another model family or,
//! when the human accepts one family only, the first agent again, with the day they accepted it.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Value;

use super::Env;
use crate::agents::invoke;
use crate::agents::models::SAME_FAMILY_ACCEPTED;
use crate::agents::sheets::{self, Sheet};
use crate::stacks::once;

/// The defaults of each agent, built into the tool.
const DEFAULTS: &str = include_str!("../../assets/init/agents.toml");

/// What gdd init gives an agent the first time: its model family, its models for strong,
/// standard and light work, and the reasoning effort it takes as the Reviewer, if any.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Defaults {
    family: String,
    strong: String,
    standard: String,
    light: String,
    effort: Option<String>,
}

/// An agent of the new project: its name, its sheet and its defaults.
#[derive(Debug, Clone)]
struct Agent {
    name: String,
    sheet: Sheet,
    defaults: Defaults,
}

/// Who plays each role in a new project.
#[derive(Debug, Clone)]
pub(super) struct Cast {
    /// Every agent of the project, in its order.
    agents: Vec<Agent>,
    /// The first agent: the Orchestrator and the Builder.
    builder: Agent,
    reviewer: Agent,
    /// The day the human accepted one model family only, when they had to.
    accepted: Option<String>,
}

impl Cast {
    /// Who plays each role among the agents `named` or, when none is, those whose program is on
    /// the `PATH` of `env`, the environment gdd runs in. With one model family only, the human
    /// must `accept` it: it is recorded as accepted on `today`.
    pub(super) fn new(
        named: &[String],
        env: &Env,
        accept: bool,
        today: &str,
    ) -> Result<Cast, String> {
        let defaults = defaults(DEFAULTS)?;
        cast(&defaults, chosen(named, env)?, accept, today)
    }

    /// The sheets of the agents, in their order.
    pub(super) fn sheets(&self) -> Vec<Sheet> {
        let sheets = self.agents.iter().map(|agent| &agent.sheet);
        sheets.cloned().collect()
    }

    /// `gdd/models.toml` for this cast, as gdd `version` writes it: the Orchestrator and the red
    /// Builder take the strong model of the first agent, the yellow Builder and test mode its
    /// standard one, the green Builder its light one; the Reviewer its strong model, with its
    /// effort; then the family of each agent.
    pub(super) fn models(&self, version: &str) -> String {
        let mut lines = vec![
            format!("# Models · GDD (written by gdd init {version})"),
            "# Who plays each role: an agent (integration) and its model. The Reviewer is of"
                .to_string(),
            "# another model family than the Builder, so it does not share its blind spots."
                .to_string(),
        ];
        let (name, own) = (&self.builder.name, &self.builder.defaults);
        for (table, model) in [
            ("orchestrator", &own.strong),
            ("builder.green", &own.light),
            ("builder.yellow", &own.standard),
            ("builder.red", &own.strong),
            ("builder_test", &own.standard),
        ] {
            lines.extend(role(table, name, model));
        }
        let reviewer = &self.reviewer;
        let strong = &reviewer.defaults.strong;
        lines.extend(role("reviewer", &reviewer.name, strong));
        if let Some(effort) = &reviewer.defaults.effort {
            lines.push(entry("effort", effort));
        }
        if let Some(day) = &self.accepted {
            let entry = entry(SAME_FAMILY_ACCEPTED, day);
            let accepted = format!("{entry}   # the human accepted one model family only");
            lines.push(accepted);
        }
        lines.extend([String::new(), "[families]".to_string()]);
        for agent in &self.agents {
            let (name, family) = (text(&agent.name), text(&agent.defaults.family));
            lines.push(format!("{name} = {family}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

/// The defaults of each agent, from `text`.
fn defaults(text: &str) -> Result<BTreeMap<String, Defaults>, String> {
    let invalid = |e: toml::de::Error| format!("the defaults of gdd init are not valid: {e}");
    toml::from_str(text).map_err(invalid)
}

/// The agents of a new project, with their sheets: the ones `named`, each once, in their order;
/// when none is, those whose program is on the `PATH` of `env`, in the order of the sheets. An
/// agent named without a sheet is an error.
fn chosen(named: &[String], env: &Env) -> Result<Vec<(String, Sheet)>, String> {
    let names = if named.is_empty() {
        sheets::NAMES.split(", ").map(str::to_string).collect()
    } else {
        once(named)
    };
    let mut chosen = Vec::new();
    for name in names {
        let sheet = sheets::sheet(&name).map_err(|e| e.to_string())?;
        if !named.is_empty() || invoke::program(&sheet, env).is_ok() {
            chosen.push((name, sheet));
        }
    }
    Ok(chosen)
}

/// Who plays each role among the agents `chosen`, with their `defaults` (see [`Cast::new`]).
fn cast(
    defaults: &BTreeMap<String, Defaults>,
    chosen: Vec<(String, Sheet)>,
    accept: bool,
    today: &str,
) -> Result<Cast, String> {
    let mut agents = Vec::new();
    for (name, sheet) in chosen {
        let Some(own) = defaults.get(&name) else {
            let why = format!("gdd init has no defaults for the agent {name}");
            return Err(why);
        };
        let defaults = own.clone();
        agents.push(Agent {
            name,
            sheet,
            defaults,
        });
    }
    let [builder, ..] = agents.as_slice() else {
        return Err(format!(
            "no coding agent found on the PATH ({}): install one or name it with --agents",
            sheets::NAMES
        ));
    };
    let family = &builder.defaults.family;
    let other = agents.iter().find(|a| a.defaults.family != *family);
    let (reviewer, accepted) = match other {
        Some(other) => (other, None),
        None if accept => (builder, Some(today.to_string())),
        None => return Err(one_family(family)),
    };
    Ok(Cast {
        builder: builder.clone(),
        reviewer: reviewer.clone(),
        agents,
        accepted,
    })
}

/// Why a project whose agents are all of the model family `family` needs the human to accept it.
fn one_family(family: &str) -> String {
    format!(
        "one model family only ({family}): the Reviewer would share the Builder's blind spots. \
         Install an agent of another family (OpenCode takes models from many providers) or run \
         again with --accept-same-family."
    )
}

/// The table `table` of a role: the agent `integration` with its model `model`, after an empty
/// line.
fn role(table: &str, integration: &str, model: &str) -> [String; 4] {
    [
        String::new(),
        format!("[{table}]"),
        entry("integration", integration),
        entry("model", model),
    ]
}

/// `<key> = <value>`, the value written as TOML text.
fn entry(key: &str, value: &str) -> String {
    format!("{key} = {}", text(value))
}

/// `value` written as TOML text.
fn text(value: &str) -> String {
    Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::models::{self, Models};
    use crate::config::Color;
    use crate::git::tests::Folder;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    /// Who plays each role among the agents `named`, with the built-in defaults.
    fn of(named: &[&str], accept: bool) -> Result<Cast, String> {
        Cast::new(&names(named), &[], accept, "2026-10-27")
    }

    fn agent_names(cast: &Cast) -> Vec<&str> {
        let names = cast.agents.iter().map(|agent| &agent.name);
        names.map(String::as_str).collect()
    }

    #[test]
    fn the_built_in_defaults_cover_every_agent_sheet() {
        let built_in = defaults(DEFAULTS).unwrap();
        let known: Vec<&str> = built_in.keys().map(String::as_str).collect();
        assert_eq!(known.join(", "), sheets::NAMES);
        assert_eq!(built_in["claude"].strong, "opus");
        assert_eq!(built_in["codex"].effort.as_deref(), Some("high"));
        assert_eq!(built_in["opencode"].effort, None);
        let wrong = defaults("[claude]\nfamily = \"x\"\n").unwrap_err();
        let start = "the defaults of gdd init are not valid: ";
        assert!(wrong.starts_with(start), "{wrong}");
        assert!(defaults("[claude]\nfamilies = \"x\"\n").is_err());
    }

    #[test]
    fn the_first_agent_builds_and_the_first_of_another_family_reviews() {
        let cast = of(&["claude", "opencode", "codex", "claude"], false).unwrap();
        assert_eq!(agent_names(&cast), ["claude", "opencode", "codex"]);
        assert_eq!(cast.builder.name, "claude");
        assert_eq!(cast.reviewer.name, "opencode");
        assert_eq!(cast.accepted, None);
        // The flag changes nothing when there is another family.
        let flagged = of(&["codex", "claude"], true).unwrap();
        assert_eq!(flagged.reviewer.name, "claude");
        assert_eq!(flagged.accepted, None);
        let sheets = cast.sheets();
        let sheets: Vec<String> = sheets.iter().map(|s| s.path("x")).collect();
        let paths = [
            ".claude/skills/gdd-x/SKILL.md",
            ".opencode/commands/gdd-x.md",
            ".agents/skills/gdd-x/SKILL.md",
        ];
        assert_eq!(sheets, paths);
    }

    #[test]
    fn one_family_only_needs_the_human_to_accept_it() {
        let refused = of(&["claude"], false).unwrap_err();
        let said = "one model family only (anthropic): the Reviewer would share the Builder's \
                    blind spots. Install an agent of another family (OpenCode takes models from \
                    many providers) or run again with --accept-same-family.";
        assert_eq!(refused, said);
        let accepted = of(&["claude"], true).unwrap();
        assert_eq!(accepted.reviewer.name, "claude");
        assert_eq!(accepted.accepted.as_deref(), Some("2026-10-27"));
    }

    #[test]
    fn an_agent_without_a_sheet_or_without_defaults_is_an_error() {
        let gemini = "no agent sheet for \"gemini\" (sheets: claude, codex, opencode)";
        assert_eq!(of(&["claude", "gemini"], false).unwrap_err(), gemini);
        let claude = "[claude]\nfamily = \"a\"\nstrong = \"s\"\nstandard = \"m\"\nlight = \"l\"\n";
        let some = defaults(claude).unwrap();
        let chosen = chosen(&names(&["claude", "codex"]), &[]).unwrap();
        let said = cast(&some, chosen, true, "d").unwrap_err();
        assert_eq!(said, "gdd init has no defaults for the agent codex");
    }

    /// A folder with a program for each of `named`; `runnable` says whether they may run.
    fn programs(named: &[&str], runnable: bool) -> Folder {
        let folder = Folder::new();
        for name in named {
            folder.write(name, "#!/bin/sh\n");
            let mode = if runnable { 0o755 } else { 0o644 };
            let file = folder.0.join(name);
            fs::set_permissions(file, fs::Permissions::from_mode(mode)).unwrap();
        }
        folder
    }

    fn path_of(folders: &[&Folder]) -> Vec<(String, String)> {
        let shown = folders.iter().map(|f| f.0.display().to_string());
        let joined: Vec<String> = shown.collect();
        vec![("PATH".to_string(), joined.join(":"))]
    }

    #[test]
    fn without_agents_named_the_ones_on_the_path_count_in_the_order_of_the_sheets() {
        let found = programs(&["opencode", "claude"], true);
        let idle = programs(&["codex"], false);
        let env = path_of(&[&found, &idle]);
        let cast = Cast::new(&[], &env, false, "d").unwrap();
        assert_eq!(agent_names(&cast), ["claude", "opencode"]);
        // Named agents need not be on the PATH.
        let named = Cast::new(&names(&["codex", "claude"]), &env, false, "d").unwrap();
        assert_eq!(agent_names(&named), ["codex", "claude"]);
        let none = Cast::new(&[], &path_of(&[&idle]), true, "d").unwrap_err();
        let said = "no coding agent found on the PATH (claude, codex, opencode): install one or \
                    name it with --agents";
        assert_eq!(none, said);
        assert_eq!(Cast::new(&[], &[], true, "d").unwrap_err(), said);
    }

    #[test]
    fn the_models_file_gives_each_role_its_model_and_each_agent_its_family() {
        let cast = of(&["claude", "codex"], false).unwrap();
        let text = cast.models("9.9.9");
        let expected = "# Models · GDD (written by gdd init 9.9.9)\n\
             # Who plays each role: an agent (integration) and its model. The Reviewer is of\n\
             # another model family than the Builder, so it does not share its blind spots.\n\n\
             [orchestrator]\nintegration = \"claude\"\nmodel = \"opus\"\n\n\
             [builder.green]\nintegration = \"claude\"\nmodel = \"haiku\"\n\n\
             [builder.yellow]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\n\
             [builder.red]\nintegration = \"claude\"\nmodel = \"opus\"\n\n\
             [builder_test]\nintegration = \"claude\"\nmodel = \"sonnet\"\n\n\
             [reviewer]\nintegration = \"codex\"\nmodel = \"gpt-6-astra\"\neffort = \"high\"\n\n\
             [families]\n\"claude\" = \"anthropic\"\n\"codex\" = \"openai\"\n";
        assert_eq!(text, expected);
        // What the tool reads back: another family reviews.
        let models = Models::from_toml(&text).unwrap();
        let roles = models.roles(Color::Red);
        assert_eq!(models.family(&roles.reviewer_model), "openai");
        assert_eq!(models.family(&roles.builder), "anthropic");
        assert_eq!(models.roles(Color::Green).builder["model"], "haiku");
        let accepted = models::same_family_accepted(&roles.reviewer_model);
        assert_eq!(accepted, None);
    }

    #[test]
    fn the_models_file_records_the_day_one_family_was_accepted() {
        let text = of(&["claude"], true).unwrap().models("1");
        let reviewer = "[reviewer]\nintegration = \"claude\"\nmodel = \"opus\"\n\
                        same_family_accepted = \"2026-10-27\"   # the human accepted one model \
                        family only\n\n[families]\n\"claude\" = \"anthropic\"\n";
        assert!(text.ends_with(reviewer), "{text}");
        let models = Models::from_toml(&text).unwrap();
        let roles = models.roles(Color::Yellow);
        let reviewer = models.family(&roles.reviewer_model);
        assert_eq!(reviewer, models.family(&roles.builder));
        let day = models::same_family_accepted(&roles.reviewer_model);
        assert_eq!(day, Some("2026-10-27"));
        // An agent without effort gives none to the Reviewer.
        let cast = of(&["codex", "opencode"], false).unwrap();
        let opencode = cast.models("1");
        assert!(!opencode.contains("effort"), "{opencode}");
        let reviewer = "[reviewer]\nintegration = \"opencode\"\n";
        assert!(opencode.contains(reviewer), "{opencode}");
    }
}
