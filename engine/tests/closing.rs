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
    let (md, feature) = (
        format!("gdd/briefs/{id}.md"),
        format!("features/{id}.feature"),
    );
    r.write_as_it_is(&md, &brief_text(id, closing));
    r.write(&feature, &ok_feature(id));
    assert_eq!(r.gdd(&["seal", id, &md, &feature]).code, 0);
    r.gdd(&["brief", id, &md, &feature, "--paths", "src/core/app.txt"])
}

fn merged_and_shipped(r: &Repo, id: &str) {
    let rv = r.report(id, "review", &r.review_pass(id, "yellow", ""), &[]);
    assert_eq!(rv.code, 0, "{}", rv.out);
    let s = r.gdd(&["ship", id, "--merge"]);
    assert_eq!(s.code, 0, "{}", s.out);
    // What the ship says next is that it is in use, with no command to run.
    assert_eq!(s.facts["next"], "close", "{}", s.out);
    assert_eq!(s.facts["next_command"], "", "{}", s.out);
}

/// A yellow change whose brief says `closing`, through every step and in the main branch.
fn yellow_shipped(r: &Repo, id: &str, closing: &str) {
    let b = brief_with(r, id, closing);
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
        &[("src/ui/page.txt", "a better page\n")],
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
    assert_eq!(n.facts["state"]["next"]["step"], "close", "{}", n.out);
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
        "Fewer messages were expected. The inbox has 3 this week and 9 the week before. Did it serve: yes, not yet or no?",
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
    assert_eq!(not_yet.facts["next"], "close", "{}", not_yet.out);
    let event = r.log_lines("y1").pop().unwrap();
    assert_eq!(event["event"], "close");
    assert_eq!(event["verdict"], "not-yet");
    assert_eq!(event["who"], "Person");
    assert_eq!(event["ask"], asked);
    let n = r.gdd(&["next"]);
    assert!(
        n.has("in use, waiting for a verdict: y1, 0 minutes, looked at 0 minutes ago: not-yet"),
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
    let ask = r.gdd(&["ask", "y1", "--of", "close", "Did it serve?"]);
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
    let ask = r.gdd(&["ask", "g1", "--of", "close", "Did it serve?"]);
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
