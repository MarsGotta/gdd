//! Ways around the layer's own checks that an agent running it could take, each one played to the
//! end: what it tried, and that the layer refuses it while the legitimate path still works.

mod common;
use common::*;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

fn review_text(verdict: &str, color: &str) -> String {
    format!(
        "{verdict}\nCOLOR: {color}\nI read the change.\n{}",
        report_block("gdd-review", verdict)
    )
}

/// A project whose logs are tracked, as a real one's are.
fn tracked_logs_repo() -> Repo {
    let r = Repo::new();
    r.write(".gitignore", "");
    r.commit("Track the logs");
    r
}

/// A green change built, through its gates and its review: ready to ship. Returns the judged head.
fn green_ready(r: &Repo, id: &str) -> String {
    assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
    let base = r.git(&["rev-parse", "main"]);
    let built = r.commit_on(
        &format!("work/{id}"),
        Some(&base),
        &[("src/ui/page.txt", "a better page\n")],
        "Build",
    );
    let b = r.report(id, "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(b.code, 0, "{}", b.out);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 0, "{}", g.out);
    let rv = r.report(id, "review", &review_text("pass", "green"), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    assert_eq!(r.gdd(&["next", id]).facts["state"]["next"]["step"], "ship");
    built
}

fn shipped(r: &Repo, id: &str) -> Vec<Value> {
    r.log_lines(id)
        .into_iter()
        .filter(|e| e["step"] == "ship")
        .collect()
}

/// A yellow change approved, with a real check sealed by the checks role. Returns the seal's tip.
fn yellow_sealed(r: &Repo, id: &str) -> String {
    r.write(
        &format!("gdd/briefs/{id}.md"),
        &format!("---\ncontract: gdd/1\nid: {id}\nversion: 1\n---\n\n# Ok\n"),
    );
    r.write(
        &format!("features/{id}.feature"),
        &format!("Feature: Ok\n\n  @brief:{id} @id:{id}-1\n  Scenario: It says ok\n"),
    );
    let files = [
        format!("gdd/briefs/{id}.md"),
        format!("features/{id}.feature"),
    ];
    assert_eq!(r.gdd(&["seal", id, &files[0], &files[1]]).code, 0);
    let b = r.gdd(&[
        "brief",
        id,
        &files[0],
        &files[1],
        "--paths",
        "src/core/app.txt",
    ]);
    assert_eq!(b.code, 0, "{}", b.out);
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        &files[1],
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    // The check the person's scenario asks for, sealed: the app must say it is built.
    r.commit_on(
        &format!("gdd-seal/{id}"),
        None,
        &[(
            &format!("tests/{id}.sh"),
            "grep -q built src/core/app.txt\n",
        )],
        "Seal its check",
    );
    let c = r.report(id, "checks", &report_block("gdd-checks", "sealed"), &[]);
    assert_eq!(c.code, 0, "{}", c.out);
    r.git(&["rev-parse", &format!("gdd-seal/{id}")])
}

/// Its check loosened on a commit on top of the seal, and a build on top of that one, which the
/// sealed check would reject. Returns the loosened commit.
fn loosened_and_built(r: &Repo, id: &str, seal: &str) -> String {
    let loose = r.commit_on(
        "loose",
        Some(seal),
        &[(&format!("tests/{id}.sh"), "true\n")],
        "Loosen the check",
    );
    r.commit_on(
        &format!("work/{id}"),
        Some(&loose),
        &[("src/core/app.txt", "ok, done\n")],
        "Build, without what the check asks",
    );
    let b = r.report(id, "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(b.code, 0, "{}", b.out);
    loose
}

// ----- 1. --head names a commit; it never relaxes readiness -----

#[test]
fn head_never_stands_in_for_a_change_ready_to_ship() {
    let r = Repo::new();
    let judged = green_ready(&r, "h1");
    // After the review, the branch gets a commit nobody judged, and the person merges the branch.
    r.commit_on(
        "work/h1",
        None,
        &[("src/ui/page.txt", "a page nobody judged\n")],
        "After the review",
    );
    assert_eq!(
        r.gdd(&["next", "h1"]).facts["state"]["next"]["step"],
        "gates"
    );
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge h1", "work/h1"]);
    // Naming the main branch's tip as the head used to skip the check that the change is ready.
    let s = r.gdd(&["ship", "h1", "--done", "main", "--head", "main"]);
    assert_eq!(s.facts["refused"], "out-of-order", "{}", s.out);
    // Naming the judged commit, while the branch that went in holds more than it: refused too.
    let s = r.gdd(&["ship", "h1", "--done", "main", "--head", &judged]);
    assert_eq!(s.facts["refused"], "unjudged", "{}", s.out);
    assert!(s.has("wasn't judged"), "{}", s.out);
    // A --head that names no commit is never the branch.
    let s = r.gdd(&["ship", "h1", "--done", "main", "--head", "nothing-here"]);
    assert_eq!(s.code, 2, "{}", s.out);
    assert_eq!(s.facts["what"], "nothing-here");
    assert!(
        shipped(&r, "h1").is_empty(),
        "nothing was recorded as shipped"
    );
}

#[test]
fn a_local_merge_merges_the_judged_commit_by_its_hash() {
    let r = tracked_logs_repo();
    let judged = green_ready(&r, "h2");
    r.commit_on(
        "work/h2",
        None,
        &[("src/ui/page.txt", "a page nobody judged\n")],
        "After the review",
    );
    let s = r.gdd(&["ship", "h2", "--merge"]);
    assert_eq!(s.facts["refused"], "out-of-order", "{}", s.out);
    // Naming the judged commit used to merge the branch by its name, with what came after it.
    let s = r.gdd(&["ship", "h2", "--merge", "--head", &judged]);
    assert_eq!(s.facts["refused"], "branch-moved", "{}", s.out);
    assert_eq!(
        r.git(&["show", "main:src/ui/page.txt"]),
        "a page",
        "nothing merged"
    );
    assert!(shipped(&r, "h2").is_empty());
    // The legitimate path: the branch back at what was judged, merged by its hash.
    r.git(&["branch", "-f", "work/h2", &judged]);
    let m = r.gdd(&["ship", "h2", "--merge"]);
    assert_eq!(m.code, 0, "{}", m.out);
    let head = m.facts["head"].as_str().unwrap().to_string();
    let merge = m.facts["merge"].as_str().unwrap().to_string();
    assert_eq!(r.git(&["rev-parse", &format!("{merge}^2")]), head);
    assert_eq!(r.git(&["rev-parse", &format!("{head}^")]), judged);
    assert_eq!(r.git(&["show", "main:src/ui/page.txt"]), "a better page");
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

#[test]
fn a_squash_merge_ships_only_when_main_holds_what_was_judged() {
    let r = Repo::new();
    let judged = green_ready(&r, "h3");
    // A squash that took something else: the judged commit isn't in main, nor its content.
    r.write("src/ui/page.txt", "a better page, edited on the platform\n");
    r.commit("Squash h3, edited");
    let s = r.gdd(&["ship", "h3", "--done", "main"]);
    assert_eq!(s.facts["refused"], "not-merged", "{}", s.out);
    assert!(s.has("src/ui/page.txt differs"), "{}", s.out);
    // A squash of exactly what was judged.
    r.write("src/ui/page.txt", "a better page\n");
    let squash = r.commit("Squash h3");
    let s = r.gdd(&["ship", "h3", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert!(s.has("matched by content"), "{}", s.out);
    assert_eq!(s.facts["matched"], "content");
    let ship = &shipped(&r, "h3")[0];
    assert_eq!(ship["matched"], "content");
    assert_eq!(ship["in"], squash.as_str());
    assert_eq!(ship["head"], judged.as_str());
    let v = r.gdd(&["verify", "h3"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

// ----- 2. gdd log writes nothing, and <start> comes from git -----

#[test]
fn log_refuses_every_event_that_has_a_command_of_its_own() {
    let r = Repo::new();
    let id = "l1";
    let seal = yellow_sealed(&r, id);
    let loose = loosened_and_built(&r, id, &seal);
    // The bypass: a seal event composed by hand moves <start> to the loosened check.
    let moved = format!(
        r#"{{"event":"seal","ref":"gdd-seal/{id}","from":"{seal}","to":"{loose}","why":"merged main"}}"#
    );
    let l = r.gdd(&["log", id, &moved]);
    assert_eq!(l.facts["refused"], "use-the-command", "{}", l.out);
    assert!(l.has(&format!("gdd seal {id}")), "{}", l.out);
    // So does a checks step that names it as the seal's commit.
    let checks = format!(
        r#"{{"event":"step","step":"checks","report":{{"skill":"gdd-checks","result":"sealed"}},"seal":{{"ref":"gdd-seal/{id}","commit":"{loose}","checks":{{}},"proofs":[]}}}}"#
    );
    let l = r.gdd(&["log", id, &checks]);
    assert_eq!(l.facts["refused"], "use-the-command", "{}", l.out);
    assert!(l.has(&format!("gdd report {id} checks")), "{}", l.out);
    // A stopped brief that lowers the color, and every other event: each has its command.
    let events = [
        (
            r#"{"event":"step","step":"brief","report":{"skill":"gdd-brief","result":"stopped"},"color":{"color":"green"},"files":{},"stop":{"reason":"x"}}"#,
            "--stop",
        ),
        (
            r#"{"event":"approval","who":"P","of":"change","version":1,"files":{},"where":"x"}"#,
            "gdd approve",
        ),
        (
            r#"{"event":"waiver","who":"P","check":"seal","why":"x"}"#,
            "gdd waiver",
        ),
        (r#"{"event":"ask","of":"decision","what":"x"}"#, "gdd ask"),
        (r#"{"event":"answer","what":"yes"}"#, "gdd answer"),
        (
            r#"{"event":"correction","line":1,"what":"x"}"#,
            "gdd correct",
        ),
        (
            r#"{"event":"step","step":"build","report":{"skill":"gdd-build","result":"done"}}"#,
            "gdd report",
        ),
        (
            r#"{"event":"step","step":"review","report":{"skill":"gdd-review","result":"pass"},"verdict":"pass"}"#,
            "gdd report",
        ),
        (
            r#"{"event":"step","step":"reading","report":{"skill":"gdd-review","result":"pass"}}"#,
            "gdd report",
        ),
        (
            r#"{"event":"step","step":"gates","start":"x","gates":[]}"#,
            "gdd gates",
        ),
        (
            r#"{"event":"step","step":"ship","where":"main"}"#,
            "gdd ship",
        ),
        (
            r#"{"event":"step","step":"harden","report":{"skill":"gdd-harden","result":"proposed"}}"#,
            "gdd step blueprint harden",
        ),
        (r#"{"event":"note","what":"x"}"#, "gdd next"),
    ];
    let before = r.read(&format!("gdd/log/{id}.jsonl"));
    for (event, command) in events {
        let l = r.gdd(&["log", id, event]);
        assert_eq!(l.facts["refused"], "use-the-command", "{event}: {}", l.out);
        assert!(l.has(command), "{event}: {}", l.out);
    }
    assert_eq!(
        r.read(&format!("gdd/log/{id}.jsonl")),
        before,
        "gdd log wrote nothing"
    );
    // The gates judge from the seal, so the loosened check is found.
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 1, "{}", g.out);
    assert_eq!(g.facts["start"], seal.as_str(), "{}", g.out);
    assert!(g.has("the seal failed"), "{}", g.out);
}

#[test]
fn gates_take_start_from_the_seal_and_not_from_a_log_line() {
    let r = Repo::new();
    let id = "l2";
    let seal = yellow_sealed(&r, id);
    let loose = loosened_and_built(&r, id, &seal);
    // The same move of <start>, appended to the log by hand, at the time of its last line.
    let log = r.read(&format!("gdd/log/{id}.jsonl"));
    let ts = r.log_lines(id).pop().unwrap()["ts"]
        .as_str()
        .unwrap()
        .to_string();
    let line = format!(
        r#"{{"contract":"gdd/1","ts":"{ts}","brief":"{id}","event":"seal","ref":"gdd-seal/{id}","from":"{seal}","to":"{loose}","why":"merged main"}}"#
    );
    r.write(&format!("gdd/log/{id}.jsonl"), &format!("{log}{line}\n"));
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.facts["refused"], "seal-moved", "{}", g.out);
    assert!(
        g.has("the seal moved where the layer didn't record it"),
        "{}",
        g.out
    );
    // Without the seal's ref, the log alone is never trusted either.
    r.git(&["branch", "-D", &format!("gdd-seal/{id}")]);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.facts["refused"], "seal-missing", "{}", g.out);
    assert!(
        r.log_lines(id).iter().all(|e| e["step"] != "gates"),
        "no gates step was written"
    );
}

#[test]
fn gates_in_ci_take_the_seal_its_remote_holds() {
    let r = Repo::new();
    let id = "c1";
    let seal = yellow_sealed(&r, id);
    r.commit_on(
        &format!("work/{id}"),
        Some(&seal),
        &[("src/core/app.txt", "ok, built\n")],
        "Build",
    );
    assert_eq!(
        r.report(id, "build", &report_block("gdd-build", "done"), &[])
            .code,
        0
    );
    // As a CI checkout has it: no local gdd-seal/<id>, only the platform's.
    r.git(&[
        "update-ref",
        &format!("refs/remotes/origin/gdd-seal/{id}"),
        &seal,
    ]);
    r.git(&["branch", "-D", &format!("gdd-seal/{id}")]);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 0, "{}", g.out);
    assert_eq!(g.facts["start"], seal.as_str());
    // A remote's seal that moved where the log doesn't say is refused like a local one.
    let other = r.commit_on(
        "elsewhere",
        Some(&seal),
        &[(&format!("tests/{id}.sh"), "true\n")],
        "Loosen",
    );
    r.git(&[
        "update-ref",
        &format!("refs/remotes/origin/gdd-seal/{id}"),
        &other,
    ]);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.facts["refused"], "seal-moved", "{}", g.out);
    assert!(g.has(&format!("origin/gdd-seal/{id} is at")), "{}", g.out);
    // No seal anywhere: the log alone is never enough.
    r.git(&[
        "update-ref",
        "-d",
        &format!("refs/remotes/origin/gdd-seal/{id}"),
    ]);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.facts["refused"], "seal-missing", "{}", g.out);
}

#[test]
fn a_dropped_change_has_a_command_of_its_own() {
    let r = Repo::new();
    assert_eq!(
        r.gdd(&["brief", "d1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    let wrong = r.gdd(&["brief", "d1", "--paths", "src/ui/page.txt", "--stop", "no"]);
    assert_eq!(wrong.code, 2, "{}", wrong.out);
    let s = r.gdd(&["brief", "d1", "--stop", "the person dropped it"]);
    assert_eq!(s.code, 0, "{}", s.out);
    let last = r.log_lines("d1").pop().unwrap();
    assert_eq!(last["step"], "brief");
    assert_eq!(last["report"]["result"], "stopped");
    assert_eq!(last["stop"]["reason"], "the person dropped it");
    assert!(last.get("color").is_none(), "it moves no color");
    let n = r.gdd(&["next", "d1"]);
    assert_eq!(n.facts["state"]["next"]["blocked"], "cannot", "{}", n.out);
    assert!(
        n.has("the brief stopped: the person dropped it"),
        "{}",
        n.out
    );
    // Its words stay out of the journal.
    assert!(!r.read("gdd/log/d1.engine.jsonl").contains("dropped it"));
    let v = r.gdd(&["verify", "d1"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

// ----- 3. the runs folder is private -----

/// Run the layer with its own temporary folder, and no GDD_RUNS_DIR unless given.
fn gdd_in_tmp(root: &Path, tmp: &Path, args: &[&str], runs: Option<&Path>) -> (i32, String) {
    let mut c = Command::new(BIN);
    c.arg("--root")
        .arg(root)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .env("TMPDIR", tmp)
        .env_remove("GDD_RUNS_DIR")
        .env_remove("GDD_NOW")
        .env_remove("GDD_MAIN")
        .env_remove("CI");
    if let Some(d) = runs {
        c.env("GDD_RUNS_DIR", d);
    }
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
    )
}

fn files_under(dir: &Path) -> Vec<String> {
    let mut out = vec![];
    if let Ok(d) = std::fs::read_dir(dir) {
        for e in d.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(files_under(&p));
            } else {
                out.push(p.display().to_string());
            }
        }
    }
    out
}

#[cfg(unix)]
#[test]
fn the_runs_folder_is_private_and_a_planted_link_is_never_followed() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let r = Repo::new();
    let base = r.git(&["rev-parse", "main"]);
    for id in ["p1", "p2"] {
        assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
        r.commit_on(
            &format!("work/{id}"),
            Some(&base),
            &[("src/ui/page.txt", "p\n")],
            "Page",
        );
        r.report(id, "build", &report_block("gdd-build", "done"), &[]);
    }
    // Someone else's temporary folder: the usual names of the runs folder are links to theirs.
    let tmp = tempfile::tempdir().unwrap();
    let victim = tempfile::tempdir().unwrap();
    let uid = std::fs::metadata(&r.root).unwrap().uid();
    for name in ["gdd-runs".to_string(), format!("gdd-runs-{uid}")] {
        std::os::unix::fs::symlink(victim.path(), tmp.path().join(name)).unwrap();
    }
    let (code, out) = gdd_in_tmp(&r.root, tmp.path(), &["gates", "p1"], None);
    assert_eq!(code, 0, "{out}");
    assert!(
        files_under(victim.path()).is_empty(),
        "nothing went through the link: {out}"
    );
    assert!(out.contains("is a link"), "{out}");
    // Where the outputs went: a folder of this user's only, with files of this user's only.
    let at = out
        .lines()
        .find_map(|l| l.split("outputs in ").nth(1))
        .unwrap()
        .trim()
        .to_string();
    let m = std::fs::metadata(&at).unwrap();
    assert_eq!(m.uid(), uid);
    assert_eq!(m.permissions().mode() & 0o777, 0o700, "{at}");
    let outputs = files_under(Path::new(&at));
    assert!(!outputs.is_empty());
    for f in outputs {
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o077,
            0,
            "{f}"
        );
    }
    // GDD_RUNS_DIR gets the same checks: a folder others can write is never used.
    let open = tempfile::tempdir().unwrap();
    std::fs::set_permissions(open.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let (code, out) = gdd_in_tmp(&r.root, tmp.path(), &["gates", "p2"], Some(open.path()));
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("others can write"), "{out}");
    assert!(files_under(open.path()).is_empty(), "{out}");
}

// ----- 4. the bookkeeping exemption covers only the layer's bookkeeping -----

#[test]
fn what_a_builder_puts_under_gdd_log_is_judged_like_any_other_file() {
    let r = tracked_logs_repo();
    let judged = green_ready(&r, "b1");
    let copy = r.read("gdd/log/b1.jsonl");
    for (path, text) in [
        ("gdd/log/payload.sh", "echo unjudged\n"),
        ("gdd/log/sub/b1.jsonl", "{}\n"),
        ("gdd/log/b1.jsonl", "{\"forged\":true}\n"),
        ("gdd/log/b1.jsonl", &format!("{copy}{{\"forged\":true}}\n")),
    ] {
        r.git(&["branch", "-f", "work/b1", &judged]);
        r.commit_on("work/b1", None, &[(path, text)], "After the review");
        let n = r.gdd(&["next", "b1"]);
        assert_eq!(
            n.facts["state"]["next"]["step"], "gates",
            "{path}: {}",
            n.out
        );
        let s = r.gdd(&["ship", "b1", "--merge"]);
        assert_eq!(s.facts["refused"], "out-of-order", "{path}: {}", s.out);
    }
    assert!(
        !git_ok(&r.root, &["cat-file", "-e", "main:gdd/log/payload.sh"]),
        "nothing a builder put under gdd/log reached main"
    );
    assert!(shipped(&r, "b1").is_empty());
    // The layer's own bookkeeping still stands for what was judged, after the copy moves on.
    r.git(&["branch", "-f", "work/b1", &judged]);
    let bk = r.gdd(&["ship", "b1", "--bookkeeping"]);
    assert_eq!(bk.code, 0, "{}", bk.out);
    let add = r.gdd(&[
        "backlog",
        "add",
        "b1-later",
        "--what",
        "Something for later",
        "--color",
        "green",
    ]);
    assert_eq!(add.code, 0, "{}", add.out);
    let n = r.gdd(&["next", "b1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "ship", "{}", n.out);
    let m = r.gdd(&["ship", "b1", "--merge"]);
    assert_eq!(m.code, 0, "{}", m.out);
    assert_eq!(r.git(&["status", "--porcelain"]), "", "{}", m.out);
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}
