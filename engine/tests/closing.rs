//! Close: what a brief says closes its change, what waits once it is in use, and the verdict.

mod common;
use common::*;

const THREE: &str = "Expected: people find the status at once\nSignal: fewer messages asking whether it is up\nWhere: the support inbox\n";

/// A blueprint that says when a change that promised nothing closes by itself.
fn repo_with_days() -> Repo {
    Repo::with(&BLUEPRINT.replace(
        "context: []",
        "closing:\n  days: 7\n  who: the owner\ncontext: []",
    ))
}

fn brief_text(id: &str, closing: &str) -> String {
    let section = if closing.is_empty() {
        String::new()
    } else {
        format!("## Closing it\n\n{closing}\n")
    };
    format!(
        "---\ncontract: gdd/1\nid: {id}\nversion: 1\nscenarios: [{id}-1]\n---\n\n# A yellow change\n\n## Request\n\n\"Make it ok.\"\n\n{section}## Undoing it\n\nRevert the merge.\n"
    )
}

/// Seal and log a yellow brief that says `closing`, and return what `gdd brief` said.
fn brief_with(r: &Repo, id: &str, closing: &str) -> Run {
    brief_with_rules(r, id, closing, &ok_feature(id))
}

fn brief_with_rules(r: &Repo, id: &str, closing: &str, rules: &str) -> Run {
    let (md, feature) = (
        format!("gdd/briefs/{id}.md"),
        format!("features/{id}.feature"),
    );
    r.write_as_it_is(&md, &brief_text(id, closing));
    r.write(&feature, rules);
    assert_eq!(r.gdd(&["seal", id, &md, &feature]).code, 0);
    r.gdd(&["brief", id, &md, &feature, "--paths", "src/core/app.txt"])
}

