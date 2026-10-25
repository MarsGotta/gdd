//! The `gdd` command line: it reads the words of a command, runs it on the project and says
//! what happened. The method itself lives in `gdd-core`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use gdd_core::change::{self, Decision};
use gdd_core::{Report, RulesTable, ZoneMap, log};

/// File that marks the root of a GDD project.
const PROJECT: &str = "gdd/project.toml";
/// Zone map of the project.
const ZONES: &str = "gdd/zones.toml";
/// Rules table of the project.
const RULES: &str = "gdd/rules.toml";

/// What a run of `gdd` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Exit code: 0 when the command did its job, 1 when it failed.
    pub code: u8,
    /// What it printed for the user.
    pub stdout: String,
    /// Why it failed.
    pub stderr: String,
}

impl Outcome {
    fn success(stdout: String) -> Outcome {
        Outcome {
            code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(stderr: String) -> Outcome {
        Outcome {
            code: 1,
            stdout: String::new(),
            stderr,
        }
    }
}

/// GDD · Gate-Driven Development: phases, gates and coding agents, from the terminal.
#[derive(Debug, Parser)]
#[command(name = "gdd", version = gdd_core::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Tokens, cost and time of the agent calls in the log.
    Report {
        /// Fields to group the calls by, separated by commas (change, phase, model, role, agent...).
        #[arg(long, default_value = "role")]
        by: String,
        /// Count only the records of this change.
        #[arg(long)]
        change: Option<String>,
        /// Print the groups as a JSON list.
        #[arg(long)]
        json: bool,
    },
    /// Color, lane and gates of a planned change, as JSON.
    Classify {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
}

impl Command {
    fn run(self, root: &Path) -> Result<String, String> {
        match self {
            Command::Report { by, change, json } => report(root, &by, change.as_deref(), json),
            Command::Classify { change } => classify(root, &change),
        }
    }
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`.
/// Help and version go to the standard output; every error goes to the standard error, with
/// exit code 1.
pub fn run(args: &[String], cwd: &Path) -> Outcome {
    let words = std::iter::once("gdd").chain(args.iter().map(String::as_str));
    let cli = match Cli::try_parse_from(words) {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => return Outcome::failure(e.to_string()),
        Err(e) => return Outcome::success(e.to_string()),
    };
    let Some(command) = cli.command else {
        return Outcome::success(Cli::command().render_help().to_string());
    };
    match project_root(cwd).and_then(|root| command.run(&root)) {
        Ok(text) => Outcome::success(format!("{text}\n")),
        Err(message) => Outcome::failure(format!("error: {message}\n")),
    }
}

/// The root of the project: the nearest folder, from `cwd` upwards, with `gdd/project.toml`.
fn project_root(cwd: &Path) -> Result<PathBuf, String> {
    cwd.ancestors()
        .find(|folder| folder.join(PROJECT).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "not a GDD project: no {PROJECT} in {} or the folders above it",
                cwd.display()
            )
        })
}

/// `gdd report`: the agent calls of the log added up by the fields `by`.
fn report(root: &Path, by: &str, change: Option<&str>, json: bool) -> Result<String, String> {
    let text = read(root, log::PATH)?.unwrap_or_default();
    let records = log::parse(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let by: Vec<String> = by.split(',').map(str::to_string).collect();
    let report = Report::new(&records, &by, change);
    if json {
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
    } else {
        Ok(report.to_string())
    }
}

/// `gdd classify`: the decision for a planned change, from its record, the zone map and the
/// rules table. Only the planned paths count. A missing zone map or rules table says nothing.
fn classify(root: &Path, id: &str) -> Result<String, String> {
    if !change::valid_id(id) {
        return Err(format!(
            "invalid change id {id:?}: use lowercase letters, digits and hyphens"
        ));
    }
    let path = change::record_path(id);
    let record = read(root, &path)?
        .ok_or_else(|| format!("change {id} does not exist: there is no {path}"))?;
    let planned = change::from_json(&record).map_err(|e| format!("{path}: {e}"))?;
    let zones = ZoneMap::from_toml(&read(root, ZONES)?.unwrap_or_default())
        .map_err(|e| format!("{ZONES}: {e}"))?;
    let table = RulesTable::from_toml(&read(root, RULES)?.unwrap_or_default())
        .map_err(|e| format!("{RULES}: {e}"))?;
    let classification = gdd_core::classify(&zones, &table, &planned).map_err(|e| e.to_string())?;
    let decision = Decision {
        change: id,
        classification: &classification,
    };
    serde_json::to_string_pretty(&decision).map_err(|e| e.to_string())
}

/// The text of a file of the project, or `None` when it does not exist.
fn read(root: &Path, path: &str) -> Result<Option<String>, String> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A project in a temporary folder, removed when dropped.
    struct Project(PathBuf);

    impl Project {
        fn new() -> Project {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "gdd-cli-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            let project = Project(path);
            project.write(PROJECT, "name = \"demo\"\n");
            project
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn run(&self, command: &str) -> Outcome {
            run_in(&self.0, command)
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn run_in(cwd: &Path, command: &str) -> Outcome {
        let args: Vec<String> = command.split_whitespace().map(str::to_string).collect();
        run(&args, cwd)
    }

    fn json(out: &Outcome) -> serde_json::Value {
        assert_eq!(out.code, 0, "{out:?}");
        serde_json::from_str(&out.stdout).unwrap()
    }

    const LOG: &str = concat!(
        r#"{"change": "a", "role": "builder-build", "model": "opus", "tokens_in": 1200, "tokens_out": 20, "cost_usd": 0.5, "duration_s": 4}"#,
        "\n",
        r#"{"change": "b", "role": "reviewer", "model": "gpt-x", "tokens_in": 10, "tokens_out": 2}"#,
        "\n",
        r#"{"change": "a", "gate": "G1", "role": "human", "result": "ok"}"#,
        "\n"
    );

    #[test]
    fn the_version_names_the_tool() {
        let out = run_in(Path::new("/"), "--version");
        assert_eq!(
            out,
            Outcome::success(format!("{}\n", gdd_core::version_line()))
        );
    }

    #[test]
    fn help_goes_to_the_standard_output() {
        for command in ["--help", "", "report --help"] {
            let out = run_in(Path::new("/"), command);
            assert_eq!((out.code, out.stderr.as_str()), (0, ""), "{command:?}");
            assert!(out.stdout.contains("Usage: gdd"), "{command:?}: {out:?}");
        }
        let out = run_in(Path::new("/"), "");
        assert!(out.stdout.contains("report") && out.stdout.contains("classify"));
    }

    #[test]
    fn a_wrong_command_is_an_error() {
        for command in ["frobnicate", "classify", "report --by", "report --nope"] {
            let out = run_in(Path::new("/"), command);
            assert_eq!((out.code, out.stdout.as_str()), (1, ""), "{command:?}");
            assert!(out.stderr.starts_with("error: "), "{command:?}: {out:?}");
        }
    }

    #[test]
    fn outside_a_project_commands_fail() {
        let folder = Project::new();
        fs::remove_file(folder.0.join(PROJECT)).unwrap();
        let out = folder.run("report");
        assert_eq!(out.code, 1);
        assert!(out.stdout.is_empty());
        assert!(
            out.stderr
                .starts_with("error: not a GDD project: no gdd/project.toml in "),
            "{out:?}"
        );
        assert!(
            out.stderr.ends_with(" or the folders above it\n"),
            "{out:?}"
        );
    }

    #[test]
    fn the_root_is_the_nearest_folder_with_the_project_file() {
        let outer = Project::new();
        outer.write(log::PATH, LOG);
        outer.write("inner/gdd/project.toml", "");
        fs::create_dir_all(outer.0.join("inner/deep/er")).unwrap();
        assert_eq!(
            project_root(&outer.0.join("inner/deep/er")),
            Ok(outer.0.join("inner"))
        );
        assert_eq!(project_root(&outer.0), Ok(outer.0.clone()));
        let out = run_in(&outer.0.join("inner/deep"), "report");
        assert_eq!(out.stdout, "No agent calls in the log yet.\n");
        // A folder named like the project file is not a project.
        fs::create_dir_all(outer.0.join("dir/gdd/project.toml")).unwrap();
        assert_eq!(project_root(&outer.0.join("dir")), Ok(outer.0.clone()));
    }

    #[test]
    fn report_prints_the_text_report() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let out = project.run("report");
        assert_eq!(
            out,
            Outcome::success(
                "role -> calls · tokens in · cache · out · cost USD · seconds\n\
                 builder-build -> 1 · 1,200 · 0 · 20 · 0.5000 · 4\n\
                 reviewer -> 1 · 10 · 0 · 2 · 0.0000 · 0\n\
                 TOTAL -> in 1,210 · cache 0 · out 22 · 0.5000 USD\n\
                 Human stops logged: 1\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn report_groups_by_the_fields_asked_and_one_change() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let out = project.run("report --by change,model --change b");
        assert_eq!(out.code, 0);
        assert!(out.stdout.starts_with("change · model -> calls"), "{out:?}");
        assert!(out.stdout.contains("\nb · gpt-x -> 1 · 10 · 0 · 2 · "));
        assert!(!out.stdout.contains("opus"));
        assert!(out.stdout.ends_with("Human stops logged: 0\n"));
    }

    #[test]
    fn report_as_json() {
        let project = Project::new();
        project.write(log::PATH, LOG);
        let entries = json(&project.run("report --json --by model"));
        assert_eq!(
            entries,
            serde_json::json!([
                {"model": "gpt-x", "calls": 1, "tokens_in": 10, "tokens_cache": 0, "tokens_out": 2, "cost_usd": 0.0, "duration_s": 0.0},
                {"model": "opus", "calls": 1, "tokens_in": 1200, "tokens_cache": 0, "tokens_out": 20, "cost_usd": 0.5, "duration_s": 4.0},
            ])
        );
        assert_eq!(
            json(&project.run("report --json --change zzz")),
            serde_json::json!([])
        );
    }

    #[test]
    fn report_without_a_log_has_no_calls() {
        let project = Project::new();
        let out = project.run("report");
        assert_eq!(
            out,
            Outcome::success("No agent calls in the log yet.\n".into())
        );
    }

    #[test]
    fn a_broken_log_line_is_an_error_that_names_it() {
        let project = Project::new();
        project.write(log::PATH, &format!("{LOG}{{\"tokens_in\": \"x\"}}\n"));
        let out = project.run("report");
        assert_eq!(out.code, 1);
        assert!(out.stdout.is_empty());
        assert!(
            out.stderr
                .starts_with("error: .gdd/log.jsonl, line 4: invalid type"),
            "{out:?}"
        );
    }

    #[test]
    fn a_log_that_cannot_be_read_is_an_error() {
        let project = Project::new();
        fs::create_dir_all(project.0.join(log::PATH)).unwrap();
        let out = project.run("report");
        assert_eq!(out.code, 1);
        assert!(
            out.stderr
                .starts_with("error: cannot read .gdd/log.jsonl: "),
            "{out:?}"
        );
    }

    const ZONE_MAP: &str = "default = \"yellow\"\n[[zone]]\ncolor = \"red\"\npaths = [\"src/storage/**\"]\nreason = \"Storage\"\n";

    fn project_with_change(record: &str) -> Project {
        let project = Project::new();
        project.write(ZONES, ZONE_MAP);
        project.write(".gdd/changes/save/change.json", record);
        project
    }

    #[test]
    fn classify_prints_the_decision_as_json() {
        let project = project_with_change(
            r#"{"id": "save", "paths": ["src/storage/a.rs", "docs/x.md"], "resumable": true, "min_color": null}"#,
        );
        project.write(RULES, "[color.red.full]\nmutation = false\n");
        let out = project.run("classify save");
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            out.stdout
                .starts_with("{\n  \"change\": \"save\",\n  \"color\": \"red\",\n")
        );
        assert!(out.stdout.ends_with("}\n"));
        assert_eq!(
            json(&out),
            serde_json::json!({
                "change": "save",
                "color": "red",
                "lane": "full",
                "reasons": [
                    "docs/x.md -> yellow (no zone: default color)",
                    "src/storage/a.rs -> red (Storage)",
                ],
                "g1": false,
                "features": [],
                "tests_first": false,
                "mutation": false,
                "reviewer": true,
                "g4": true,
                "human_plan": true,
            })
        );
    }

    #[test]
    fn classify_from_a_subfolder_with_a_minimum_color() {
        let project = project_with_change(r#"{"paths": ["docs/x.md"], "min_color": "red"}"#);
        let cwd = project.0.join("docs/deep");
        fs::create_dir_all(&cwd).unwrap();
        let decision = json(&run_in(&cwd, "classify save"));
        assert_eq!(decision["color"], "red");
        assert_eq!(decision["reasons"][1], "declared minimum color: red");
    }

    #[test]
    fn classify_without_zone_map_or_rules_takes_the_defaults() {
        let project = Project::new();
        project.write(
            ".gdd/changes/save/change.json",
            r#"{"paths": ["a.rs"], "resumable": true}"#,
        );
        let decision = json(&project.run("classify save"));
        assert_eq!(decision["color"], "yellow");
        assert_eq!(decision["lane"], "fast");
        assert_eq!(decision["tests_first"], false);
        assert_eq!(decision["mutation"], false);
        assert_eq!(decision["reviewer"], true);
    }

    #[test]
    fn classify_an_unknown_change_names_it() {
        let project = Project::new();
        let out = project.run("classify nope");
        assert_eq!(
            out,
            Outcome::failure(
                "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n"
                    .into()
            )
        );
    }

    #[test]
    fn classify_rejects_ids_that_are_not_change_ids() {
        let project = Project::new();
        project.write("x/change.json", "{}");
        for id in ["../x", "Save", "a/b"] {
            let out = project.run(&format!("classify {id}"));
            assert_eq!(
                out,
                Outcome::failure(format!(
                    "error: invalid change id {id:?}: use lowercase letters, digits and hyphens\n"
                ))
            );
        }
    }

    #[test]
    fn classify_names_the_file_that_is_wrong() {
        let project = project_with_change(r#"{"min_color": "purple"}"#);
        let out = project.run("classify save");
        assert_eq!(out.code, 1);
        assert!(
            out.stderr
                .starts_with("error: .gdd/changes/save/change.json: unknown color \"purple\""),
            "{out:?}"
        );

        let project = project_with_change("{}");
        project.write(ZONES, "default = \"blue\"\n");
        let out = project.run("classify save");
        assert_eq!(
            out.stderr,
            "error: gdd/zones.toml: unknown color \"blue\" (use green, yellow or red)\n"
        );

        let project = project_with_change("{}");
        project.write(RULES, "[color.purple]\n");
        let out = project.run("classify save");
        assert_eq!(
            out.stderr,
            "error: gdd/rules.toml: unknown color \"purple\" (use green, yellow or red)\n"
        );
    }

    #[test]
    fn classify_a_record_that_cannot_be_read_is_an_error() {
        let project = Project::new();
        fs::create_dir_all(project.0.join(".gdd/changes/save/change.json")).unwrap();
        let out = project.run("classify save");
        assert!(
            out.stderr
                .starts_with("error: cannot read .gdd/changes/save/change.json: "),
            "{out:?}"
        );
    }
}
