//! The single next step of a change, with the command that runs it, or why it is stopped and what
//! moves it. Read from where the log stands, what its limits allow and what git says.

use crate::blueprint::Blueprint;
use crate::judged::GitView;
use crate::limits::Limits;
use crate::state::{GatesRun, LogKind, Next, Progress, log_kind};

fn n(step: &str, say: impl Into<String>, command: impl Into<String>) -> Next {
    Next {
        step: step.into(),
        say: say.into(),
        command: command.into(),
        blocked: None,
    }
}

fn blocked(step: &str, code: &str, say: impl Into<String>, command: impl Into<String>) -> Next {
    Next {
        step: step.into(),
        say: say.into(),
        command: command.into(),
        blocked: Some(code.into()),
    }
}

/// The checks role's turn in a setup log. A spent cap comes first. One that stopped is launched
/// once more. One that stopped twice goes to the person, and so does every stop after their
/// answer: the same launch is never named again and again.
fn setup_checks(id: &str, p: &Progress, lim: &Limits, say: &str) -> Next {
    if !lim.stop.is_empty() {
        return cap_spent(id, lim);
    }
    if p.checks_stopped >= 2 {
        let answered = p
            .checks_line
            .is_some_and(|c| p.asks.iter().any(|a| a.line > c && a.answered));
        if !answered {
            return blocked(
                "stopped",
                "cannot",
                "gdd-checks stopped twice: what it can't set up goes to the person, with what you'd do. If they leave it for later, it stays in the backlog and the next request doesn't wait for it",
                format!("gdd ask {id} --of decision \"<what it can't set up, and what you'd do>\""),
            );
        }
        return n(
            "checks",
            "the person answered: launch gdd-checks once more, with their answer, unless they left it for later",
            format!("gdd launch {id} checks"),
        );
    }
    n("checks", say, format!("gdd launch {id} checks"))
}

/// What comes after the reviewer sent a change back: a stop goes to the person first, and then,
/// as after a `fix`, a fix round.
fn review_sent_back(id: &str, p: &Progress, lim: &Limits, verdict: &str) -> Next {
    if verdict == "stop" && !p.stop_told {
        return blocked(
            "stopped",
            "review-stop",
            "the reviewer said stop: the change doesn't go in. Tell the person why; a stop is never waived, it gets a fix and a new review",
            format!("gdd ask {id} --of stop \"<why it stopped>\""),
        );
    }
    let say = if verdict == "stop" {
        "a fix round after the reviewer's stop"
    } else {
        "a fix round with the reviewer's notes"
    };
    round_needs_room(id, lim, n("build", say, format!("gdd launch {id} build")))
}

/// What comes after the gates found what judges the change touched: the person is told, and then
/// a fix round puts it back as it was sealed.
fn seal_broken(id: &str, p: &Progress, lim: &Limits) -> Next {
    if !p.seal_told {
        return blocked(
            "stopped",
            "seal-broken",
            "what judges the change was touched: nothing else counts, the change stops, and the person is told",
            format!("gdd ask {id} --of stop \"<what was touched>\""),
        );
    }
    // Told: what judges it goes back as it was sealed, in a fix round, and the gates again.
    round_needs_room(
        id,
        lim,
        n(
            "build",
            format!(
                "a fix round that puts back what judges the change exactly as it was sealed (the seal check's output names the files), then the gates; if the person dropped it: gdd brief {id} --stop \"<their words>\""
            ),
            format!("gdd launch {id} build"),
        ),
    )
}

fn round_needs_room(id: &str, lim: &Limits, then: Next) -> Next {
    if lim.stop.is_empty() {
        return then;
    }
    cap_spent(id, lim)
}

