//! Walk small changes through the public CLI. The seed chooses role outputs, including reports
//! that make no progress. Only the commands (and the roles' ordinary git commits) create state.

mod common;
use common::*;
use serde_json::Value;
use std::collections::BTreeSet;

struct Choices(u32);

impl Choices {
    fn pick(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 16) % n
    }
}

fn accepted(run: Run) {
    assert_eq!(run.code, 0, "{}", run.out);
}

/// Run the command next printed, filling the slots that belong to the fixture.
fn follow(r: &Repo, command: &str) {
    accepted(r.named(command));
}

/// Decisions 70 and 74: eleven examples travel together with a file the owner supplied.
fn brief_with_supplied_file(r: &Repo) {
    r.write("tests/run.sh", "test -s examples/quantities.txt\n");
    r.commit("Use the supplied quantities when checking the change");
    r.write("examples/quantities.txt", "3\n");
    assert!(!git_ok(
        &r.root,
        &["ls-files", "--error-unmatch", "examples/quantities.txt"]
    ));
    r.write(
        "gdd/briefs/example.md",
        "# Accept the supplied quantities\n",
    );
    let mut feature = String::from("Feature: Supplied quantities\n");
    for i in 1..=11 {
        feature.push_str(&format!("\n  @brief:example @id:example-{i}\n  Scenario: Quantity {i}\n    Given {i} items\n    Then the receipt lists {i} items\n"));
    }
    r.write("features/example.feature", &feature);
    follow(
        r,
        "gdd seal example gdd/briefs/example.md features/example.feature examples/quantities.txt",
    );
    follow(
        r,
        "gdd brief example gdd/briefs/example.md features/example.feature examples/quantities.txt --paths src/pay/pay.txt",
    );
}

/// The fixture role reports one rejecting run per scenario and one passing run of its suite.
fn checks_report(cases: u32) -> String {
    use serde_json::json;
    let mut proofs = Vec::new();
    for i in 1..=cases {
        proofs.push(json!({"check":"tests","test":format!("example-{i}"),"wrong":"drops one item",
            "case":"rejects","command":format!("check quantity {i}"),"exit":1,"tail":"one item missing"}));
    }
    proofs.push(json!({"check":"tests","case":"passes","command":"check all quantities","exit":0,"tail":"ok"}));
    report_text(
        "gdd-checks",
        "sealed",
        &format!("```json\n{}\n```", json!(proofs)),
    )
}

/// These two decisions offer alternatives in a single command string. The fixture
/// chooses an offered command, just as the person chooses whether to defer or ship locally.
fn offered(command: &str, choice: &str) {
    assert!(command.contains(choice), "{choice} missing from {command}");
}

fn answer_question(r: &Repo, state: &Value, command: &str) {
    if command.starts_with("gdd approve ") {
        follow(
            r,
            &command
                .replace("<person>", "Owner")
                .replace("<conversation, commit or pull request>", "conversation")
                .replace("<files>", "gdd/briefs/example.md features/example.feature"),
        );
    } else if state["open_ask"]["of"] == "waiver" {
        let answer = r#"gdd answer example "<their words>""#;
        offered(command, answer);
        follow(r, &answer.replace("<their words>", "Leave it pending"));
    } else {
        follow(r, &command.replace("<their words>", "Yes"));
    }
}

fn ask(r: &Repo, next: &Value, command: &str) {
    let args = gdd::launch::split(command).unwrap();
    assert_eq!(&args[..4], ["gdd", "ask", "example", "--of"]);
    assert_eq!(args.len(), 6);
    if matches!(args[4].as_str(), "brief" | "stop") {
        for _ in 0..2 {
            assert_eq!(
                r.report("example", "build", "no role output", &[]).facts["refused"],
                "no-report"
            );
        }
        assert_eq!(r.gdd(&["next", "example"]).facts["state"]["next"], *next);
    }
    accepted(r.gdd(&[
        "ask",
        "example",
        "--of",
        &args[4],
        "Shall we use this example?",
    ]));
}

/// The seed chooses missing reports and what the first completed review says.
struct Roles {
    bad_reports: u32,
    send_back: u32,
    builds: u32,
    consecutive_refusals: u32,
}

