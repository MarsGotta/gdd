//! Ways around the layer's own checks that an agent running it could take, each one played to the
//! end: what it tried, and that the layer refuses it while the legitimate path still works.

mod common;
use common::*;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

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
    let rv = r.report(id, "review", &review_text("pass", "green", ""), &[]);
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

// ----- 5. what --done finds in the main branch is what was judged -----

/// Merge a branch into main the way a platform does, while the copy's untracked logs, which the
/// branch's bookkeeping also holds, are set aside and then put back.
fn merge_as_the_platform(r: &Repo, branch: &str) {
    let aside = r
        .dir
        .path()
        .join(format!("aside-{}", branch.replace('/', "-")));
    std::fs::rename(r.path("gdd/log"), &aside).unwrap();
    r.git(&["merge", "-q", "--no-ff", "--no-edit", branch]);
    for e in std::fs::read_dir(&aside).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), r.path("gdd/log").join(e.file_name())).unwrap();
    }
}

#[test]
fn a_squash_that_brings_more_than_was_judged_is_refused() {
    let r = Repo::new();
    green_ready(&r, "q1");
    // The squash holds the judged page, and a change to the core that nobody judged.
    r.write("src/ui/page.txt", "a better page\n");
    r.write("src/core/app.txt", "ok, and something nobody judged\n");
    r.commit("Squash q1");
    let s = r.gdd(&["ship", "q1", "--done", "main"]);
    assert_eq!(s.facts["refused"], "unjudged", "{}", s.out);
    assert!(s.has("src/core/app.txt"), "{}", s.out);
    assert!(shipped(&r, "q1").is_empty());
}