/// A spent cap: no new round, and how more room is asked for. More room is always the person's
/// to give, by a waiver; for `standing-checks`, which every change waits for, there is no next
/// item to go on with, so the person is asked at once.
pub fn cap_spent(id: &str, lim: &Limits) -> Next {
    let caps = lim.stop.join(", ");
    let have = if lim.stop.len() == 1 { "has" } else { "have" };
    let spent = lim
        .lines
        .iter()
        .find(|l| lim.stop.iter().any(|s| s == l.name));
    let (name, key, used, cap) = spent
        .map(|l| {
            (
                l.name,
                l.key,
                l.used.trunc() as i64,
                l.cap.map(|c| c.to_string()).unwrap_or("<cap>".into()),
            )
        })
        .unwrap_or(("<cap>", "<key>", 0, "<cap>".into()));
    // The cap that gives one more round: a count of rounds takes one more; a count of rounds sent
    // back is spent past its cap, so it takes what was used.
    let to = match key {
        "rounds_per_change" | "runs_before_review" => (used + 1).to_string(),
        "fix_rounds_per_gate" => used.to_string(),
        _ => "<n>".into(),
    };
    let ask = format!(
        "gdd ask {id} --of waiver \"{name}: {used} of {cap} used. One more round would <what it buys>. Yes?\""
    );
    let backlog = if log_kind(id) == LogKind::Change {
        "<item>".to_string()
    } else {
        id.to_string()
    };
    let waive = format!(
        "gdd waiver {id} --who <person> --check {key} --why \"<their words>\" --backlog {backlog} --limit {key} --to {to}"
    );
    if id == "standing-checks" {
        return blocked(
            "stopped",
            "cap-spent",
            format!(
                "no new round: {caps} {have} no room left, and every change waits for the standing checks, so there is no next item to go on with. Ask the person at once for one more round: {ask}; after their yes: {waive}"
            ),
            ask,
        );
    }
    let ends = if log_kind(id) == LogKind::Change {
        format!(". Left in the backlog, it ends here: gdd brief {id} --stop \"<their words>\"")
    } else {
        String::new()
    };
    blocked(
        "stopped",
        "cap-spent",
        format!(
            "no new round: {caps} {have} no room left. Tell the person in one line and propose splitting it or leaving it in the backlog, never more room; ask once. More room is the person's to give: {ask}, and after their yes: {waive}{ends}"
        ),
        format!("gdd ask {id} --of decision \"<split it, or leave it in the backlog?>\""),
    )
}

fn waiting_or_ask(id: &str, p: &Progress, of: &str, what: &str, then: &str, now: i64) -> Next {
    if let Some(a) = p.open_ask().filter(|a| a.of == of) {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        return blocked(
            "wait",
            "needs-person",
            format!(
                "waiting for the person since line {} ({waited}): {}",
                a.line, a.what
            ),
            then.to_string(),
        );
    }
    blocked(
        "ask",
        "needs-person",
        format!("ask the person: {what}"),
        format!("gdd ask {id} --of {of} \"<one question a yes answers>\""),
    )
}

/// What records the person's answer to an ask of a kind: an approval for what a person approves,
/// a waiver for a waiver, and their words for the rest.
pub fn follow_up(id: &str, of: &str, version: i64) -> String {
    match of {
        "brief" | "change" | "blueprint" | "gates" | "debt" => format!(
            "gdd approve {id} --of {of} --version {version} --who <person> --where <conversation, commit or pull request> <files>"
        ),
        "waiver" => format!(
            "gdd waiver {id} --who <person> --check <check or limit> --why \"<reason>\" --backlog <item> [--limit <cap> --to <n>], or for a no: gdd answer {id} \"<their words>\""
        ),
        _ => format!("gdd answer {id} \"<their words>\""),
    }
}

/// How a change that passed everything ships: by a local merge, or by pull request.
fn ship_step(id: &str, git: &GitView) -> Next {
    let merge = format!("gdd ship {id} --merge");
    let request = format!(
        "gdd ship {id} --bookkeeping, then once the pull request is merged: gdd ship {id} --done <where>"
    );
    let (first, second) = if git.has_remote {
        (request, merge)
    } else {
        (merge, request)
    };
    n(
        "ship",
        format!(
            "describe it (gdd ship {id} --describe) and ship it as the constitution says: by a local merge, or by pull request"
        ),
        format!("{first}; or {second}"),
    )
}

