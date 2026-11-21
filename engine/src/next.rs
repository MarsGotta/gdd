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
            let order = if id == "standing-checks" {
                "The standing checks come before any change."
            } else {
                "If they leave it for later, it stays in the backlog and the next request doesn't wait for it."
            };
            return blocked(
                "stopped",
                "cannot",
                format!(
                    "gdd-checks stopped twice: what it can't set up goes to the person, with what you'd do. {order}"
                ),
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

/// A review that found a color above the brief's sends the change back to Brief, whatever its
/// verdict and whatever came after it: no fix round starts at the lower color. What the person
/// has to hear comes first, a stop or a seal that broke, each told where its own step says.
fn color_rose(id: &str, p: &Progress, color: &str) -> Option<Next> {
    let r = p.last_review.as_ref()?;
    let found = r.color.as_deref()?;
    let stop = r.verdict == "stop";
    let untold = (stop && !p.stop_told)
        || (p.last_gates.as_ref().is_some_and(GatesRun::seal_failed) && !p.seal_told);
    if untold || crate::color_rank(found) <= crate::color_rank(color) {
        return None;
    }
    let dropped = if stop {
        format!("; if the person dropped it: gdd brief {id} --stop \"<their words>\"")
    } else {
        String::new()
    };
    Some(n(
        "brief",
        format!(
            "the reviewer found {found}, higher than {color}: back to Brief, and gdd-checks adds its cases{dropped}"
        ),
        format!(
            "gdd brief {id} <files> --color {found} --why \"<what raised it>\" --paths <path>..."
        ),
    ))
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
        "a fix round after the reviewer's stop, for the agent that built it"
    } else {
        "a fix round with the reviewer's notes, for the agent that built it"
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

/// A single question covers the spent caps and exactly what another attempt would buy.
pub fn cap_spent(id: &str, lim: &Limits) -> Next {
    let mut spent = Vec::new();
    let mut grants = std::collections::BTreeMap::new();
    let backlog = if log_kind(id) == LogKind::Change {
        "<item>"
    } else {
        id
    };
    for line in lim
        .lines
        .iter()
        .filter(|l| lim.stop.iter().any(|s| s == l.name))
    {
        let used = line.used as i64;
        let Some(cap) = line.cap else { continue };
        spent.push(format!("{}: {used} of {cap} used", line.name));
        let n = grants.entry(line.key).or_insert(used);
        *n = (*n).max(used);
    }
    let mut waivers = Vec::new();
    for (key, used) in grants {
        let to = match key {
            "rounds_per_change" | "runs_before_review" => (used + 1).to_string(),
            "fix_rounds_per_gate" => used.to_string(),
            _ => "<agreed amount>".into(),
        };
        waivers.push(format!("gdd waiver {id} --who <person> --check {key} --why \"<their words>\" --backlog {backlog} --limit {key} --to {to}"));
    }
    let (advice, proposal) = if id == "standing-checks" {
        (
            "Every change waits for the standing checks. Ask for the extra attempt now.",
            "<what will wait>. Shall I make one more attempt?",
        )
    } else {
        (
            "If the next item needs this work, say what will wait and propose one more attempt now. Otherwise propose leaving it for later.",
            "<propose another try if the next item depends on this, otherwise defer>. Yes?",
        )
    };
    let delegated = if log_kind(id) == LogKind::Change {
        format!(
            "If they delegated and other work can go on, defer it: say what works and what is left and end the message there, with no question, no offer of another attempt and no word on who could allow one. Other work can go on when gdd next, run without an id, names a backlog item that doesn't need this one. To defer, keep or add the item in the backlog and use gdd brief {id} --stop \"<what is left>\". Otherwise use the waiver question below. "
        )
    } else {
        String::new()
    };
    blocked(
        "stopped",
        "cap-spent",
        format!(
            "No new round. {delegated}{advice} If they choose to wait, record their answer. If they agree to the extra attempt, record their reply directly: {}",
            waivers.join(", then ")
        ),
        format!(
            "gdd ask {id} --of waiver \"<what works and what is missing>. {}. One more attempt <how much more, and what it would buy>. {proposal}\"",
            spent.join(", ")
        ),
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

/// What the passing reviewer reported for the person to look at before they approve the change,
/// quoted as its words: it helps them and stops nothing (decision 110).
fn evidence_for_the_person(p: &Progress) -> String {
    let Some(r) = p.last_review.as_ref().filter(|r| !r.evidence.is_empty()) else {
        return String::new();
    };
    let items: Vec<String> = r
        .evidence
        .iter()
        .map(|e| serde_json::json!(e).to_string())
        .collect();
    format!(
        ". The reviewer reported this evidence for the person to look at, its words quoted: {}",
        items.join("; ")
    )
}

/// The person's reply records either the extra attempt offered or the choice to wait.
pub(crate) fn limit_reply(id: &str) -> String {
    let grant = format!(
        "gdd waiver {id} --who <person> --check <cap> --why \"<their words>\" --backlog <item> --limit <cap> --to <agreed amount>"
    );
    let defer = format!("gdd answer {id} \"<their words>\", then keep the item in the backlog");
    if id == "standing-checks" {
        format!("If they agree to the extra attempt: {grant}. If they choose to wait: {defer}")
    } else {
        format!(
            "If they agree to leave it pending: {defer}. If they agree to the extra attempt: {grant}"
        )
    }
}

/// What records the person's answer to an ask of a kind: an approval for what a person approves,
/// a waiver for a waiver, and their words for the rest.
pub fn follow_up(id: &str, of: &str, version: i64) -> String {
    match of {
        "brief" | "change" | "blueprint" | "gates" | "debt" => format!(
            "gdd approve {id} --of {of} --version {version} --who <person> --where <conversation, commit or pull request> <files>"
        ),
        "close" => crate::closing::verdict_command(id),
        "waiver" => format!(
            "gdd waiver {id} --who <person> --check <check or limit> --why \"<reason>\" --backlog <item> [--limit <cap> --to <n>], only if they chose what the question would loosen. To keep the limit or check: gdd answer {id} \"<their words>\""
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
    standing: Option<&crate::closing::Standing>,
    now: i64,
) -> Next {
    let kind = p.kind.unwrap_or(LogKind::Change);
    if p.shipped.is_some() {
        return crate::closing::next(id, p, standing, now);
    }
    // An open question waits first, whatever it is about.
    if let Some(a) = p.open_ask() {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        let reminder = match a.of.as_str() {
            "brief" => ". To ask again, load gdd-brief and send its approval message: each rule or scenario in a line, your assumptions, the feature file, one question".into(),
            "change" => evidence_for_the_person(p),
            _ => String::new(),
        };
        let then = if a.spent_limit {
            limit_reply(id)
        } else {
            follow_up(id, &a.of, p.version.unwrap_or(1))
        };
        return blocked(
            "wait",
            "needs-person",
            format!(
                "waiting for the person since line {} ({waited}), {}: {}{reminder}",
                a.line, a.of, a.what
            ),
            then,
        );
    }
    if kind == LogKind::Blueprint {
        return n(
            "done",
            "the blueprint's log has no next step of its own",
            "",
        );
    }
    let color = p.color.clone().unwrap_or_default();
    let asks = |c: &str| !color.is_empty() && bp.color_asks(&color, c);
    let next = if kind == LogKind::Setup {
        next_in_setup(id, p, lim, git, asks("person-review"), now)
    } else {
        let split = bp.splits_reading(&color);
        color_rose(id, p, &color)
            .or_else(|| before_the_build(id, p, lim, &color, split, asks("brief-approval"), now))
            .unwrap_or_else(|| {
                from_the_build_on(id, p, lim, git, &color, asks("person-review"), now)
            })
    };
    if next.blocked.as_deref() == Some("cap-spent") && p.limit_deferred {
        let say = format!(
            "The person left this for later. Keep it in the backlog. When they want to resume, ask about the extra attempt: {}",
            next.command
        );
        let command = if kind == LogKind::Change {
            format!("gdd brief {id} --stop \"<their words>\"")
        } else {
            next.command
        };
        return blocked("stopped", "cap-spent", say, command);
    }
    next
}

/// Setting up the standing checks: the checks, their trial, the person's review where the color
/// asks for one, and shipping them.
fn next_in_setup(
    id: &str,
    p: &Progress,
    lim: &Limits,
    git: &GitView,
    person_reviews: bool,
    now: i64,
) -> Next {
    if !p.checks_done {
        return setup_checks(
            id,
            p,
            lim,
            "launch gdd-checks to set the checks up and seal them",
        );
    }
    match &p.last_review {
        None => n(
            "review",
            "launch gdd-review to try each check",
            format!("gdd launch {id} review"),
        ),
        Some(r) if r.verdict == "pass" => {
            if person_reviews && !p.person_reviewed() {
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
    }
}

/// What comes before anyone builds a change: its brief, its reading on red, the person's approval
/// and its checks. None when all of it is done.
fn before_the_build(
    id: &str,
    p: &Progress,
    lim: &Limits,
    color: &str,
    split: bool,
    asks_approval: bool,
    now: i64,
) -> Option<Next> {
    if p.brief_line.is_none() {
        return Some(n(
            "brief",
            "load gdd-brief: its color goes to the log before anyone builds",
            format!(
                "gdd brief {id} [<brief and scenario files>] --paths <path the change will touch>... [--color <color> --why \"<reason>\"] [--request \"<their words>\" or --request-file <a file with them>, when there is no brief file]"
            ),
        ));
    }
    if let Some(reason) = &p.brief_stopped {
        return Some(blocked(
            "stopped",
            "cannot",
            format!(
                "the brief stopped: {reason}. It stays dropped until a new brief takes it up again"
            ),
            format!("gdd brief {id} ..."),
        ));
    }
    if color == "red"
        && p.has_brief()
        && !p.brief_read(split)
        && !p.approved()
        && !p.built_since_brief()
        && p.open_ask().is_none()
    {
        let say = match p.next_part().filter(|_| split) {
            Some(part) => format!(
                "a red brief is read before the person approves it, here in three parts with a reader each, and `{part}` is next: launch gdd-review to read it, and once the three are in take into a new version what you agree with, up to three additions in all"
            ),
            None => "a red brief is read before the person approves it: launch gdd-review to read it, then take into a new version what you agree with".into(),
        };
        return Some(n("reading", say, format!("gdd launch {id} reading")));
    }
    if p.gaps_open() {
        return Some(n(
            "brief",
            format!(
                "the reading left {}: answer each in a new version of the brief, sealed (gdd seal) and logged (gdd brief), before {}",
                gaps_said(&p.gaps),
                if asks_approval && !p.approved() {
                    "asking the person to approve it"
                } else {
                    "its checks"
                }
            ),
            format!("gdd brief {id} <files of the new version> --paths <path>..."),
        ));
    }
    if asks_approval && !p.approved() {
        return Some(approval_step(id, p, color, now));
    }
    if p.needs_checks_fix() || p.has_brief() && !p.checks_done {
        return Some(checks_step(id, p, lim));
    }
    None
}

/// The lines a reading left under `MISSING:`, counted and quoted.
pub(crate) fn gaps_said(gaps: &[String]) -> String {
    format!(
        "{} under MISSING: ({})",
        match gaps.len() {
            1 => "a line".to_string(),
            n => format!("{n} lines"),
        },
        gaps.join("; ")
    )
}

/// The person approves what has to pass, before anyone builds.
fn approval_step(id: &str, p: &Progress, color: &str, now: i64) -> Next {
    let read = if color == "yellow" {
        format!(
            " (if it changes data another zone owns, have it read first: gdd launch {id} reading)"
        )
    } else {
        String::new()
    };
    waiting_or_ask(
        id,
        p,
        "brief",
        &format!("approve what has to pass, which {color} asks for, before anyone builds{read}"),
        &format!(
            "gdd approve {id} --of brief --version {} --who <person> --where conversation <files>",
            p.version.unwrap_or(1)
        ),
        now,
    )
}

/// The checks of a change: launched, launched deeper after a stop, or with the person after two.
fn checks_step(id: &str, p: &Progress, lim: &Limits) -> Next {
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
    round_needs_room(
        id,
        lim,
        n(
            "checks",
            if p.needs_checks_fix() {
                "return the findings to the checks agent to correct its sealed tests"
            } else {
                "launch gdd-checks: a test for each scenario, seen failing and sealed"
            },
            format!("gdd launch {id} checks{deep}"),
        ),
    )
}

/// From the first build on: its gates, its review, what sent it back, and shipping it.
fn from_the_build_on(
    id: &str,
    p: &Progress,
    lim: &Limits,
    git: &GitView,
    color: &str,
    person_reviews: bool,
    now: i64,
) -> Next {
    let head_short = git
        .head
        .as_deref()
        .map(crate::git::short)
        .unwrap_or("its head");

    let Some((_, build_stopped)) = p.last_build.filter(|_| p.built_since_seal) else {
        return round_needs_room(
            id,
            lim,
            n(
                "build",
                if p.checks_fix.as_ref().is_some_and(|f| !f.tests_only) {
                    "resume the builder to merge the corrected seal and address the remaining findings"
                } else if p.checks_fix.is_some() {
                    "resume the builder to merge the corrected seal into the change branch"
                } else {
                    "launch gdd-build on its own branch, from <start>"
                },
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
                format!(
                    "a fix round for the agent that built it: {} failed",
                    g.failed.join(", ")
                ),
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
            if person_reviews && !p.person_reviewed() {
                return waiting_or_ask(
                    id,
                    p,
                    "change",
                    &format!(
                        "the person reviews the change, which {color} asks for{}",
                        evidence_for_the_person(p)
                    ),
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