fn merged_and_shipped(r: &Repo, id: &str) {
    let rv = r.report(id, "review", &r.review_pass(id, "yellow", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    let s = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(s.code, 0, "{}", s.out);
    // What the ship says next is that it is in use, with no command to run, and what waits.
    assert_eq!(s.facts["next"], "in-use", "{}", s.out);
    assert_eq!(s.facts["next_command"], "", "{}", s.out);
    assert!(
        s.has(&format!(
            "in use, waiting for a verdict: {id}, 0 minutes; expected: "
        )),
        "{}",
        s.out
    );
    assert!(s.has("with no question and no offer to look"), "{}", s.out);
}

/// A yellow change whose brief says `closing`, through every step and in the main branch.
fn yellow_shipped(r: &Repo, id: &str, closing: &str) {
    shipped_with_rules(r, id, closing, &ok_feature(id));
}

fn shipped_with_rules(r: &Repo, id: &str, closing: &str, rules: &str) {
    let b = brief_with_rules(r, id, closing, rules);
    assert_eq!(b.code, 0, "{}", b.out);
    let feature = format!("features/{id}.feature");
    let a = r.gdd(&[
        "approve",
        id,
        "--of",
        "brief",
        "--who",
        "Person",
        "--where",
        "conversation",
        &feature,
    ]);
    assert_eq!(a.code, 0, "{}", a.out);
    r.commit_on(
        &format!("gdd-seal/{id}"),
        None,
        &[(&format!("tests/{id}.sh"), "true\n")],
        "Seal its test",
    );
    let proofs = "```json\n[{\"check\":\"tests\",\"case\":\"rejects\",\"test\":\"says_ok\",\"wrong\":\"prints nothing\",\"command\":\"sh tests/run.sh\",\"exit\":1,\"tail\":\"FAILED\"}]\n```\n";
    let c = r.report(
        id,
        "checks",
        &format!("Sealed.\n{proofs}{}", report_block("gdd-checks", "sealed")),
        &[],
    );
    assert_eq!(c.code, 0, "{}", c.out);
    let start = r.git(&["rev-parse", &format!("gdd-seal/{id}")]);
    r.commit_on(
        &format!("work/{id}"),
        Some(&start),
        &[("src/core/app.txt", "ok, built\n")],
        "Build",
    );
    let built = r.report(id, "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(built.code, 0, "{}", built.out);
    let g = r.gdd(&["gates", id]);
    assert_eq!(g.code, 0, "{}", g.out);
    merged_and_shipped(r, id);
}

/// A green change with no brief file, in the main branch: it promised nothing.
fn green_shipped(r: &Repo, id: &str) {
    assert_eq!(r.gdd(&["brief", id, "--paths", "src/ui/page.txt"]).code, 0);
    r.commit_on(
        &format!("work/{id}"),
        Some("main"),
        &[("src/ui/page.txt", &format!("a better page, by {id}\n"))],
        "Fix the page",
    );
    let built = r.report(id, "build", &report_block("gdd-build", "done"), &[]);
    assert_eq!(built.code, 0, "{}", built.out);
    assert_eq!(r.gdd(&["gates", id]).code, 0);
    let rv = r.report(id, "review", &r.review_pass(id, "green", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    r.git(&[
        "merge",
        "-q",
        "--no-ff",
        "-m",
        "Merge it",
        &format!("work/{id}"),
    ]);
    assert_eq!(r.gdd(&["ship", id, "--done", "main"]).code, 0);
}

fn verdict(r: &Repo, id: &str, verdict: &str, saw: &str) -> Run {
    r.gdd(&[
        "close",
        id,
        "--verdict",
        verdict,
        "--who",
        "Person",
        "--where",
        "conversation",
        "--saw",
        saw,
    ])
}

/// The clock, this many days from now.
fn in_days(days: i64) -> String {
    gdd::time::format(gdd::time::now() + days * 86_400)
}

#[test]
fn a_brief_says_how_it_closes_or_is_refused() {
    let r = Repo::new();
    // No section: refused, with the form to add.
    let none = brief_with(&r, "y1", "");
    assert_eq!(none.facts["refused"], "closing-form", "{}", none.out);
    assert!(
        none.has("## Closing it") && none.has("Nothing to close: <why>"),
        "{}",
        none.out
    );
    assert!(none.has("gdd seal y1 gdd/briefs/y1.md"), "{}", none.out);
    // The template left as it came: refused too.
    let template = brief_with(&r, "y1", "Expected: <what changes>\nSignal: s\nWhere: w\n");
    assert_eq!(
        template.facts["refused"], "closing-form",
        "{}",
        template.out
    );
    // Both forms at once say nothing.
    let both = brief_with(&r, "y1", &format!("{THREE}Nothing to close: a rename\n"));
    assert_eq!(both.facts["refused"], "closing-form", "{}", both.out);
    assert!(r.log_lines("y1").iter().all(|e| e["step"] != "brief"));
    // The three lines: kept in the brief step, and shown with what the person approves.
    let b = brief_with(&r, "y1", THREE);
    assert_eq!(b.code, 0, "{}", b.out);
    let step = r.log_lines("y1").pop().unwrap();
    assert_eq!(
        step["closing"]["expected"],
        "people find the status at once"
    );
    assert_eq!(
        step["closing"]["signal"],
        "fewer messages asking whether it is up"
    );
    assert_eq!(step["closing"]["where"], "the support inbox");
    assert!(
        b.has("closes: expected people find the status at once; signal fewer messages asking whether it is up; looked at the support inbox"),
        "{}",
        b.out
    );
}

#[test]
fn nothing_to_close_is_a_line_the_person_approves_too() {
    let r = Repo::new();
    let b = brief_with(&r, "y1", "Nothing to close: it only renames a file\n");
    assert_eq!(b.code, 0, "{}", b.out);
    let step = r.log_lines("y1").pop().unwrap();
    assert_eq!(step["closing"]["nothing"], "it only renames a file");
    assert!(
        b.has("closes: nothing to close, it only renames a file"),
        "{}",
        b.out
    );
}

#[test]
fn a_change_that_promised_something_waits_for_a_verdict_and_stops_nothing() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.code, 0, "{}", n.out);
    assert_eq!(n.facts["state"]["next"]["step"], "in-use", "{}", n.out);
    assert_eq!(n.facts["state"]["close"], "waiting");
    // Close never starts by itself: the step names no command to run.
    assert_eq!(n.facts["state"]["next"]["command"], "");
    assert!(!n.has("run: "), "{}", n.out);
    assert!(
        n.has("expected: people find the status at once"),
        "{}",
        n.out
    );
    assert!(n.has("Close never starts by itself"), "{}", n.out);
    // The project's next keeps it in sight, with how long it has been in use.
    let p = r.gdd(&["next"]);
    assert!(
        p.has(
            "in use, waiting for a verdict: y1, 0 minutes; expected: people find the status at once"
        ),
        "{}",
        p.out
    );
    assert_eq!(p.facts["in_use"][0]["id"], "y1");
    // Another change starts as if nothing waited.
    let b = r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]);
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(b.facts["next"], "build", "{}", b.out);
    assert_eq!(r.gdd(&["verify", "y1"]).facts["errors"], 0);
}

#[test]
fn the_person_gives_the_verdict_and_only_a_yes_or_a_no_closes() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    // Looking writes nothing: it says what to gather, from where not, and names the question.
    let lines = r.log_lines("y1").len();
    let look = r.gdd(&["close", "y1"]);
    assert_eq!(look.code, 0, "{}", look.out);
    for said in [
        "expected: people find the status at once",
        "signal: fewer messages asking whether it is up",
        "where: the support inbox",
        "Never from the builder's report",
        "propose no verdict",
        "run: gdd ask y1 --of close",
    ] {
        assert!(look.has(said), "{said}: {}", look.out);
    }
    assert_eq!(r.log_lines("y1").len(), lines);
    // The question waits for the person, and what answers it is their verdict.
    let ask = r.gdd(&[
        "ask",
        "y1",
        "--of",
        "close",
        "Fewer messages were expected. The inbox has 3 this week and 9 the week before. Did it help: yes, not yet or no?",
    ]);
    assert_eq!(ask.code, 0, "{}", ask.out);
    assert!(ask.has("gdd close y1 --verdict"), "{}", ask.out);
    let asked = r.log_lines("y1").len();
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "wait", "{}", n.out);
    assert_eq!(n.facts["state"]["next"]["blocked"], "needs-person");
    assert_eq!(n.code, 0, "a wait for the person is no failure: {}", n.out);
    // A not-yet keeps it in use and in sight.
    let not_yet = verdict(&r, "y1", "not-yet", "Too early: one week of messages");
    assert_eq!(not_yet.code, 0, "{}", not_yet.out);
    assert_eq!(not_yet.facts["next"], "in-use", "{}", not_yet.out);
    let event = r.log_lines("y1").pop().unwrap();
    assert_eq!(event["event"], "close");
    assert_eq!(event["verdict"], "not-yet");
    assert_eq!(event["who"], "Person");
    assert_eq!(event["ask"], asked);
    let n = r.gdd(&["next"]);
    assert!(
        n.has("in use, waiting for a verdict: y1, 0 minutes, Person looked 0 minutes ago and said not-yet"),
        "{}",
        n.out
    );
    // A yes closes it, and nothing follows.
    let yes = verdict(&r, "y1", "yes", "Three messages this month");
    assert_eq!(yes.code, 0, "{}", yes.out);
    assert_eq!(yes.facts["next"], "done", "{}", yes.out);
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.facts["state"]["close"], "closed", "{}", n.out);
    assert!(n.has("closed, Person said yes"), "{}", n.out);
    let again = verdict(&r, "y1", "no", "On second thought");
    assert_eq!(again.facts["refused"], "out-of-order", "{}", again.out);
    let ask = r.gdd(&["ask", "y1", "--of", "close", "Did it help?"]);
    assert_eq!(ask.facts["refused"], "out-of-order", "{}", ask.out);
    assert!(!r.gdd(&["next"]).has("in use, waiting"));
    assert_eq!(r.gdd(&["verify", "y1"]).facts["errors"], 0);
}

