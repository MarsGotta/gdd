//! Helpers for the integration tests: throwaway git repositories and the built binary.
#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const BIN: &str = env!("CARGO_BIN_EXE_gdd");

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub struct Run {
    pub code: i32,
    pub out: String,
    pub facts: Value,
}

impl Run {
    pub fn has(&self, s: &str) -> bool {
        self.out.contains(s)
    }
}

/// Keep a command's git to the repository's own configuration: no test passes or fails because
/// of what the machine it runs on has set for its user, starting with a name and an email.
pub fn own_config(c: &mut Command) -> &mut Command {
    c.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
}

pub fn run_gdd(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut c = Command::new(BIN);
    c.arg("--root").arg(root).args(args).stdin(Stdio::null());
    own_config(&mut c);
    c.env_remove("GDD_NOW")
        .env_remove("GDD_ENV")
        .env_remove("GDD_MAIN")
        .env_remove("CI");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("gdd runs");
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    let last = out.lines().last().unwrap_or("");
    let facts = last
        .strip_prefix("gdd ")
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or(Value::Null);
    Run {
        code: o.status.code().unwrap_or(-1),
        out,
        facts,
    }
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let mut c = Command::new("git");
    c.arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stdin(Stdio::null());
    let o = own_config(&mut c).output().unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

/// Take away (`a-w`) or give back (`u+w`) the right to write under a folder. Says whether taking
/// it away holds: for a user that writes whatever the permissions say, such as root in a container,
/// it doesn't, and a test of what happens where git can't write has nothing to try there.
pub fn lock(dir: &Path, mode: &str) -> bool {
    assert!(
        Command::new("chmod")
            .args(["-R", mode])
            .arg(dir)
            .status()
            .unwrap()
            .success()
    );
    let probe = dir.join("gdd-test-probe");
    if mode == "a-w" && std::fs::write(&probe, "").is_ok() {
        let _ = std::fs::remove_file(&probe);
        eprintln!("skipped: this user writes whatever the permissions say");
        return false;
    }
    true
}

pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args).stdin(Stdio::null());
    own_config(&mut c).output().unwrap().status.success()
}

pub const BLUEPRINT: &str = r#"---
contract: gdd/1
guarantee: rule
levels:
  fake:
    launch: the Agent tool, with its model field
    light: model-a
    standard: model-b
    deep: model-c
limits:
  lines_per_change: 400
  seconds_each_round: 120
  seconds_before_review: 600
checks:
  - id: seal
    type: machine
    command: "git diff --exit-code <start> HEAD -- gdd features AGENTS.md <sealed paths> ':(exclude)gdd/log' ':(exclude)gdd/backlog.md'"
    runs: each round
  - id: tests
    type: machine
    command: sh tests/run.sh
    runs: each round
  - id: slow
    type: machine
    command: sh -c 'echo the slow check ran'
    runs: before review
    when: [':(glob)src/ui/**']
  - id: review
    type: reviewer
  - id: brief-approval
    type: person
  - id: person-review
    type: person
colors:
  green:
    checks: [seal, tests, slow, review]
    level: light
    limits:
      minutes_per_change: 600
      rounds_per_change: 1
      fix_rounds_per_gate: 1
      runs_before_review: 1
  yellow:
    checks: [brief-approval, seal, tests, slow, review]
    level: standard
    limits:
      minutes_per_change: 600
      rounds_per_change: 3
      fix_rounds_per_gate: 2
      runs_before_review: 2
  red:
    checks: [brief-approval, seal, tests, review, person-review]
    level: standard
    limits:
      minutes_per_change: 600
      rounds_per_change: 3
      fix_rounds_per_gate: 2
      runs_before_review: 2
zones:
  - id: ui
    color: green
    paths:
      - ':(glob)src/ui/**'
  - id: core
    color: yellow
    paths:
      - ':(glob)src/core/**'
      - ':(glob)tests/**'
  - id: pay
    color: red
    paths:
      - ':(glob)src/pay/**'
  - id: judges
    color: red
    paths:
      - ':(glob)gdd/**'
      - 'AGENTS.md'
context: []
---

# Blueprint

A test project.
"#;

pub const BACKLOG: &str = "# Backlog\n\n| Id | What | Color | Phase | Brief | State |\n|---|---|---|---|---|---|\n| standing-checks | Set up the checks | red | 0 | | done |\n| g1 | A green change | green | 1 | | todo |\n| y1 | A yellow change | yellow | 1 | | todo |\n| r1 | A red change | red | 1 | | todo |\n";

