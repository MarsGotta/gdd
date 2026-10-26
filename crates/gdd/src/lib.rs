//! The `gdd` command line: it reads the words of a command, runs it on the project and says
//! what happened. The method itself lives in `gdd-core`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use gdd_core::change::{self, Decision, Record};
use gdd_core::guard::{self, Hashes, Settings, Verdict};
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
    /// Color, lane and gates of a change, from its plan and what it touched, as JSON.
    Classify {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Path Guard: what a change touched since its build started that a role must not touch,
    /// as JSON.
    Guard {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
        /// Role whose protected paths count (builder-build, builder-test, reviewer...).
        #[arg(long, default_value = "builder")]
        role: String,
        /// Exit with code 1 when Path Guard fails.
        #[arg(long)]
        strict: bool,
        /// Count only what changed since the last `gdd snapshot`.
        #[arg(long)]
        since_snapshot: bool,
    },
    /// Records what a change touched as it is now, so `gdd guard --since-snapshot` judges only
    /// what changes after.
    Snapshot {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
    /// Paths a change touched outside its plan: accepted when their color is not higher than
    /// the change's, as JSON.
    Scope {
        /// Id of the change: its record is .gdd/changes/<id>/change.json.
        change: String,
    },
}

impl Command {
    /// Runs the command on the project at `root`: what it prints and its exit code.
    fn run(self, root: &Path) -> Result<(String, u8), String> {
        let mut code = 0;
        let text = match self {
            Command::Report { by, change, json } => report(root, &by, change.as_deref(), json)?,
            Command::Classify { change } => classify(root, &change)?,
            Command::Guard {
                change,
                role,
                strict,
                since_snapshot,
            } => {
                let verdict = path_guard(root, &change, &role, since_snapshot)?;
                code = u8::from(strict && !verdict.ok);
                serde_json::to_string(&verdict).map_err(|e| e.to_string())?
            }
            Command::Snapshot { change } => snapshot(root, &change)?,
            Command::Scope { change } => scope(root, &change)?,
        };
        Ok((text, code))
    }
}

/// Runs `gdd` with `args`, the words after the program name, from the folder `cwd`.
/// Help and version go to the standard output; every error goes to the standard error, with
/// exit code 1. `gdd guard --strict` also exits with 1 when Path Guard fails.
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
        Ok((text, code)) => Outcome {
            code,
            stdout: format!("{text}\n"),
            stderr: String::new(),
        },
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

/// `gdd classify`: the decision for a change, from its record, the zone map and the rules
/// table. Its planned paths count, and the ones it touched in git (see [`guard::touched`]). A
/// missing zone map or rules table says nothing.
fn classify(root: &Path, id: &str) -> Result<String, String> {
    let record = load_record(root, id)?;
    let zones = load_zones(root)?;
    let table = load_table(root)?;
    let touched = guard::touched(root, id, &load_settings(root)?).map_err(|e| e.to_string())?;
    let mut plan = record.plan;
    plan.paths.extend(touched);
    let classification = gdd_core::classify(&zones, &table, &plan).map_err(|e| e.to_string())?;
    let decision = Decision {
        change: id,
        classification: &classification,
    };
    serde_json::to_string_pretty(&decision).map_err(|e| e.to_string())
}

/// `gdd guard`: Path Guard for `role` on change `id`, with the protected paths of the rules
/// table. With `since_snapshot`, only what changed since the last snapshot counts, if there is
/// one.
fn path_guard(root: &Path, id: &str, role: &str, since_snapshot: bool) -> Result<Verdict, String> {
    let record = load_record(root, id)?;
    let settings = load_settings(root)?;
    let before = if since_snapshot {
        last_snapshot(root, id)?
    } else {
        None
    };
    guard::path_guard(root, id, &record, &settings, role, before.as_ref())
        .map_err(|e| e.to_string())
}

/// `gdd snapshot`: records the hashes of what change `id` touched since its build started.
fn snapshot(root: &Path, id: &str) -> Result<String, String> {
    let record = load_record(root, id)?;
    let hashes = guard::snapshot(root, id, &record).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&hashes).map_err(|e| e.to_string())?;
    let path = change::snapshot_path(id);
    fs::write(root.join(&path), format!("{text}\n"))
        .map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(r#"{"ok":true}"#.to_string())
}

/// `gdd scope`: the scope check of change `id`, with the zone map and the rules table.
fn scope(root: &Path, id: &str) -> Result<String, String> {
    let record = load_record(root, id)?;
    let zones = load_zones(root)?;
    let table = load_table(root)?;
    let checked = guard::scope(root, id, &record, &zones, &table).map_err(|e| e.to_string())?;
    serde_json::to_string(&checked).map_err(|e| e.to_string())
}

/// The record of change `id`. An id that cannot name a change, or a change without a record,
/// is an error.
fn load_record(root: &Path, id: &str) -> Result<Record, String> {
    if !change::valid_id(id) {
        return Err(format!(
            "invalid change id {id:?}: use lowercase letters, digits and hyphens"
        ));
    }
    let path = change::record_path(id);
    let text = read(root, &path)?
        .ok_or_else(|| format!("change {id} does not exist: there is no {path}"))?;
    Record::from_json(&text).map_err(|e| format!("{path}: {e}"))
}

/// The hashes of the last snapshot of change `id`; `None` when there is none.
fn last_snapshot(root: &Path, id: &str) -> Result<Option<Hashes>, String> {
    let path = change::snapshot_path(id);
    match read(root, &path)? {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{path}: {e}")),
        None => Ok(None),
    }
}

/// The zone map of the project.
fn load_zones(root: &Path) -> Result<ZoneMap, String> {
    let text = read(root, ZONES)?.unwrap_or_default();
    ZoneMap::from_toml(&text).map_err(|e| format!("{ZONES}: {e}"))
}

/// The rules table of the project.
fn load_table(root: &Path) -> Result<RulesTable, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    RulesTable::from_toml(&text).map_err(|e| format!("{RULES}: {e}"))
}