#[test]
fn a_no_closes_and_names_its_two_ways_out() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    let no = r.gdd(&[
        "close",
        "y1",
        "--verdict",
        "no",
        "--who",
        "Person",
        "--where",
        "conversation",
        "--saw",
        "As many messages as before",
        "--then",
        "none",
    ]);
    assert_eq!(no.code, 0, "{}", no.out);
    assert!(no.has("a new request in the backlog"), "{}", no.out);
    assert!(no.has("load gdd-harden"), "{}", no.out);
    assert_eq!(no.facts["next"], "done", "{}", no.out);
    let event = r.log_lines("y1").pop().unwrap();
    assert_eq!(event["verdict"], "no");
    assert_eq!(event["then"], "none");
    // Its log is not opened again: what comes next has an id of its own.
    let brief = r.gdd(&["brief", "y1", "--paths", "src/core/app.txt"]);
    assert_eq!(brief.facts["refused"], "out-of-order", "{}", brief.out);
}

#[test]
fn a_verdict_needs_a_change_in_use_a_person_and_what_they_saw() {
    let r = Repo::new();
    assert_eq!(
        r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    let early = verdict(&r, "g1", "yes", "It looks fine");
    assert_eq!(early.facts["refused"], "out-of-order", "{}", early.out);
    let ask = r.gdd(&["ask", "g1", "--of", "close", "Did it help?"]);
    assert_eq!(ask.facts["refused"], "out-of-order", "{}", ask.out);
    assert_eq!(r.gdd(&["close", "g1"]).facts["refused"], "out-of-order");
    yellow_shipped(&r, "y1", THREE);
    let lines = r.log_lines("y1").len();
    assert_eq!(verdict(&r, "y1", "maybe", "It looks fine").code, 2);
    assert_eq!(verdict(&r, "y1", "yes", " ").code, 2);
    assert_eq!(verdict(&r, "y1", "yes", "<what they saw>").code, 2);
    let nobody = r.gdd(&["close", "y1", "--verdict", "yes", "--saw", "Fewer messages"]);
    assert_eq!(nobody.code, 2, "{}", nobody.out);
    assert_eq!(r.log_lines("y1").len(), lines);
}

#[test]
fn what_promised_nothing_closes_by_itself_after_the_blueprints_days() {
    let r = repo_with_days();
    green_shipped(&r, "g1");
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "done", "{}", n.out);
    assert_eq!(n.facts["state"]["close"], "quiet", "{}", n.out);
    assert!(
        n.has("it closes by itself in 7 days 0 hours")
            || n.has("it closes by itself in 6 days 23 hours"),
        "{}",
        n.out
    );
    assert!(!r.gdd(&["next"]).has("in use, waiting"));
    // In the meantime the person named in the blueprint can still say something.
    let look = r.gdd(&["close", "g1"]);
    assert!(
        look.has("the owner can still give a verdict"),
        "{}",
        look.out
    );
    assert!(!look.has("run: "), "{}", look.out);
    // Its days go by with nothing recorded against it: closed, and no verdict follows.
    let later = in_days(8);
    let n = r.gdd_env(&["next", "g1"], &[("GDD_NOW", &later)]);
    assert_eq!(n.facts["state"]["close"], "closed", "{}", n.out);
    assert!(n.has("closed, by itself"), "{}", n.out);
    let late = r.gdd_env(
        &[
            "close",
            "g1",
            "--verdict",
            "no",
            "--who",
            "Person",
            "--where",
            "conversation",
            "--saw",
            "The page broke",
        ],
        &[("GDD_NOW", &later)],
    );
    assert_eq!(late.facts["refused"], "out-of-order", "{}", late.out);
    assert!(late.has("load gdd-harden"), "{}", late.out);
}