pub struct Repo {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
    pub runs: tempfile::TempDir,
}

impl Repo {
    pub fn new() -> Repo {
        Repo::with(BLUEPRINT)
    }

    pub fn with(blueprint: &str) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        let r = Repo {
            dir,
            root,
            runs: tempfile::tempdir().unwrap(),
        };
        git(&r.root, &["init", "-q", "-b", "main", "."]);
        // The name and the email of a project somebody set up, in the repository itself.
        git(&r.root, &["config", "user.name", "Test"]);
        git(&r.root, &["config", "user.email", "test@example.com"]);
        r.write("gdd/blueprint.md", blueprint);
        r.write("gdd/backlog.md", BACKLOG);
        r.write("AGENTS.md", "# A project\n");
        r.write("src/ui/page.txt", "a page\n");
        r.write("src/core/app.txt", "ok\n");
        r.write("src/pay/pay.txt", "pay\n");
        r.write("tests/run.sh", "grep -q ok src/core/app.txt\n");
        r.write(".gitignore", "gdd/log/\n");
        r.commit("The project");
        std::fs::create_dir_all(r.root.join("gdd/log")).unwrap();
        r
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap_or_default()
    }

    pub fn git(&self, args: &[&str]) -> String {
        git(&self.root, args)
    }

    pub fn commit(&self, msg: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
        self.git(&["rev-parse", "HEAD"])
    }

    pub fn gdd(&self, args: &[&str]) -> Run {
        let runs = self.runs.path().to_string_lossy().to_string();
        run_gdd(&self.root, args, &[("GDD_RUNS_DIR", &runs)])
    }

    pub fn gdd_env(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        let runs = self.runs.path().to_string_lossy().to_string();
        let mut e = vec![("GDD_RUNS_DIR", runs.as_str())];
        e.extend_from_slice(env);
        run_gdd(&self.root, args, &e)
    }

    /// Commit files on a branch through a worktree outside the project, as a role would.
    pub fn commit_on(
        &self,
        branch: &str,
        from: Option<&str>,
        files: &[(&str, &str)],
        msg: &str,
    ) -> String {
        let wt = self
            .dir
            .path()
            .join(format!("wt-{}", branch.replace('/', "-")));
        if wt.exists() {
            git(
                &self.root,
                &["worktree", "remove", "--force", &wt.to_string_lossy()],
            );
        }
        let w = wt.to_string_lossy().to_string();
        match from {
            Some(f) => git(&self.root, &["worktree", "add", "-q", "-b", branch, &w, f]),
            None => git(&self.root, &["worktree", "add", "-q", &w, branch]),
        };
        for (p, t) in files {
            let path = wt.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, t).unwrap();
        }
        git(&wt, &["add", "-A"]);
        git(&wt, &["commit", "-q", "-m", msg]);
        let head = git(&wt, &["rev-parse", "HEAD"]);
        git(&self.root, &["worktree", "remove", "--force", &w]);
        head
    }

    /// Write a role's last message to a file and report it.
    pub fn report(&self, id: &str, step: &str, text: &str, extra: &[&str]) -> Run {
        let f = self.dir.path().join(format!("report-{id}-{step}.txt"));
        std::fs::write(&f, text).unwrap();
        let fs = f.to_string_lossy().to_string();
        let mut args = vec!["report", id, step, fs.as_str()];
        args.extend_from_slice(extra);
        self.gdd(&args)
    }

    pub fn log_lines(&self, id: &str) -> Vec<Value> {
        self.read(&format!("gdd/log/{id}.jsonl"))
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

/// A role's last message: what it says, then its report.
pub fn report_text(skill: &str, result: &str, body: &str) -> String {
    format!("{body}\n{}", report_block(skill, result))
}

/// A reviewer's last message: its verdict first, the color it found, and its report.
pub fn review_text(verdict: &str, color: &str, extra: &str) -> String {
    format!(
        "{verdict}\nCOLOR: {color}\nI read the change.\n{extra}\n{}",
        report_block("gdd-review", verdict)
    )
}

pub fn report_block(skill: &str, result: &str) -> String {
    format!("```gdd-report\nskill: {skill} 0.10.1\nresult: {result}\ncaught: none\n```\n")
}
