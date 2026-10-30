//! The texts gdd init writes: the project basics, the gate commands, the list of the files it
//! writes once (`gdd/generated.toml`), and the templates of `assets/init/` with their
//! placeholders filled. A template only changes where it has a placeholder; any other brace stays.

use toml::Value;

use super::{Answers, Options, PLATFORMS};
use crate::VERSION;
use crate::agents::sheets::fill;
use crate::stacks::{self, Sheet, once};

/// The rules table.
pub(super) const RULES: &str = include_str!("../../assets/init/rules.toml");
/// The zone map.
pub(super) const ZONES: &str = include_str!("../../assets/init/zones.toml");
/// The Project Brief.
pub(super) const BRIEF: &str = include_str!("../../assets/init/brief.md");
/// The CI workflow.
pub(super) const WORKFLOW: &str = include_str!("../../assets/init/gdd.yml");
/// The release steps, after the commands of the stacks.
const RELEASE: &str = include_str!("../../assets/init/release.toml");
/// The instructions for agents.
const AGENTS: &str = include_str!("../../assets/init/AGENTS.md");
/// Where Claude Code finds the instructions for agents.
const CLAUDE: &str = include_str!("../../assets/init/CLAUDE.md");

/// What the Brief says while the project has no description.
const NO_DESCRIPTION: &str = "(To fill in with the Orchestrator.)";

/// What the Brief says while the project names no sensitive area.
const NO_SENSITIVE: &str = "(None named yet.)";

/// What the output says when the project has its own AGENTS.md, before what to add to it.
const AGENTS_YOURS: &str = "AGENTS.md is yours: add to AGENTS.md a line that sends agents to GDD:";

/// A file of instructions for agents that gdd init writes when the project has none.
pub(super) struct Instructions {
    /// The file, from the project root.
    pub(super) file: &'static str,
    /// Its text.
    pub(super) template: &'static str,
    /// What the output says when the project has its own: what to add to it.
    pub(super) yours: fn() -> String,
}

/// The files of instructions for agents.
pub(super) const INSTRUCTIONS: [Instructions; 2] = [
    Instructions {
        file: "AGENTS.md",
        template: AGENTS,
        yours: agents_yours,
    },
    Instructions {
        file: "CLAUDE.md",
        template: CLAUDE,
        yours: claude_yours,
    },
];

/// What to add to an AGENTS.md of the project's own: the second paragraph of the template.
fn agents_yours() -> String {
    let paragraph = AGENTS.split("\n\n").nth(1).unwrap_or_default();
    format!("{AGENTS_YOURS}\n{}", paragraph.trim())
}

/// What to add to a CLAUDE.md of the project's own: the line of the template.
fn claude_yours() -> String {
    let line = CLAUDE.trim();
    format!("CLAUDE.md is yours: add to CLAUDE.md: {line}")
}