#[test]
fn a_hardening_record_keeps_it_from_closing_by_itself() {
    let r = repo_with_days();
    green_shipped(&r, "g1");
    let h = r.gdd(&[
        "step",
        "blueprint",
        "harden",
        "--change",
        "g1",
        "--crack",
        "no check read the page",
        "--proposal",
        "a check that reads the page",
    ]);
    assert_eq!(h.code, 0, "{}", h.out);
    let later = in_days(8);
    let n = r.gdd_env(&["next", "g1"], &[("GDD_NOW", &later)]);
    assert_eq!(n.facts["state"]["close"], "waiting", "{}", n.out);
    assert!(n.has("a hardening record names it"), "{}", n.out);
    let p = r.gdd_env(&["next"], &[("GDD_NOW", &later)]);
    assert!(
        p.has("in use, waiting for a verdict: g1, 8 days"),
        "{}",
        p.out
    );
    let no = r.gdd_env(
        &[
            "close",
            "g1",
            "--verdict",
            "no",
            "--who",
            "Person",
            "--where",
            "conversation",
            "--saw",
            "The page broke",
        ],
        &[("GDD_NOW", &later)],
    );
    assert_eq!(no.code, 0, "{}", no.out);
}

#[test]
fn a_log_from_before_reads_as_closed_when_it_shipped() {
    // The blueprint names no days and the brief no closing, as before the layer knew Close.
    let r = Repo::new();
    green_shipped(&r, "g1");
    let n = r.gdd(&["next", "g1"]);
    assert_eq!(n.facts["state"]["next"]["step"], "done", "{}", n.out);
    assert_eq!(n.facts["state"]["close"], "closed", "{}", n.out);
    // A hardening record that names it afterwards doesn't open it.
    let h = r.gdd(&[
        "step",
        "blueprint",
        "harden",
        "--change",
        "g1",
        "--crack",
        "no check read the page",
        "--proposal",
        "a check that reads the page",
    ]);
    assert_eq!(h.code, 0, "{}", h.out);
    let later = in_days(1);
    let n = r.gdd_env(&["next", "g1"], &[("GDD_NOW", &later)]);
    assert_eq!(n.facts["state"]["close"], "closed", "{}", n.out);
    assert!(!r.gdd(&["next"]).has("in use, waiting"));
    assert_eq!(r.gdd(&["verify", "--all"]).facts["errors"], 0);
}