/// What the rules file says to Path Guard: the main branch and the protected paths.
fn load_settings(root: &Path) -> Result<Settings, String> {
    let text = read(root, RULES)?.unwrap_or_default();
    Settings::from_toml(&text).map_err(|e| format!("{RULES}: {e}"))
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
        for command in ["report", "classify", "guard", "snapshot", "scope"] {
            assert!(out.stdout.contains(command), "{command}: {out:?}");
        }
    }

    #[test]
    fn a_wrong_command_is_an_error() {
        for command in [
            "frobnicate",
            "classify",
            "report --by",
            "report --nope",
            "guard",
            "guard save --role",
            "snapshot",
            "scope",
        ] {
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

    /// Git settings of the tests: a fixed author, and no hooks or signing from the user's config.
    const GIT_CONFIG: [&str; 8] = [
        "-c",
        "user.name=GDD Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];

    fn git(project: &Project, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(GIT_CONFIG)
            .args(args)
            .current_dir(&project.0)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "git {args:?}: {stderr}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    const PROTECTED: &str = "[protected]\nall = [\"gdd/**\"]\nbuilder_test = [\"src/**\"]\n";

    /// A project in a git repository: its files on `main`, then change `save`, which plans
    /// `src/a.rs`, started on its branch.
    fn started_project() -> Project {
        let project = Project::new();
        project.write(RULES, PROTECTED);
        project.write(ZONES, ZONE_MAP);
        git(&project, &["init", "-q", "-b", "main"]);
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "project"]);
        git(&project, &["checkout", "-q", "-b", "change/save"]);
        let start = git(&project, &["rev-parse", "HEAD"]);
        let record = format!(r#"{{"paths": ["src/a.rs"], "build_start": "{start}"}}"#);
        project.write(".gdd/changes/save/change.json", &record);
        project
    }

    #[test]
    fn classify_counts_the_paths_the_change_touched() {
        let project = started_project();
        assert_eq!(json(&project.run("classify save"))["color"], "yellow");
        project.write("src/storage/db.rs", "x\n");
        let decision = json(&project.run("classify save"));
        assert_eq!(decision["color"], "red");
        assert_eq!(
            decision["reasons"],
            serde_json::json!([
                "src/a.rs -> yellow (no zone: default color)",
                "src/storage/db.rs -> red (Storage)",
            ])
        );
    }

    #[test]
    fn guard_prints_its_verdict_and_fails_the_run_only_when_strict() {
        let project = started_project();
        let out = project.run("guard save --role builder-test --strict");
        assert_eq!(out.code, 0);
        assert_eq!(out.stdout, "{\"ok\":true,\"failures\":[]}\n");
        project.write("src/a.rs", "x\n");
        project.write(ZONES, "default = \"red\"\n");
        let verdict = serde_json::json!({
            "ok": false,
            "failures": [
                "gdd/zones.toml: protected path for builder-test",
                "src/a.rs: protected path for builder-test",
            ],
        });
        assert_eq!(
            json(&project.run("guard save --role builder-test")),
            verdict
        );
        let strict = project.run("guard save --role builder-test --strict");
        assert_eq!((strict.code, strict.stderr.as_str()), (1, ""));
        let printed: serde_json::Value = serde_json::from_str(&strict.stdout).unwrap();
        assert_eq!(printed, verdict);
        let builder = json(&project.run("guard save"));
        assert_eq!(
            builder["failures"],
            serde_json::json!(["gdd/zones.toml: protected path for builder"])
        );
    }

    #[test]
    fn snapshot_records_the_round_and_guard_then_judges_only_what_follows() {
        let project = started_project();
        project.write("src/a.rs", "1\n");
        let out = project.run("snapshot save");
        assert_eq!(out, Outcome::success("{\"ok\":true}\n".into()));
        let path = project.0.join(".gdd/changes/save/snapshot.json");
        let saved = fs::read_to_string(path).unwrap();
        let hash = guard::file_hash(&project.0.join("src/a.rs")).unwrap();
        assert_eq!(saved, format!("{{\n  \"src/a.rs\": \"{hash}\"\n}}\n"));
        project.write("src/b.rs", "1\n");
        let since = json(&project.run("guard save --role builder-test --since-snapshot"));
        assert_eq!(
            since["failures"],
            serde_json::json!(["src/b.rs: protected path for builder-test"])
        );
        let all = json(&project.run("guard save --role builder-test"));
        assert_eq!(all["failures"].as_array().map(Vec::len), Some(2));
        project.write(".gdd/changes/save/snapshot.json", "{not json");
        let out = project.run("guard save --role builder-test --since-snapshot");
        let expected = "error: .gdd/changes/save/snapshot.json: ";
        assert_eq!(out.code, 1);
        assert!(out.stderr.starts_with(expected), "{out:?}");
        assert_eq!(project.run("guard save --role builder-test").code, 0);
    }

    #[test]
    fn scope_prints_its_verdict_and_never_fails_the_run() {
        let project = started_project();
        let empty = serde_json::json!({"ok": true, "failures": [], "accepted": []});
        assert_eq!(json(&project.run("scope save")), empty);
        project.write("docs/x.md", "x\n");
        project.write("src/storage/db.rs", "x\n");
        let out = project.run("scope save");
        assert_eq!(out.code, 0);
        assert!(out.stdout.starts_with("{\"ok\":false,"), "{out:?}");
        let failure = "src/storage/db.rs: outside the package and red (the change is yellow); \
                       remove it, or the Orchestrator widens the scope";
        let checked = serde_json::json!({
            "ok": false,
            "failures": [failure],
            "accepted": ["docs/x.md"],
        });
        assert_eq!(json(&out), checked);
    }

    #[test]
    fn a_change_never_prepared_fails_guard_and_scope_and_has_no_snapshot() {
        let project = project_with_change(r#"{"paths": ["a.rs"]}"#);
        let guarded = project.run("guard save --strict");
        assert_eq!(guarded.code, 1);
        let printed: serde_json::Value = serde_json::from_str(&guarded.stdout).unwrap();
        let failure = "Change save has not been prepared: Path Guard has no starting point.";
        assert_eq!(
            printed,
            serde_json::json!({"ok": false, "failures": [failure]})
        );
        assert_eq!(json(&project.run("scope save"))["ok"], false);
        assert_eq!(
            project.run("snapshot save"),
            Outcome::failure(
                "error: change save has not been prepared: its record has no build_start\n".into()
            )
        );
    }

    #[test]
    fn guard_scope_and_snapshot_need_a_change_that_exists() {
        let project = Project::new();
        let invalid =
            "error: invalid change id \"Save\": use lowercase letters, digits and hyphens\n";
        let missing =
            "error: change nope does not exist: there is no .gdd/changes/nope/change.json\n";
        for command in ["guard", "scope", "snapshot"] {
            let out = project.run(&format!("{command} Save"));
            assert_eq!(out, Outcome::failure(invalid.into()), "{command}");
            let out = project.run(&format!("{command} nope"));
            assert_eq!(out, Outcome::failure(missing.into()), "{command}");
        }
    }

    #[test]
    fn a_git_failure_fails_the_run() {
        let project = project_with_change(r#"{"paths": [], "build_start": "abc123"}"#);
        let expected = "error: git rev-parse --abbrev-ref HEAD failed";
        for command in ["guard save --strict", "scope save", "snapshot save"] {
            let out = project.run(command);
            assert_eq!((out.code, out.stdout.as_str()), (1, ""), "{command}");
            assert!(out.stderr.starts_with(expected), "{command}: {out:?}");
        }
    }

    #[test]
    fn guard_names_a_rules_file_that_is_wrong() {
        let project = started_project();
        project.write(RULES, "[protected]\nall = \"gdd/**\"\n");
        for command in ["guard save", "classify save"] {
            let out = project.run(command);
            assert_eq!(out.code, 1, "{command}");
            assert!(out.stderr.starts_with("error: gdd/rules.toml: "), "{out:?}");
        }
    }
}