pub(crate) fn next_step(
    id: &str,
    bp: &Blueprint,
    p: &Progress,
    lim: &Limits,
    git: &GitView,
    now: i64,
) -> Next {
    let kind = p.kind.unwrap_or(LogKind::Change);
    if kind == LogKind::Blueprint {
        return n(
            "done",
            "the blueprint's log has no next step of its own",
            "",
        );
    }
    if let Some((line, w)) = &p.shipped {
        return n(
            "done",
            format!("it shipped ({w}), line {line}: its log is closed"),
            "",
        );
    }
    // An open question waits first, whatever it is about.
    if let Some(a) = p.open_ask() {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        let then = follow_up(id, &a.of, p.version.unwrap_or(1));
        return blocked(
            "wait",
            "needs-person",
            format!(
                "waiting for the person since line {} ({waited}), {}: {}",
                a.line, a.of, a.what
            ),
            then,
        );
    }
    let color = p.color.clone().unwrap_or_default();
    let asks = |c: &str| !color.is_empty() && bp.color_asks(&color, c);
    let head_short = git
        .head
        .as_deref()
        .map(crate::git::short)
        .unwrap_or("its head");

    if kind == LogKind::Setup {
        if !p.checks_done {
            return setup_checks(
                id,
                p,
                lim,
                "launch gdd-checks to set the checks up and seal them",
            );
        }
        return match &p.last_review {
            None => n(
                "review",
                "launch gdd-review to try each check",
                format!("gdd launch {id} review"),
            ),
            Some(r) if r.verdict == "pass" => {
                if asks("person-review") && !p.person_reviewed() {
                    return waiting_or_ask(
                        id,
                        p,
                        "change",
                        "the person reviews the checks before they ship",
                        "",
                        now,
                    );
                }
                ship_step(id, git)
            }
            // A stop is never waived: once the person was told, a fix round, then a new trial,
            // and only a trial that passes lifts it.
            Some(r) if r.verdict == "stop" && !p.stop_told => blocked(
                "stopped",
                "review-stop",
                "the trial said stop: the checks don't ship. Tell the person why; a stop is never waived, it gets a fix round and a new trial",
                format!("gdd ask {id} --of stop \"<why it stopped>\""),
            ),
            Some(r) if r.verdict == "stop" => setup_checks(
                id,
                p,
                lim,
                "a fix round after the trial's stop: launch gdd-checks, which gets the trial's findings and the person's answer, then a new trial",
            ),
            Some(_) => setup_checks(
                id,
                p,
                lim,
                "the trial sent the checks back: launch gdd-checks again",
            ),
        };
    }

    if p.brief_line.is_none() {
        return n(
            "brief",
            "load gdd-brief: its color goes to the log before anyone builds",
            format!(
                "gdd brief {id} --paths <path>... [--color <color> --why \"<reason>\"] [--request \"<their words>\" or --request-file <a file with them>, when there is no brief file] [<brief and scenario files>]"
            ),
        );
    }
    if let Some(reason) = &p.brief_stopped {
        return blocked(
            "stopped",
            "cannot",
            format!(
                "the brief stopped: {reason}. It stays dropped until a new brief takes it up again"
            ),
            format!("gdd brief {id} ..."),
        );
    }
    if color == "red"
        && p.has_brief()
        && !p.read_since_red
        && !p.approved()
        && p.last_build.is_none()
        && p.open_ask().is_none()
    {
        return n(
            "reading",
            "a red brief is read before the person approves it: launch gdd-review to read it, then take into a new version what you agree with",
            format!("gdd launch {id} reading"),
        );
    }
    if asks("brief-approval") && !p.approved() {
        let read = if color == "yellow" {
            format!(
                " (if it changes data another zone owns, have it read first: gdd launch {id} reading)"
            )
        } else {
            String::new()
        };
        return waiting_or_ask(
            id,
            p,
            "brief",
            &format!(
                "approve what has to pass, which {color} asks for, before anyone builds{read}"
            ),
            &format!(
                "gdd approve {id} --of brief --version {} --who <person> --where conversation <files>",
                p.version.unwrap_or(1)
            ),
            now,
        );
    }
    if p.has_brief() && !p.checks_done {
        // The person answered after the second stop: what they decided moves it.
        let decided = p.checks_stopped >= 2
            && p.checks_line
                .is_some_and(|c| p.asks.iter().any(|a| a.line > c && a.answered));
        if decided {
            return blocked(
                "stopped",
                "cannot",
                format!(
                    "gdd-checks stopped twice and the person answered: a new version of the brief, sealed (gdd seal) and logged (gdd brief), takes the checks up again; if they said to try once more as it is, gdd launch {id} checks --level deep; if they dropped it, gdd brief {id} --stop \"<their words>\""
                ),
                format!("gdd brief {id} ..."),
            );
        }
        if p.checks_stopped >= 2 {
            return blocked(
                "stopped",
                "cannot",
                "gdd-checks stopped twice: a check it can't write goes to the person",
                format!(
                    "gdd ask {id} --of decision \"<the check it can't write, and the new version of the brief you propose>\""
                ),
            );
        }
        let deep = if p.checks_stopped == 1 {
            " --level deep"
        } else {
            ""
        };
        return round_needs_room(
            id,
            lim,
            n(
                "checks",
                "launch gdd-checks: a test for each scenario, seen failing and sealed",
                format!("gdd launch {id} checks{deep}"),
            ),
        );
    }
    let Some((_, build_stopped)) = p.last_build.filter(|_| p.built_since_seal) else {
        return round_needs_room(
            id,
            lim,
            n(
                "build",
                "launch gdd-build on its own branch, from <start>",
                format!("gdd launch {id} build"),
            ),
        );
    };
    if build_stopped {
        return blocked(
            "stopped",
            "cannot",
            format!(
                "the builder stopped with CANNOT: {}. A check that looks wrong goes to gdd-checks or the person; anything else to a new version of the brief",
                p.build_stop.clone().unwrap_or_default()
            ),
            format!("gdd brief {id} ..."),
        );
    }
    // What sent the change back stands whatever the branch holds now. A build step answers it;
    // commits nobody logged don't, and before this the gates were named for them and the stop was
    // forgotten.
    if let Some(r) = p.last_review.as_ref().filter(|r| r.verdict != "pass") {
        return review_sent_back(id, p, lim, &r.verdict);
    }
    if p.last_gates.as_ref().is_some_and(GatesRun::seal_failed) {
        return seal_broken(id, p, lim);
    }
    let gates_on_head = p
        .last_gates
        .as_ref()
        .filter(|g| match (&g.head, &git.head) {
            (Some(a), Some(b)) => git.stands_for(a, b),
            _ => true,
        });
    let Some(g) = gates_on_head else {
        // The branch moved past gates that failed and no step says so: that round is reported
        // before its gates run, or it would never count as one.
        if let Some(failed) = p.last_gates.as_ref().filter(|g| !g.passed)
            && failed.head.is_some()
            && git.head.is_some()
            && !p.seal_since_gates
        {
            return round_needs_room(
                id,
                lim,
                n(
                    "build",
                    format!(
                        "the branch moved past the gates that failed ({}) and no build step says so: report that round, then its gates run",
                        failed.failed.join(", ")
                    ),
                    format!("gdd report {id} build <the builder's last message, a file or ->"),
                ),
            );
        }
        if !lim.stop.is_empty() && lim.judge != "gates" {
            return cap_spent(id, lim);
        }
        return n(
            "gates",
            format!("run the gates on {head_short}, out of the builder's reach"),
            format!("gdd gates {id}"),
        );
    };
    if !g.passed {
        return round_needs_room(
            id,
            lim,
            n(
                "build",
                format!("a fix round: {} failed", g.failed.join(", ")),
                format!("gdd launch {id} build"),
            ),
        );
    }
    let review_on_head = p.last_review.as_ref().filter(|r| match (&r.head, &g.head) {
        (Some(a), Some(b)) => git.stands_for(a, b),
        _ => r.line > g.line,
    });
    let Some(r) = review_on_head else {
        return n(
            "review",
            "launch gdd-review with a clean context",
            format!("gdd launch {id} review"),
        );
    };
    match r.verdict.as_str() {
        v @ ("stop" | "fix") => review_sent_back(id, p, lim, v),
        _ => {
            if let Some(rc) = &r.color
                && crate::color_rank(rc) > crate::color_rank(&color)
            {
                return n(
                    "brief",
                    format!(
                        "the reviewer found {rc}, higher than {color}: back to Brief, and gdd-checks adds its cases"
                    ),
                    format!("gdd brief {id} --color {rc} --why \"<what raised it>\" <files>"),
                );
            }
            if asks("person-review") && !p.person_reviewed() {
                return waiting_or_ask(
                    id,
                    p,
                    "change",
                    &format!("the person reviews the change, which {color} asks for"),
                    &format!(
                        "gdd approve {id} --of change --version 1 --who <person> --where <pull request or conversation>"
                    ),
                    now,
                );
            }
            ship_step(id, git)
        }
    }
}