#[test]
fn a_verdict_written_wrong_is_voided_and_the_change_waits_again() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    let shipped = r.log_lines("y1").len().to_string();
    assert_eq!(verdict(&r, "y1", "yes", "Fewer messages").code, 0);
    let line = r.log_lines("y1").len().to_string();
    // No line that judged the change is voided once it shipped. Its verdict can be.
    let ship = r.gdd(&["correct", "y1", "--line", &shipped, "--void", "reopen it"]);
    assert_eq!(ship.facts["refused"], "out-of-order", "{}", ship.out);
    let void = r.gdd(&[
        "correct",
        "y1",
        "--line",
        &line,
        "--void",
        "they said not yet",
    ]);
    assert_eq!(void.code, 0, "{}", void.out);
    let n = r.gdd(&["next", "y1"]);
    assert_eq!(n.facts["state"]["close"], "waiting", "{}", n.out);
    assert_eq!(verdict(&r, "y1", "not-yet", "Too early").code, 0);
}

#[test]
fn verify_finds_a_verdict_written_before_the_change_shipped() {
    let r = Repo::new();
    assert_eq!(
        r.gdd(&["brief", "g1", "--paths", "src/ui/page.txt"]).code,
        0
    );
    let mut log = r.read("gdd/log/g1.jsonl");
    log.push_str(&format!(
        "{{\"contract\":\"gdd/1\",\"ts\":\"{}\",\"brief\":\"g1\",\"event\":\"close\",\"verdict\":\"yes\",\"who\":\"Person\",\"where\":\"conversation\",\"saw\":\"It looks fine\"}}\n",
        in_days(0)
    ));
    r.write("gdd/log/g1.jsonl", &log);
    let v = r.gdd(&["verify", "g1", "--no-git"]);
    assert_eq!(v.code, 1, "{}", v.out);
    assert!(v.has("a verdict before the change shipped"), "{}", v.out);
}