impl Roles {
    fn report(&mut self, r: &Repo, path: &str, state: &Value, step: &str, choices: &mut Choices) {
        if self.bad_reports > 0 && (self.consecutive_refusals > 0 || choices.pick(2) == 0) {
            self.bad_reports -= 1;
            let refused = r.report("example", step, "no role output", &[]);
            assert_eq!(refused.facts["refused"], "no-report", "{}", refused.out);
            self.consecutive_refusals += 1;
            return;
        }
        self.consecutive_refusals = 0;
        let text = match step {
            "reading" => report_block("gdd-review", "pass"),
            "checks" => {
                r.commit_on(
                    "gdd-seal/example",
                    None,
                    &[("tests/example.sh", "true\n")],
                    "Seal the example",
                );
                checks_report(if path.contains("/pay/") { 11 } else { 1 })
            }
            "build" => {
                self.builds += 1;
                let from = (self.builds == 1).then(|| state["start"].as_str().unwrap());
                r.commit_on(
                    "work/example",
                    from,
                    &[(path, &format!("ok {}\n", self.builds))],
                    "Build the example",
                );
                report_block("gdd-build", "done")
            }
            "review" => {
                let verdict = ["pass", "fix", "stop"][self.send_back as usize];
                self.send_back = 0;
                review_text(
                    verdict,
                    state["color"].as_str().unwrap(),
                    "f1 example:1 The result is missing | @id:example-1",
                )
            }
            _ => unreachable!(),
        };
        accepted(r.report("example", step, &text, &[]));
    }
}

fn walk_change(path: &str, choices: &mut Choices, visited: &mut BTreeSet<String>) {
    let r = Repo::new();
    let mut roles = Roles {
        bad_reports: choices.pick(3),
        send_back: choices.pick(3),
        builds: 0,
        consecutive_refusals: 0,
    };
    for _ in 0..28 {
        let n = r.gdd(&["next", "example"]);
        let state = &n.facts["state"];
        let next = &state["next"];
        let step = next["step"].as_str().expect(&n.out);
        let command = next["command"].as_str().expect(&n.out);
        visited.insert(step.to_owned());
        if roles.consecutive_refusals == 2 {
            assert_eq!(next["blocked"], "needs-person", "{}", n.out);
            assert!(
                command.starts_with("gdd ask example --of decision "),
                "{}",
                n.out
            );
            roles.consecutive_refusals = 0;
        }
        match step {
            "brief" => {
                assert!(command.starts_with("gdd brief example [<brief and scenario files>] --paths <path the change will touch>..."));
                if path.contains("/ui/") {
                    follow(&r, &format!("gdd brief example --paths {path}"));
                } else if path.contains("/pay/") {
                    brief_with_supplied_file(&r);
                } else {
                    yellow_brief(&r, "example", path);
                }
            }
            "stopped" if command.starts_with("gdd brief example --stop ") => {
                assert_eq!(next["blocked"], "cap-spent");
                follow(&r, &command.replace("<their words>", "Leave it pending"));
                let dropped = r.gdd(&["next", "example"]);
                assert_eq!(dropped.facts["state"]["next"]["blocked"], "cannot");
                assert!(dropped.has("a new brief takes it up again"));
                return;
            }
            "ask" | "stopped" => {
                assert!(next["blocked"].is_string(), "{}", n.out);
                if next["blocked"] == "cap-spent" {
                    offered(&n.out, "gdd brief example --stop");
                }
                ask(&r, next, command);
            }
            "wait" => {
                assert_eq!(next["blocked"], "needs-person");
                answer_question(&r, state, command);
            }
            "checks" | "reading" | "build" | "review" => {
                assert!(next["blocked"].is_null());
                assert!(command.starts_with(&format!("gdd launch example {step}")));
                follow(&r, command);
                roles.report(&r, path, state, step, choices);
            }
            "gates" => follow(&r, command),
            "ship" => {
                let merge = "gdd ship example --merge";
                offered(command, merge);
                follow(&r, merge);
            }
            "done" => {
                assert_eq!(state["shipped"], true);
                return;
            }
            _ => panic!("no way on for {path}: {}", n.out),
        }
    }
    panic!("{path} had neither shipped nor been deferred within 28 steps");
}

#[test]
fn each_reachable_step_has_a_way_on() {
    let mut choices = Choices(0x5ea1);
    let mut visited = BTreeSet::new();
    for path in ["src/ui/page.txt", "src/core/app.txt", "src/pay/pay.txt"] {
        walk_change(path, &mut choices, &mut visited);
    }
    for step in [
        "reading", "ask", "wait", "checks", "build", "gates", "review", "ship", "done",
    ] {
        assert!(visited.contains(step), "seed missed {step}");
    }
}

#[test]
fn a_repeat_before_the_first_event_shows_up_for_the_project() {
    let r = Repo::new();
    r.write("features/example.feature", "Feature: Example\n");
    for _ in 0..2 {
        let rejected = r.gdd(&[
            "brief",
            "example",
            "features/example.feature",
            "--paths",
            "src/core/app.txt",
        ]);
        assert_eq!(rejected.facts["refused"], "not-sealed", "{}", rejected.out);
    }
    let all = r.gdd(&["next"]);
    let item = &all.facts["under_way"][0];
    assert_eq!(item["id"], "example", "{}", all.out);
    assert_eq!(item["blocked"], "needs-person");
    follow(&r, item["command"].as_str().unwrap());
    assert_eq!(
        r.gdd(&["next", "example"]).facts["state"]["next"]["step"],
        "wait"
    );
}
