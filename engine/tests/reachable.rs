//! Walk small changes through the public CLI. The seed chooses role outputs, including reports
//! that make no progress. Only the commands (and the roles' ordinary git commits) create state.

mod common;
use common::*;
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

#[test]
fn each_reachable_step_has_a_way_on() {
    let mut choices = Choices(0x5ea1);
    let mut visited = BTreeSet::new();
    for path in ["src/ui/page.txt", "src/core/app.txt", "src/pay/pay.txt"] {
        let r = Repo::new();
        let id = "example";
        let brief = "gdd/briefs/example.md";
        let scenario = "features/example.feature";
        // Choose when a role gives no usable report, and what its next completed review says.
        let mut bad_reports = choices.pick(3);
        let mut send_back = choices.pick(3);
        let mut builds = 0;
        let mut consecutive_refusals = 0;
        let mut finished = false;
        for _ in 0..28 {
            let n = r.gdd(&["next", id]);
            let state = &n.facts["state"];
            let next = &state["next"];
            let step = next["step"].as_str().expect(&n.out);
            let command = next["command"].as_str().expect(&n.out);
            visited.insert(step.to_owned());
            if consecutive_refusals == 2 {
                assert_eq!(next["blocked"], "needs-person", "{}", n.out);
                assert!(
                    command.starts_with("gdd ask example --of decision "),
                    "{}",
                    n.out
                );
                consecutive_refusals = 0;
            }
            match step {
                "brief" => {
                    assert!(command.starts_with("gdd brief example --paths <path>..."));
                    if path.contains("/ui/") {
                        follow(&r, &format!("gdd brief example --paths {path}"));
                    } else if path.contains("/pay/") {
                        brief_with_supplied_file(&r);
                    } else {
                        yellow_brief(&r, id, path);
                    }
                }
                "stopped" if command.starts_with("gdd brief example --stop ") => {
                    assert_eq!(next["blocked"], "cap-spent");
                    follow(&r, &command.replace("<their words>", "Leave it pending"));
                    let dropped = r.gdd(&["next", id]);
                    assert_eq!(dropped.facts["state"]["next"]["blocked"], "cannot");
                    assert!(dropped.has("a new brief takes it up again"));
                    finished = true;
                    break;
                }
                "ask" | "stopped" => {
                    assert!(next["blocked"].is_string(), "{}", n.out);
                    let (ask, _) = command.split_once(" \"").expect(command);
                    assert!(ask.starts_with("gdd ask example --of "), "{command}");
                    // Repeated reports while a required approval or the reviewer's stop waits
                    // must leave that original question in place.
                    if ask.ends_with("--of brief") || ask.ends_with("--of stop") {
                        for _ in 0..2 {
                            assert_eq!(
                                r.report(id, "build", "no role output", &[]).facts["refused"],
                                "no-report"
                            );
                        }
                        assert_eq!(r.gdd(&["next", id]).facts["state"]["next"], *next);
                    }
                    follow(&r, &format!("{ask} \"Shall we use this example?\""));
                }
                "wait" => {
                    assert_eq!(next["blocked"], "needs-person");
                    if command.starts_with("gdd approve ") {
                        follow(
                            &r,
                            &command
                                .replace("<person>", "Owner")
                                .replace("<conversation, commit or pull request>", "conversation")
                                .replace("<files>", &format!("{brief} {scenario}")),
                        );
                    } else if state["open_ask"]["of"] == "waiver" {
                        let reply = command
                            .strip_prefix("If they agree to leave it pending: ")
                            .unwrap();
                        follow(
                            &r,
                            &reply
                                .split_once(", then")
                                .unwrap()
                                .0
                                .replace("<their words>", "Leave it pending"),
                        );
                    } else {
                        follow(&r, &command.replace("<their words>", "Yes"));
                    }
                }
                "checks" | "reading" | "build" | "review" => {
                    assert!(next["blocked"].is_null());
                    assert!(command.starts_with(&format!("gdd launch {id} {step}")));
                    follow(&r, command);
                    if bad_reports > 0 && (consecutive_refusals > 0 || choices.pick(2) == 0) {
                        bad_reports -= 1;
                        let refused = r.report(id, step, "no role output", &[]);
                        assert_eq!(refused.facts["refused"], "no-report", "{}", refused.out);
                        consecutive_refusals += 1;
                        continue;
                    }
                    consecutive_refusals = 0;
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
                            builds += 1;
                            let from = (builds == 1).then(|| state["start"].as_str().unwrap());
                            r.commit_on(
                                "work/example",
                                from,
                                &[(path, &format!("ok {builds}\n"))],
                                "Build the example",
                            );
                            report_block("gdd-build", "done")
                        }
                        _ => {
                            let verdict = ["pass", "fix", "stop"][send_back as usize];
                            send_back = 0;
                            review_text(
                                verdict,
                                state["color"].as_str().unwrap(),
                                "f1 example:1 The result is missing | @id:example-1",
                            )
                        }
                    };
                    accepted(r.report(id, step, &text, &[]));
                }
                "gates" => follow(&r, command),
                "ship" => follow(&r, command.split_once("; or ").unwrap().0),
                "done" => {
                    assert!(state["shipped"].as_bool().unwrap());
                    finished = true;
                    break;
                }
                _ => panic!("no way on for {path}: {}", n.out),
            }
        }
        assert!(
            finished,
            "{path} had neither shipped nor been deferred within 28 steps"
        );
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