#[test]
fn only_a_change_is_closed() {
    let r = repo_with_days();
    for id in ["standing-checks", "blueprint"] {
        let look = r.gdd(&["close", id]);
        assert_eq!(look.facts["refused"], "out-of-order", "{}", look.out);
        let ask = r.gdd(&["ask", id, "--of", "close", "Did it help?"]);
        assert_eq!(ask.facts["refused"], "out-of-order", "{}", ask.out);
        assert_eq!(verdict(&r, id, "yes", "It looks fine").code, 1);
        assert!(!r.path(&format!("gdd/log/{id}.jsonl")).is_file());
    }
}

#[test]
fn how_it_closes_is_part_of_what_the_person_approved() {
    let r = Repo::new();
    assert_eq!(brief_with(&r, "y1", THREE).code, 0);
    let approve = |r: &Repo| {
        r.gdd(&[
            "approve",
            "y1",
            "--of",
            "brief",
            "--who",
            "Person",
            "--where",
            "conversation",
            "features/y1.feature",
        ])
    };
    assert_eq!(approve(&r).code, 0);
    assert_eq!(r.gdd(&["next", "y1"]).facts["state"]["approved"], true);
    // Promising nothing instead, once approved, asks for the yes again and shows the new line.
    let b = brief_with(&r, "y1", "Nothing to close: nobody would notice");
    assert_eq!(b.code, 0, "{}", b.out);
    assert_eq!(b.facts["next"], "ask", "{}", b.out);
    assert!(
        b.has("closes: nothing to close, nobody would notice"),
        "{}",
        b.out
    );
    // A brief step that names no file keeps what the last one said.
    assert_eq!(approve(&r).code, 0);
    let again = r.gdd(&["brief", "y1", "--paths", "src/core/app.txt"]);
    assert_eq!(again.code, 0, "{}", again.out);
    assert_eq!(
        r.log_lines("y1").pop().unwrap()["closing"]["nothing"],
        "nobody would notice"
    );
}

#[test]
fn a_first_brief_with_files_needs_the_brief_that_says_how_it_closes() {
    let r = Repo::new();
    let (other, feature) = ("gdd/briefs/y1-brief.md", "features/y1.feature");
    r.write_as_it_is(other, &brief_text("y1", THREE));
    r.write(feature, &ok_feature("y1"));
    assert_eq!(r.gdd(&["seal", "y1", other, feature]).code, 0);
    for files in [vec![feature], vec![other, feature]] {
        let mut args = vec!["brief", "y1"];
        args.extend(files);
        args.extend(["--paths", "src/core/app.txt"]);
        let b = r.gdd(&args);
        assert_eq!(b.facts["refused"], "closing-form", "{}", b.out);
        assert!(b.has("gdd seal y1 gdd/briefs/y1.md"), "{}", b.out);
    }
}

#[test]
fn where_no_approval_is_due_the_person_still_hears_how_it_closes() {
    let r = Repo::with(&BLUEPRINT.replace(
        "checks: [brief-approval, seal, tests, slow, review]",
        "checks: [seal, tests, slow, review]",
    ));
    let b = brief_with(&r, "y1", "Nothing to close: it only renames a file");
    assert_eq!(b.code, 0, "{}", b.out);
    assert!(
        b.has("Tell the person what will close it, in a line: closes: nothing to close, it only renames a file"),
        "{}",
        b.out
    );
}