#[test]
fn a_squash_among_other_peoples_commits_still_ships() {
    let r = Repo::new();
    let judged = green_ready(&r, "q2");
    r.write("src/pay/pay.txt", "pay, by someone else\n");
    r.commit("Someone else's change");
    r.write("src/ui/page.txt", "a better page\n");
    let squash = r.commit("Squash q2");
    r.write("src/core/other.txt", "later\n");
    r.commit("Someone else's, later");
    let s = r.gdd(&["ship", "q2", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert!(s.has("matched by content"), "{}", s.out);
    let ship = &shipped(&r, "q2")[0];
    assert_eq!(ship["in"], squash.as_str(), "where it landed");
    assert_eq!(ship["head"], judged.as_str());
    let v = r.gdd(&["verify", "q2"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

#[test]
fn a_squash_while_the_branch_holds_more_is_refused() {
    let r = Repo::new();
    let judged = green_ready(&r, "q3");
    r.commit_on(
        "work/q3",
        None,
        &[("src/core/app.txt", "ok, and more\n")],
        "After the review",
    );
    r.write("src/ui/page.txt", "a better page\n");
    r.commit("Squash q3");
    let s = r.gdd(&["ship", "q3", "--done", "main", "--head", &judged]);
    assert_eq!(s.facts["refused"], "unjudged", "{}", s.out);
    assert!(s.has("work/q3 is at"), "{}", s.out);
    assert!(shipped(&r, "q3").is_empty());
}

#[test]
fn a_merged_branch_that_held_more_is_refused_once_it_is_deleted() {
    let r = tracked_logs_repo();
    green_ready(&r, "m1");
    r.commit_on(
        "work/m1",
        None,
        &[("src/ui/page.txt", "a page nobody judged\n")],
        "After the review",
    );
    r.git(&["merge", "-q", "--no-ff", "--no-edit", "work/m1"]);
    r.git(&["branch", "-D", "work/m1"]);
    // With no branch left, the judged commit is in main, and so is what came on top of it.
    let s = r.gdd(&["ship", "m1", "--done", "main"]);
    assert_eq!(s.facts["refused"], "unjudged", "{}", s.out);
    assert!(s.has("by the merge"), "{}", s.out);
    assert!(shipped(&r, "m1").is_empty());
    // The legitimate path: the bookkeeping, the platform's merge, the branch deleted.
    green_ready(&r, "m2");
    let bk = r.gdd(&["ship", "m2", "--bookkeeping"]);
    assert_eq!(bk.code, 0, "{}", bk.out);
    merge_as_the_platform(&r, "work/m2");
    r.git(&["branch", "-D", "work/m2"]);
    let s = r.gdd(&["ship", "m2", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert_eq!(shipped(&r, "m2")[0]["head"], bk.facts["commit"]);
    let v = r.gdd(&["verify", "m2"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

#[test]
fn a_fast_forward_is_bounded_by_what_comes_after_it() {
    let r = Repo::new();
    green_ready(&r, "f1");
    let unjudged = r.commit_on(
        "work/f1",
        None,
        &[("src/ui/page.txt", "a page nobody judged\n")],
        "After the review",
    );
    r.git(&["merge", "-q", "--ff-only", "work/f1"]);
    r.git(&["branch", "-D", "work/f1"]);
    let s = r.gdd(&["ship", "f1", "--done", "main"]);
    assert_eq!(s.facts["refused"], "which-tip", "{}", s.out);
    assert!(s.has("--head"), "{}", s.out);
    // Naming the commit nobody judged as the tip: the state on it isn't ready to ship.
    let s = r.gdd(&["ship", "f1", "--done", "main", "--head", &unjudged]);
    assert_eq!(s.facts["refused"], "out-of-order", "{}", s.out);
    assert!(shipped(&r, "f1").is_empty());
    // A fast-forward whose branch says where it ended, with someone else's commit after it.
    let judged = green_ready(&r, "f2");
    r.git(&["merge", "-q", "--ff-only", "work/f2"]);
    r.write("src/pay/pay.txt", "someone else\n");
    r.commit("Someone else's change");
    let s = r.gdd(&["ship", "f2", "--done", "main"]);
    assert_eq!(s.code, 0, "{}", s.out);
    assert_eq!(shipped(&r, "f2")[0]["head"], judged.as_str());
    // After the review the builder merges the main branch into its own, the main branch takes
    // it by a fast-forward, and the branch is deleted: that merge is the change's, unjudged.
    r.write("src/ui/page.txt", "the page again\n");
    r.commit("The page again");
    green_ready(&r, "f3");
    r.write("src/pay/pay.txt", "someone else, again\n");
    r.commit("Someone else's other change");
    let wt = r.dir.path().join("wt-f3");
    let w = wt.to_string_lossy().to_string();
    r.git(&["worktree", "add", "-q", &w, "work/f3"]);
    git(&wt, &["merge", "-q", "--no-edit", "main"]);
    r.git(&["worktree", "remove", "--force", &w]);
    r.git(&["merge", "-q", "--ff-only", "work/f3"]);
    r.git(&["branch", "-D", "work/f3"]);
    let s = r.gdd(&["ship", "f3", "--done", "main"]);
    assert_eq!(s.facts["refused"], "which-tip", "{}", s.out);
    assert!(shipped(&r, "f3").is_empty());
}

// ----- 6. a ref is never taken for an option of git -----

#[test]
fn a_head_or_branch_that_starts_with_a_dash_is_refused() {
    let r = Repo::new();
    assert_eq!(
        r.gdd(&["brief", "d2", "--paths", "src/ui/page.txt"]).code,
        0
    );
    for args in [
        ["next", "d2", "--head=-x"],
        ["next", "d2", "--branch=--output=pwned"],
        ["limits", "d2", "--head=--all"],
        ["gates", "d2", "--branch=-b"],
    ] {
        let o = r.gdd(&args);
        assert_eq!(o.code, 2, "{args:?}: {}", o.out);
        assert_eq!(o.facts["reason"], "usage", "{args:?}: {}", o.out);
    }
    assert!(!r.path("pwned").exists());
}

// ----- 7. what sent a change back, and a spent cap, hold whatever command comes next -----

fn report_at(r: &Repo, id: &str, step: &str, text: &str, now: &str) -> Run {
    let f = r.dir.path().join(format!("report-{id}-{step}-at.txt"));
    std::fs::write(&f, text).unwrap();
    r.gdd_env(
        &["report", id, step, &f.to_string_lossy()],
        &[("GDD_NOW", now)],
    )
}

/// The reviewer sent the change back, and the orchestrator tried everything but what that asks
/// for: another review, the gates again, a void of the line, a fix nobody told the person about.
/// These rules were only in what `gdd next` names, so a step written without asking it went
/// around them: a second review that said pass made a stopped change ready to ship.
#[test]
fn what_the_reviewer_sent_back_is_not_judged_again_or_voided() {
    let r = Repo::new();
    let seal = yellow_sealed(&r, "y7");
    let built = report_block("gdd-build", "done");
    let page = |text: &str, msg: &str| {
        r.commit_on("work/y7", None, &[("src/core/app.txt", text)], msg);
    };
    r.commit_on(
        "work/y7",
        Some(&seal),
        &[("src/core/app.txt", "ok, built\n")],
        "Build",
    );
    assert_eq!(r.report("y7", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y7"]).code, 0);
    let refused = |what: &str, run: &Run, code: &str, way: &str| {
        assert_eq!(run.code, 1, "{what}: {}", run.out);
        assert_eq!(run.facts["refused"], code, "{what}: {}", run.out);
        assert!(run.has(way), "{what} names no way on: {}", run.out);
    };

    // A fix: no second opinion, and its line stays.
    let fix = r.report("y7", "review", &review_text("fix", "yellow", ""), &[]);
    assert_eq!(fix.code, 0, "{}", fix.out);
    let line = r.log_lines("y7").len().to_string();
    let way = "gdd launch y7 build";
    refused(
        "another review",
        &r.gdd(&["launch", "y7", "review"]),
        "out-of-order",
        way,
    );
    refused(
        "a second verdict",
        &r.report("y7", "review", &review_text("pass", "yellow", ""), &[]),
        "out-of-order",
        way,
    );
    refused(
        "a void of the fix",
        &r.gdd(&["correct", "y7", "--line", &line, "--void", "it was wrong"]),
        "never-voided",
        way,
    );
    page("ok, built better\n", "The fix round");
    assert_eq!(r.report("y7", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y7"]).code, 0);

    // A stop: the same, and the person is told before anyone builds again.
    let stop = r.report("y7", "review", &review_text("stop", "yellow", ""), &[]);
    assert_eq!(stop.code, 0, "{}", stop.out);
    let lines = r.log_lines("y7").len();
    let way = "gdd ask y7 --of stop";
    refused(
        "another review",
        &r.gdd(&["launch", "y7", "review"]),
        "out-of-order",
        way,
    );
    refused(
        "a second verdict",
        &r.report("y7", "review", &review_text("pass", "yellow", ""), &[]),
        "out-of-order",
        way,
    );
    refused(
        "a void of the stop",
        &r.gdd(&[
            "correct",
            "y7",
            "--line",
            &lines.to_string(),
            "--void",
            "the reviewer was wrong",
        ]),
        "never-voided",
        way,
    );
    // Commits on the branch change nothing: the stop isn't forgotten for the gates of a new head.
    page("ok, built again\n", "A fix nobody told the person about");
    let n = r.gdd(&["next", "y7"]);
    assert_eq!(
        n.facts["state"]["next"]["blocked"], "review-stop",
        "{}",
        n.out
    );
    refused("the gates", &r.gdd(&["gates", "y7"]), "out-of-order", way);
    refused(
        "a build",
        &r.report("y7", "build", &built, &[]),
        "out-of-order",
        way,
    );
    assert_eq!(r.log_lines("y7").len(), lines, "nothing was written");

    // What it asks for still works: the person is told and answers, then a fix round and a review.
    assert_eq!(
        r.gdd(&["ask", "y7", "--of", "stop", "It stopped: fix it?"])
            .code,
        0
    );
    let early = r.report("y7", "build", &built, &[]);
    assert_eq!(early.facts["refused"], "needs-person", "{}", early.out);
    assert_eq!(r.gdd(&["answer", "y7", "yes"]).code, 0);
    assert_eq!(r.report("y7", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y7"]).code, 0);
    let pass = r.report("y7", "review", &review_text("pass", "yellow", ""), &[]);
    assert_eq!(pass.code, 0, "{}", pass.out);
    assert_eq!(
        r.gdd(&["next", "y7"]).facts["state"]["next"]["step"],
        "ship"
    );
    let v = r.gdd(&["verify", "y7"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

/// A cap stops the next round, launched through the layer or not. What counts is how the caps
/// stood when the round began, at the log's last line, so a build that ended after its time ran
/// out is still written and judged.
#[test]
fn a_round_nobody_launched_is_not_written_past_a_spent_cap() {
    let r = Repo::new();
    let seal = yellow_sealed(&r, "y8");
    let built = report_block("gdd-build", "done");
    r.commit_on(
        "work/y8",
        Some(&seal),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    assert_eq!(r.report("y8", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y8"]).code, 1);
    // Its fix round ends long after the change's minutes ran out. It began with room.
    r.commit_on(
        "work/y8",
        None,
        &[("src/core/app.txt", "still bad\n")],
        "Round 2",
    );
    let late = report_at(&r, "y8", "build", &built, "2099-01-01T00:00:00Z");
    assert_eq!(late.code, 0, "{}", late.out);
    let g = r.gdd_env(&["gates", "y8"], &[("GDD_NOW", "2099-01-01T00:01:00Z")]);
    assert_eq!(g.code, 1, "it fails, but it was judged: {}", g.out);
    // The next one began with no room: launched or not, it isn't written.
    r.commit_on("work/y8", None, &[("src/core/app.txt", "ok\n")], "Round 3");
    let lines = r.log_lines("y8").len();
    let now = [("GDD_NOW", "2099-01-01T00:02:00Z")];
    let l = r.gdd_env(&["launch", "y8", "build"], &now);
    assert_eq!(l.facts["refused"], "cap-spent", "{}", l.out);
    let past = report_at(&r, "y8", "build", &built, "2099-01-01T00:02:00Z");
    assert_eq!(past.facts["refused"], "cap-spent", "{}", past.out);
    assert!(past.has("gdd ask y8 --of"), "{}", past.out);
    assert_eq!(r.log_lines("y8").len(), lines, "nothing was written");
    // With the room only the person gives, it is.
    let w = r.gdd_env(
        &[
            "waiver",
            "y8",
            "--who",
            "Person",
            "--check",
            "minutes_per_change",
            "--why",
            "one more try",
            "--backlog",
            "y8-minutes",
            "--limit",
            "minutes_per_change",
            "--to",
            "99999999",
        ],
        &now,
    );
    assert_eq!(w.code, 0, "{}", w.out);
    let given = report_at(&r, "y8", "build", &built, "2099-01-01T00:03:00Z");
    assert_eq!(given.code, 0, "{}", given.out);
}

/// Voiding a round doesn't give it back. A voided line decides nothing, but what it used was used:
/// before this, voiding the failed gates and the second build brought the counter back to zero.
#[test]
fn a_voided_round_is_still_a_round() {
    let one = BLUEPRINT.replace(
        "      rounds_per_change: 3\n      fix_rounds_per_gate: 2\n      runs_before_review: 2\n  red:",
        "      rounds_per_change: 1\n      fix_rounds_per_gate: 2\n      runs_before_review: 2\n  red:",
    );
    let r = Repo::with(&one);
    let seal = yellow_sealed(&r, "y9");
    let built = report_block("gdd-build", "done");
    r.commit_on(
        "work/y9",
        Some(&seal),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    assert_eq!(r.report("y9", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y9"]).code, 1);
    r.commit_on(
        "work/y9",
        None,
        &[("src/core/app.txt", "still bad\n")],
        "Round 2",
    );
    assert_eq!(r.report("y9", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y9"]).code, 1);
    assert_eq!(
        r.gdd(&["launch", "y9", "build"]).facts["refused"],
        "cap-spent"
    );
    // The failed gates and the second build, voided: lines a correction may take out.
    let mut builds = 0;
    for (n, e) in r.log_lines("y9").iter().enumerate() {
        builds += usize::from(e["step"] == "build");
        if e["step"] == "gates" || e["step"] == "build" && builds > 1 {
            let line = (n + 1).to_string();
            let c = r.gdd(&["correct", "y9", "--line", &line, "--void", "noise"]);
            assert_eq!(c.code, 0, "{}", c.out);
        }
    }
    let lim = r.gdd(&["limits", "y9"]);
    assert!(lim.has("rounds 1 of 1, no room left"), "{}", lim.out);
    // The first build waits for its gates again; after them, there is still no room.
    assert_eq!(r.gdd(&["gates", "y9"]).code, 1);
    let l = r.gdd(&["launch", "y9", "build"]);
    assert_eq!(l.facts["refused"], "cap-spent", "{}", l.out);
    r.commit_on("work/y9", None, &[("src/core/app.txt", "ok\n")], "Round 3");
    let past = r.report("y9", "build", &built, &[]);
    assert_eq!(past.facts["refused"], "cap-spent", "{}", past.out);
    // And whoever reads the log is told which lines that judged the change were voided.
    let v = r.gdd(&["verify", "y9"]);
    let voids = v.facts["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "void" && f["level"] == "warning")
        .count();
    assert_eq!(
        (v.facts["errors"].as_i64(), voids),
        (Some(0), 2),
        "{}",
        v.out
    );
}

/// Gates that failed are answered by a round, and a round has a build step. A fix committed with
/// none, and the gates again, was a round the cap on rounds never counted.
#[test]
fn gates_that_failed_dont_run_again_on_a_fix_nobody_reported() {
    let r = Repo::new();
    let seal = yellow_sealed(&r, "y6");
    let built = report_block("gdd-build", "done");
    r.commit_on(
        "work/y6",
        Some(&seal),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    assert_eq!(r.report("y6", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y6"]).code, 1);
    // On the same commit they may run again: a check can fail for reasons of its own.
    let again = r.gdd(&["gates", "y6"]);
    assert!(again.facts["refused"].is_null(), "{}", again.out);
    // A fix lands on the branch, and nobody reports the round.
    r.commit_on("work/y6", None, &[("src/core/app.txt", "ok\n")], "A fix");
    let n = r.gdd(&["next", "y6"]);
    let command = n.facts["state"]["next"]["command"].as_str().unwrap_or("");
    assert!(command.starts_with("gdd report y6 build"), "{}", n.out);
    let lines = r.log_lines("y6").len();
    let g = r.gdd(&["gates", "y6"]);
    assert_eq!(g.facts["refused"], "out-of-order", "{}", g.out);
    assert!(g.has("gdd report y6 build"), "{}", g.out);
    assert_eq!(r.log_lines("y6").len(), lines, "nothing was written");
    // Reported, it is a round, and its gates run.
    assert_eq!(r.report("y6", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y6"]).code, 0);
    let lim = r.gdd(&["limits", "y6"]);
    assert!(lim.has("rounds 1 of 3"), "{}", lim.out);

    // A branch that only took the main branch in, through its seal, has no round to report.
    let seal = yellow_sealed(&r, "y5");
    r.commit_on(
        "work/y5",
        Some(&seal),
        &[("src/core/app.txt", "bad\n")],
        "Round 1",
    );
    assert_eq!(r.report("y5", "build", &built, &[]).code, 0);
    assert_eq!(r.gdd(&["gates", "y5"]).code, 1);
    r.write("src/ui/page.txt", "another page\n");
    r.commit("Something else shipped");
    assert_eq!(r.gdd(&["seal", "y5", "--merge-main"]).code, 0);
    let tree = r.git(&["merge-tree", "--write-tree", "work/y5", "gdd-seal/y5"]);
    let merge = r.git(&[
        "commit-tree",
        &tree,
        "-p",
        "work/y5",
        "-p",
        "gdd-seal/y5",
        "-m",
        "Take the seal in",
    ]);
    r.git(&["branch", "-f", "work/y5", &merge]);
    let g = r.gdd(&["gates", "y5"]);
    assert!(g.facts["refused"].is_null(), "{}", g.out);
    assert_eq!(g.code, 1, "judged, and it still fails: {}", g.out);
    let v = r.gdd(&["verify", "--all"]);
    assert_eq!(v.facts["errors"], 0, "{}", v.out);
}

/// A slip of the pen in the blueprint is no way to drop a check: one whose `runs` was neither
/// tier was planned, never run and never named, and the gates passed without it.
#[test]
fn a_check_whose_tier_is_misspelled_is_never_left_out_without_a_word() {
    let slip = BLUEPRINT.replace(
        "    command: sh tests/run.sh\n    runs: each round\n",
        "    command: sh tests/run.sh\n    runs: each-round\n",
    );
    assert_ne!(slip, BLUEPRINT, "the blueprint of the tests changed shape");
    let r = Repo::with(&slip);
    let base = r.git(&["rev-parse", "main"]);
    assert_eq!(
        r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    // A build the tests would reject: with the check left out, the gates passed.
    r.commit_on(
        "work/g1",
        Some(&base),
        &[
            ("src/ui/page.txt", "a page\n"),
            ("src/core/app.txt", "broken\n"),
        ],
        "Build",
    );
    r.report("g1", "build", &report_block("gdd-build", "done"), &[]);
    let g = r.gdd(&["gates", "g1"]);
    assert_eq!(g.facts["refused"], "bad-blueprint", "{}", g.out);
    assert!(g.has("tests") && g.has("each-round"), "{}", g.out);
    assert!(
        r.log_lines("g1").iter().all(|e| e["step"] != "gates"),
        "no gates step was written"
    );
}