/// What the placeholders of the templates say for a new project.
pub(super) struct Values(Vec<(&'static str, String)>);

impl Values {
    /// The placeholders for the project of `options`, whose stacks have the `sheets` and whose
    /// main branch is `main`: `{version}`, `{main}` (escaped, for it goes between double quotes
    /// in the TOML and the YAML templates), `{name}`, `{description}`, `{platform}`, `{stacks}`,
    /// `{runners}`, `{sensitive}` (a numbered list), `{setup}` (the CI steps of every sheet, in
    /// their order), and the TOML lists `{builder_build}`, `{builder_test}` and `{supply_chain}`
    /// (the paths of every sheet, in their order, each once).
    pub(super) fn new(options: &Options, sheets: &[Sheet], main: &str) -> Values {
        let description = if options.description.is_empty() {
            NO_DESCRIPTION
        } else {
            options.description.as_str()
        };
        let titles: Vec<&str> = sheets.iter().map(Sheet::title).collect();
        let union = |paths: fn(&Sheet) -> &[String]| {
            let each_once = once(sheets.iter().flat_map(paths));
            list(each_once.as_slice())
        };
        Values(vec![
            ("version", VERSION.to_string()),
            ("main", quoted(main)),
            ("name", options.name.clone()),
            ("description", description.to_string()),
            ("platform", options.platform.clone()),
            ("stacks", titles.join(", ")),
            ("runners", runners(sheets)),
            ("sensitive", sensitive(&options.sensitive)),
            ("setup", sheets.iter().map(Sheet::ci_setup).collect()),
            ("builder_build", union(Sheet::builder_build)),
            ("builder_test", union(Sheet::builder_test)),
            ("supply_chain", union(Sheet::supply_chain)),
        ])
    }

    /// `template` with each placeholder filled, in one pass: what a value brings in is never
    /// filled again.
    pub(super) fn fill(&self, template: &str) -> String {
        let pairs = self.0.iter().map(|(key, value)| (*key, value.as_str()));
        let values: Vec<(&str, &str)> = pairs.collect();
        fill(template, &values)
    }
}

/// `text` as it goes between double quotes, in a TOML basic string and in a YAML double-quoted
/// one alike: with `\` and `"` escaped. Git refuses the other characters these would need to
/// escape in a branch name.
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The scenario runners of `sheets`, joined with ", ".
fn runners(sheets: &[Sheet]) -> String {
    let runners: Vec<&str> = sheets.iter().map(Sheet::scenario_runner).collect();
    runners.join(", ")
}

/// The sensitive `areas` as a numbered list, one per line; [`NO_SENSITIVE`] when there is none.
fn sensitive(areas: &[String]) -> String {
    if areas.is_empty() {
        return NO_SENSITIVE.to_string();
    }
    let line = |(at, area): (usize, &String)| format!("{}. {area}", at + 1);
    let lines: Vec<String> = areas.iter().enumerate().map(line).collect();
    lines.join("\n")
}

/// `gdd/project.toml` for the project of `options`, whose `stacks` have the `sheets`.
pub(super) fn project(options: &Options, stacks: &[String], sheets: &[Sheet]) -> String {
    let language = entry("language", &options.language);
    let platform = entry("platform", &options.platform);
    let stack = list(stacks);
    let written = format!("written by gdd init {VERSION}");
    [
        format!("# Project basics · GDD ({written})"),
        String::new(),
        entry("name", &options.name),
        entry("description", &options.description),
        format!("{language}   # language the agents use with the human"),
        format!("{platform}   # {}", PLATFORMS.join(" | ")),
        format!("stack = {stack}   # the stack sheets (gdd stacks)"),
        entry("scenario_runner", &runners(sheets)),
        String::new(),
    ]
    .join("\n")
}

/// `gdd/commands.toml` for the `stacks`: what `gdd stacks compose` prints for them, then the
/// release steps.
pub(super) fn commands(stacks: &[String]) -> Result<String, String> {
    let composed = stacks::compose(stacks).map_err(|e| e.to_string())?;
    Ok(format!("{composed}\n{RELEASE}"))
}

/// `gdd/generated.toml`: the version of gdd, the `files` it writes once and, in the table
/// `[answers]`, what gdd init was asked.
pub(super) fn generated(files: &[&str], answers: &Answers) -> Result<String, String> {
    let comment = "# Written by gdd init: the files it writes once and what it was asked, for gdd \
                   upgrade.";
    let version = entry("version", VERSION);
    let answers = toml::to_string(answers);
    let answers = answers.map_err(|e| format!("cannot write the answers: {e}"))?;
    let head = format!("{comment}\n{version}\nfiles = {}\n", list(files));
    Ok(format!("{head}\n[answers]\n{answers}"))
}

/// `<key> = <value>`, the value written as TOML text.
fn entry(key: &str, value: &str) -> String {
    format!("{key} = {}", Value::String(value.to_string()))
}

/// `items` written as a TOML list of text.
fn list<S: AsRef<str>>(items: &[S]) -> String {
    let text = |item: &S| Value::String(item.as_ref().to_string());
    Value::Array(items.iter().map(text).collect()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheets(names: &[&str]) -> Vec<Sheet> {
        let sheet = |name: &&str| stacks::sheet(name).unwrap();
        names.iter().map(sheet).collect()
    }

    fn options() -> Options {
        Options {
            name: "Notes {main}".to_string(),
            platform: "cli".to_string(),
            stacks: vec!["rust".to_string()],
            agents: Vec::new(),
            language: "es".to_string(),
            description: String::new(),
            sensitive: Vec::new(),
            accept_same_family: false,
        }
    }

    /// The templates filled for a project of Rust and TypeScript, on the branch `trunk`.
    fn filled(template: &str) -> String {
        let mut options = options();
        options.sensitive = vec!["Losing notes".into(), "Leaking notes".into()];
        options.description = "A notes app".into();
        let values = Values::new(&options, &sheets(&["rust", "typescript"]), "trunk");
        values.fill(template)
    }

    #[test]
    fn the_brief_says_what_the_project_is_in_words() {
        let brief = filled(BRIEF);
        let start = "# Project Brief · Notes {main}\n";
        assert!(brief.starts_with(start), "{brief}");
        assert!(brief.contains("\nA notes app\n"), "{brief}");
        assert!(brief.contains("- Platform: cli.\n"), "{brief}");
        let technology = "- Technology: Rust, TypeScript. Scenarios run with cucumber-rs, \
                          cucumber-js.\n";
        assert!(brief.contains(technology), "{brief}");
        let sensitive = "\n1. Losing notes\n2. Leaking notes\n";
        assert!(brief.ends_with(sensitive), "{brief}");
    }

    #[test]
    fn the_rules_and_zones_protect_the_paths_of_every_stack() {
        let rules = filled(RULES);
        let version = format!("(written by gdd init {VERSION})\n");
        assert!(rules.contains(&version), "{rules}");
        assert!(rules.contains("main_branch = \"trunk\"\n"), "{rules}");
        let build = "builder_build = [\"features/**\", \"tests/acceptance/**\", \
                     \"tests/acceptance.rs\"]   #";
        assert!(rules.contains(build), "{rules}");
        let test = "builder_test = [\"src/**\", \"crates/*/src/**\"]   #";
        assert!(rules.contains(test), "{rules}");
        let zones = filled(ZONES);
        let supply_chain = "paths = [\"Cargo.lock\", \"deny.toml\", \"rust-toolchain.toml\", \
                            \"package-lock.json\", \"package.json\", \".npmrc\", \
                            \"osv-scanner.toml\", \"osv-scanner-licenses.toml\"]\n";
        assert!(zones.contains(supply_chain), "{zones}");
        for text in [&rules, &zones] {
            assert!(toml::from_str::<toml::Table>(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn other_braces_stay_as_they_are() {
        let workflow = filled(WORKFLOW);
        let branches = "branches: [\"trunk\", \"change/**\"]";
        assert!(workflow.contains(branches), "{workflow}");
        let gitleaks = "gitleaks_${V}_linux_x64.tar.gz";
        assert!(workflow.contains(gitleaks), "{workflow}");
        let install = format!("--tag v{VERSION} gdd\n");
        assert!(workflow.contains(&install), "{workflow}");
        assert!(workflow.ends_with("\n        run: gdd ci\n"), "{workflow}");
    }

    #[test]
    fn the_workflow_sets_up_the_tools_of_every_stack_in_order() {
        let workflow = filled(WORKFLOW);
        let both = sheets(&["rust", "typescript"]);
        let (rust, typescript) = (both[0].ci_setup(), both[1].ci_setup());
        let checkout = "          fetch-depth: 0\n";
        let gitleaks = "      - name: Install gitleaks";
        let steps = format!("{checkout}{rust}{typescript}{gitleaks}");
        assert!(workflow.contains(&steps), "{workflow}");
        assert!(!workflow.contains("{setup}"), "{workflow}");
    }

    #[test]
    fn a_branch_with_a_quote_keeps_its_name_in_the_rules_and_the_workflow() {
        let branch = "release\"candidate";
        let values = Values::new(&options(), &sheets(&["rust"]), branch);
        let rules: toml::Table = toml::from_str(&values.fill(RULES)).unwrap();
        assert_eq!(rules["main_branch"].as_str(), Some(branch));
        let workflow = values.fill(WORKFLOW);
        let (_, after) = workflow.split_once("branches: ").unwrap();
        let (flow, _) = after.split_once('\n').unwrap();
        // A YAML flow list of double-quoted text reads as JSON.
        let branches: Vec<String> = serde_json::from_str(flow).unwrap();
        assert_eq!(branches, [branch, "change/**"]);
        assert_eq!(quoted(r#"a\"b"#), r#"a\\\"b"#);
    }

    #[test]
    fn without_a_description_or_sensitive_areas_the_brief_says_so() {
        let values = Values::new(&options(), &sheets(&["python"]), "main");
        let brief = values.fill(BRIEF);
        let none = "\n(To fill in with the Orchestrator.)\n";
        assert!(brief.contains(none), "{brief}");
        assert!(brief.ends_with("\n(None named yet.)\n"), "{brief}");
        let technology = "Technology: Python. Scenarios run with pytest-bdd.\n";
        assert!(brief.contains(technology), "{brief}");
    }

    #[test]
    fn the_project_file_says_the_basics() {
        let mut options = options();
        options.description = "Say \"hi\"".into();
        let stacks = ["rust".to_string(), "typescript".to_string()];
        let text = project(&options, &stacks, &sheets(&["rust", "typescript"]));
        let table: toml::Table = toml::from_str(&text).unwrap();
        let text_of = |key: &str| table[key].as_str().unwrap().to_string();
        assert_eq!(text_of("name"), "Notes {main}");
        assert_eq!(text_of("description"), "Say \"hi\"");
        assert_eq!(text_of("language"), "es");
        assert_eq!(text_of("platform"), "cli");
        assert_eq!(text_of("scenario_runner"), "cucumber-rs, cucumber-js");
        let stack = Value::Array(vec!["rust".into(), "typescript".into()]);
        assert_eq!(table["stack"], stack);
        assert!(text.starts_with("# Project basics · GDD "), "{text}");
        let written = format!("(written by gdd init {VERSION})\n\n");
        assert!(text.contains(&written), "{text}");
        let platforms = "   # mobile | desktop | web | cli | other\n";
        assert!(text.contains(platforms), "{text}");
        assert!(text.ends_with("\"\n"), "{text}");
    }

    #[test]
    fn the_commands_are_the_composed_sheets_then_the_release_steps() {
        let stacks = ["rust".to_string()];
        let text = commands(&stacks).unwrap();
        let composed = stacks::compose(&stacks).unwrap();
        assert!(text.starts_with(&composed), "{text}");
        let release = "\"]\n\n[release]\nsimulated = true";
        assert!(text.contains(release), "{text}");
        let table: toml::Table = toml::from_str(&text).unwrap();
        let unit = table["g2"]["unit"].as_str();
        assert_eq!(unit, Some("cargo test --workspace"));
        let unknown = commands(&["cobol".to_string()]).unwrap_err();
        let said = "unknown stack cobol (known: python, rust, typescript)";
        assert_eq!(unknown, said);
    }

    fn answers() -> Answers {
        Answers {
            name: "Say \"hi\" {main}".to_string(),
            description: "It's 'quoted'\nover two lines".to_string(),
            platform: "cli".to_string(),
            stacks: vec!["rust".to_string()],
            agents: vec!["claude".to_string(), "codex".to_string()],
            language: "en".to_string(),
            sensitive: Vec::new(),
            accept_same_family: false,
            day: "2026-10-28".to_string(),
            main: "main".to_string(),
        }
    }

    #[test]
    fn the_generated_file_names_the_version_the_files_written_once_and_the_answers() {
        let text = generated(&["gdd/a.toml", "docs/b.md"], &answers()).unwrap();
        let table: toml::Table = toml::from_str(&text).unwrap();
        assert_eq!(table["version"].as_str(), Some(VERSION));
        let files = Value::Array(vec!["gdd/a.toml".into(), "docs/b.md".into()]);
        assert_eq!(table["files"], files);
        let comment = "# Written by gdd init: the files it writes once and what it was asked, \
                       for gdd upgrade.\nversion = ";
        assert!(text.starts_with(comment), "{text}");
        let after = "\"docs/b.md\"]\n\n[answers]\n";
        assert!(text.contains(after), "{text}");
        // Read back, the answers are what was written, quotes and all.
        let read: Answers = table["answers"].clone().try_into().unwrap();
        assert_eq!(read, answers());
        let agents = Value::Array(vec!["claude".into(), "codex".into()]);
        let written = &table["answers"];
        assert_eq!(written["agents"], agents);
        assert_eq!(written["accept_same_family"], Value::Boolean(false));
    }

    #[test]
    fn the_project_s_own_instructions_get_what_to_add() {
        let [agents, claude] = &INSTRUCTIONS;
        assert_eq!((agents.file, claude.file), ("AGENTS.md", "CLAUDE.md"));
        let title = "# Instructions for agents\n";
        assert!(agents.template.starts_with(title));
        assert_eq!(claude.template, "@AGENTS.md\n");
        let expected = "AGENTS.md is yours: add to AGENTS.md a line that sends agents to GDD:\n\
                        This project works with **GDD · Gate-Driven Development**: work moves \
                        forward by passing gates, and\nrisk decides how many gates and how \
                        strict. `gdd` runs the gates.";
        assert_eq!((agents.yours)(), expected);
        let claude_md = "CLAUDE.md is yours: add to CLAUDE.md: @AGENTS.md";
        assert_eq!((claude.yours)(), claude_md);
    }
}