#[test]
fn the_wait_for_a_verdict_is_no_work_on_the_change() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    let before = r.gdd(&["limits", "y1"]).out;
    let later = in_days(2);
    let ask = r.gdd_env(
        &["ask", "y1", "--of", "close", "Did it help?"],
        &[("GDD_NOW", &later)],
    );
    assert_eq!(ask.code, 0, "{}", ask.out);
    let yes = r.gdd_env(
        &[
            "close",
            "y1",
            "--verdict",
            "yes",
            "--who",
            "Person",
            "--where",
            "conversation",
            "--saw",
            "Fewer messages",
        ],
        &[("GDD_NOW", &in_days(3))],
    );
    assert_eq!(yes.code, 0, "{}", yes.out);
    let after = r
        .gdd_env(&["limits", "y1"], &[("GDD_NOW", &in_days(3))])
        .out;
    let minutes = |out: &str| {
        out.lines()
            .find(|l| l.starts_with("minutes"))
            .map(str::to_string)
    };
    assert_eq!(minutes(&after), minutes(&before), "{after}");
}

#[test]
fn the_answer_to_whether_it_helped_is_a_verdict() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    assert_eq!(
        r.gdd(&["ask", "y1", "--of", "close", "Did it help?"]).code,
        0
    );
    let answer = r.gdd(&["answer", "y1", "no, nobody uses it"]);
    assert_eq!(answer.facts["refused"], "verdict", "{}", answer.out);
    assert!(answer.has("gdd close y1 --verdict"), "{}", answer.out);
    // Looking again while the question is out names the verdict, not a second question.
    let look = r.gdd(&["close", "y1"]);
    assert_eq!(look.code, 0, "{}", look.out);
    assert!(look.has("run: gdd close y1 --verdict"), "{}", look.out);
    assert!(!look.has("--of close"), "{}", look.out);
}

#[test]
fn what_promised_nothing_waits_once_someone_looked_or_asked() {
    let r = repo_with_days();
    green_shipped(&r, "g1");
    assert_eq!(verdict(&r, "g1", "not-yet", "Too early to say").code, 0);
    let later = in_days(8);
    let n = r.gdd_env(&["next", "g1"], &[("GDD_NOW", &later)]);
    assert_eq!(n.facts["state"]["close"], "waiting", "{}", n.out);
    assert!(
        n.has("it promised nothing, and Person said not yet"),
        "{}",
        n.out
    );
    green_shipped(&r, "g2");
    assert_eq!(
        r.gdd(&["ask", "g2", "--of", "close", "Did it help?"]).code,
        0
    );
    let n = r.gdd_env(&["next", "g2"], &[("GDD_NOW", &later)]);
    assert_eq!(n.facts["state"]["close"], "waiting", "{}", n.out);
    assert_eq!(n.facts["state"]["next"]["step"], "wait", "{}", n.out);
    let no = r.gdd_env(
        &[
            "close",
            "g2",
            "--verdict",
            "no",
            "--who",
            "Person",
            "--where",
            "conversation",
            "--saw",
            "It broke the page",
        ],
        &[("GDD_NOW", &later)],
    );
    assert_eq!(no.code, 0, "{}", no.out);
}

#[test]
fn a_hardening_record_from_before_the_ship_names_nothing_in_use() {
    let r = repo_with_days();
    green_shipped(&r, "g1");
    let h = r.gdd_env(
        &[
            "step",
            "blueprint",
            "harden",
            "--change",
            "g1",
            "--crack",
            "found in review",
            "--proposal",
            "a check",
        ],
        &[("GDD_NOW", &in_days(-1))],
    );
    assert_eq!(h.code, 0, "{}", h.out);
    let n = r.gdd_env(&["next", "g1"], &[("GDD_NOW", &in_days(8))]);
    assert_eq!(n.facts["state"]["close"], "closed", "{}", n.out);
}

#[test]
fn a_change_named_by_hardening_has_nothing_to_gather_and_metrics_see_it() {
    let r = repo_with_days();
    green_shipped(&r, "g1");
    let h = r.gdd(&[
        "step",
        "blueprint",
        "harden",
        "--change",
        "g1",
        "--crack",
        "no check read the page",
        "--proposal",
        "a check that reads the page",
    ]);
    assert_eq!(h.code, 0, "{}", h.out);
    let look = r.gdd(&["close", "g1"]);
    assert!(
        look.has("there is nothing written to gather"),
        "{}",
        look.out
    );
    assert!(look.has("run: gdd ask g1 --of close"), "{}", look.out);
    assert!(!look.has("Gather what is seen"), "{}", look.out);
    for args in [vec!["metrics", "g1"], vec!["metrics", "--all"]] {
        let m = r.gdd(&args);
        let compact = m.out.replace(": ", ":");
        assert!(
            compact.contains("\"close\":\"waiting\""),
            "{args:?}: {}",
            m.out
        );
    }
    // Voiding the record that named it is said by verify.
    let void = r.gdd(&[
        "correct",
        "blueprint",
        "--line",
        "1",
        "--void",
        "wrong change",
    ]);
    assert_eq!(void.code, 0, "{}", void.out);
    assert!(
        r.gdd(&["verify", "blueprint"])
            .has("a hardening record that names g1")
    );
}

#[test]
fn only_a_yes_or_a_no_closes_and_the_flags_say_what_they_hold() {
    let r = Repo::new();
    yellow_shipped(&r, "y1", THREE);
    let mut log = r.read("gdd/log/y1.jsonl");
    log.push_str(&format!(
        "{{\"contract\":\"gdd/1\",\"ts\":\"{}\",\"brief\":\"y1\",\"event\":\"close\",\"verdict\":\"maybe\",\"who\":\"Person\",\"where\":\"conversation\",\"saw\":\"x\"}}\n",
        in_days(0)
    ));
    r.write("gdd/log/y1.jsonl", &log);
    assert_eq!(r.gdd(&["next", "y1"]).facts["state"]["close"], "waiting");
    let then = r.gdd(&[
        "close",
        "y1",
        "--verdict",
        "no",
        "--who",
        "Person",
        "--where",
        "conversation",
        "--saw",
        "Nobody uses it",
        "--then",
        "<the backlog item>",
    ]);
    assert_eq!(then.code, 2, "{}", then.out);
    let stray = r.gdd(&["close", "y1", "--who", "Person", "--saw", "Nobody uses it"]);
    assert_eq!(stray.code, 2, "{}", stray.out);
}

#[test]
fn an_unreadable_closing_in_the_blueprint_is_said() {
    for closing in [
        "closing:\n  days: 7 days\n",
        "closing:\n  days: -3\n",
        "closing: 7\n",
        "closing:\n  who: [Mar, Ana]\n",
    ] {
        let r = Repo::with(&BLUEPRINT.replace("context: []", &format!("{closing}context: []")));
        let n = r.gdd(&["next"]);
        assert!(n.has("the blueprint: closing."), "{closing}: {}", n.out);
    }
}

#[test]
fn what_nobody_had_checked_is_the_first_thing_to_look_at() {
    let r = Repo::new();
    let rules = format!(
        "{}\n  @unverified\n  Rule: The carrier answers within a day\n",
        ok_feature("u1")
    );
    shipped_with_rules(&r, "u1", THREE, &rules);
    let look = r.gdd(&["close", "u1"]);
    assert_eq!(look.code, 0, "{}", look.out);
    assert!(look.has("nobody had checked them"), "{}", look.out);
    assert!(
        look.has("unchecked: The carrier answers within a day"),
        "{}",
        look.out
    );

    // A change that took nothing on trust says nothing about it.
    let plain = Repo::new();
    yellow_shipped(&plain, "y1", THREE);
    let look = plain.gdd(&["close", "y1"]);
    assert!(!look.has("unchecked"), "{}", look.out);
}
