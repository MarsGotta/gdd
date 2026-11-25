//! What comes next: for one change, and for the project.

use super::*;

/// Existing stops and approvals keep their own remedies. Only replace an otherwise live step.
fn stop_repeating(state: &mut State, entries: &[Entry], repeated: Option<(Value, String)>) {
    if state.next.blocked.is_some() || state.next.step == "done" {
        return;
    }
    if let Some((attempt, reason)) = repeated
        && attempt == crate::journal::attempt(&state.next.step, &state.git, entries)
    {
        let question = "<what is blocked in the product>. Shall I <proposed fix for the cause in the last refusal> before trying again?";
        let remedy = if reason == "vague-stop" {
            "The checks role must name the blocked scenario by @id or the file it needs."
        } else {
            "Use the last refusal to explain what must change and propose one fix."
        };
        state.next = state::Next {
            step: "ask".into(),
            say: format!(
                "The {} step was refused twice ({reason}). {remedy} Wait for the person's answer.",
                state.next.step
            ),
            command: format!("gdd ask {} --of decision \"{question}\"", state.id),
            blocked: Some("needs-person".into()),
        };
    }
}

pub(super) fn load_next(ctx: &Ctx, id: &str) -> R<Loaded> {
    let mut l = load(ctx, id)?;
    if l.state.next.step == "brief"
        && let Some(wait) = brief::pending_standing_checks(ctx)?
    {
        l.state.next = wait;
        return Ok(l);
    }
    // A step that seals, builds, judges or ships needs a git that can write.
    if ["brief", "checks", "build", "gates", "ship"].contains(&l.state.next.step.as_str())
        && git_writes(ctx).is_err()
    {
        l.state.next = state::Next {
            step: "stopped".into(),
            say: format!(
                "{CANNOT_WRITE}: work where git can write (its .git folder, and its refs), then ask again"
            ),
            command: format!("gdd next {id}"),
            blocked: Some("cannot".into()),
        };
    } else if ["brief", "checks", "build", "ship"].contains(&l.state.next.step.as_str())
        && git_commits(ctx).is_err()
    {
        // The same for a step that ends in a commit: of the layer's, or of the role it launches.
        l.state.next = state::Next {
            step: "stopped".into(),
            say: format!("{NO_AUTHOR}: {SET_AUTHOR}"),
            command: format!("gdd next {id}"),
            blocked: Some("cannot".into()),
        };
    }
    stop_repeating(
        &mut l.state,
        &l.entries,
        crate::journal::repeated_refusal(&l.project.journal_path(id)),
    );
    // A report the layer sent back goes to the reviewer who wrote it, never to a fresh one.
    if l.state.next.step == "review" && review_sent_back(&ctx.root, id).is_some() {
        l.state.next.say =
            "the layer sent the reviewer's report back: resume that reviewer for its whole report again, never a fresh one".into();
    }
    prefer_run(ctx, &l.bp, &mut l.state);
    Ok(l)
}

pub fn next(ctx: &Ctx, id: Option<&str>) -> Out {
    run("next", |out| {
        let Some(id) = id else {
            return next_project(ctx, out);
        };
        let l = load_next(ctx, id)?;
        let s = &l.state;
        if l.entries.is_empty() {
            out.say(format!("{id} has no log yet"));
        }
        if let Some(c) = &s.progress.color {
            out.say(format!(
                "{id}: {c}{}, {} events",
                s.progress
                    .version
                    .map(|v| format!(", brief version {v}"))
                    .unwrap_or_default(),
                s.progress.events
            ));
        }
        if let Some(st) = &s.progress.start {
            out.say(format!(
                "base {}, start {}{}",
                s.progress.base.as_deref().map(git::short).unwrap_or("?"),
                git::short(st),
                s.git
                    .head
                    .as_deref()
                    .map(|h| format!(", head {}", git::short(h)))
                    .unwrap_or_default()
            ));
        }
        if !s.passed_on_head.is_empty() {
            out.say(format!(
                "passed on this head: {}",
                s.passed_on_head.join(", ")
            ));
        }
        // A fact for the orchestrator, never a stop: time spent once the gates passed.
        if s.progress.shipped.is_none()
            && let Some(m) =
                crate::limits::minutes_after_gates_passed(&log::events(&l.entries), time::now())
        {
            out.say(format!(
                "its gates first all passed {m} minutes ago, without the waits for a person"
            ));
            out.fact("minutes_after_gates_passed", m);
        }
        if !s.limits.stop.is_empty() {
            out.say(format!("no room left: {}", s.limits.stop.join(", ")));
        }
        brief::show_approval(&l, id, out);
        out.next = Some(s.next.clone());
        out.fact("state", s.facts());
        if s.next
            .blocked
            .as_deref()
            .is_some_and(|b| b != "needs-person")
        {
            out.code = 1;
            out.refused = s.next.blocked.clone();
        }
        Ok(())
    })
}

fn next_project(ctx: &Ctx, out: &mut Out) -> R<()> {
    let project = Project::open(&ctx.root);
    if !project.has_blueprint() {
        out.say("there is no gdd/blueprint.md: the next moment is Prepare, with gdd-prepare");
        out.fact("blueprint", "missing");
        return Ok(());
    }
    let bp = project.blueprint()?;
    for p in bp.problems() {
        out.say(format!("the blueprint: {p}"));
    }
    show_blueprint_approval(&project, &bp, out);
    let path = project.path("gdd/backlog.md");
    let read = backlog::read(&std::fs::read_to_string(&path).unwrap_or_default());
    if !read.problems.is_empty() && path.is_file() {
        let mut problems = Vec::new();
        for p in read.problems {
            out.say(format!(
                "can't read {}: fix it, then gdd backlog list",
                p.say()
            ));
            problems.push(json!({"line": p.line, "what": p.what}));
        }
        out.fact("backlog_problems", json!(problems));
    }
    let items = read.items;
    let standing = backlog::standing_checks(&items);
    out.say(match standing {
        Some("done") => "the standing checks are set up".to_string(),
        Some(s) => format!("the standing checks are {s}: they come before any change"),
        None => "the backlog has no standing-checks item".into(),
    });
    out.fact("standing_checks", json!(standing));
    show_work_under_way(ctx, &project, &bp, out);
    match backlog::next(&items) {
        Some(i) if standing == Some("done") || i.id == "standing-checks" => {
            out.say(format!(
                "next in the backlog: {} ({}, {}): {}",
                i.id, i.color, i.state, i.what
            ));
            out.fact("backlog_next", json!(i.id));
        }
        Some(i) => {
            out.say(format!(
                "next in the backlog, once the standing checks hold: {}",
                i.id
            ));
            out.fact("backlog_next", json!(i.id));
        }
        None => out.say("the backlog has nothing to do"),
    }
    Ok(())
}

fn show_blueprint_approval(project: &Project, bp: &Blueprint, out: &mut Out) {
    let entries = project.read_log("blueprint");
    if let Some(words) = events::levels_answer(&entries, bp) {
        out.say(format!(
            "the person's answer about working at the available level: {words}"
        ));
        out.fact("levels_answer", words);
    }
    // What the gates read is asked once, when preparing: a project prepared before that hears it
    // here, before its next change is briefed (decision 133).
    match events::context_answer(&entries) {
        Some(words) => {
            out.say(format!(
                "the person's answer about what the gates should read: {words}"
            ));
            out.fact("context_answer", words);
        }
        None => out.say(
            "nobody has asked the person what the gates should read, such as a design or a prototype to follow, documents or the MCP servers of their tools: ask them once, with gdd ask blueprint --of context, and if they give something, gdd-prepare adds it to the blueprint before the next brief",
        ),
    }
    let approvals: Vec<&Value> = log::events(&entries)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| log::kind(v) == "approval" && str_of(v, "of") == "blueprint")
        .collect();
    let now_hash = git::hash_object(&project.root, "gdd/blueprint.md").ok();
    let named = approvals
        .iter()
        .rev()
        .find(|a| a["files"]["gdd/blueprint.md"].is_string())
        .or(approvals.last());
    let approved = match named {
        None => "no approval of the blueprint in gdd/log/blueprint.jsonl".to_string(),
        Some(a) => match a["files"]["gdd/blueprint.md"].as_str() {
            Some(h) if Some(h) == now_hash.as_deref() => "the blueprint is approved as it is now".into(),
            Some(_) => "the blueprint changed after its last approval: it needs the person's approval again".into(),
            None => "the blueprint is approved; the approval names no hash of it".into(),
        },
    };
    out.say(approved.clone());
    out.fact("blueprint", approved);
}

fn show_work_under_way(ctx: &Ctx, project: &Project, bp: &Blueprint, out: &mut Out) {
    let now = time::now();
    let mut open = vec![];
    let mut under_way = vec![];

    // A setup or first brief can be refused twice before it writes its first event.
    let ids: std::collections::BTreeSet<_> = project
        .log_ids()
        .into_iter()
        .chain(project.journal_ids())
        .collect();
    for lid in ids {
        let e = project.read_log(&lid);
        let p = state::Progress::of(&lid, &e);
        if let Some(a) = p.open_ask() {
            open.push((
                a.ts.unwrap_or(now),
                lid.clone(),
                a.of.clone(),
                a.what.clone(),
            ));
        }
        if log_kind(&lid) != LogKind::Blueprint && p.shipped.is_none() {
            let repeated = crate::journal::repeated_refusal(&project.journal_path(&lid));
            if p.events == 0 && repeated.is_none() {
                continue;
            }
            let mut s = state::derive(
                &lid,
                bp,
                &e,
                GitView::for_log(&ctx.root, &lid, None, None, &e),
                None,
                None,
                now,
            );
            stop_repeating(&mut s, &e, repeated);
            if p.events == 0 && s.next.blocked.as_deref() != Some("needs-person") {
                continue;
            }
            prefer_run(ctx, bp, &mut s);
            under_way.push(json!({"id": lid, "next": s.next.step, "command": s.next.command, "blocked": s.next.blocked}));
            out.say(match &s.next.blocked {
                Some(b) => format!("under way: {lid}, stopped ({b}): {}", s.next.command),
                None => format!("under way: {lid}, next {}: {}", s.next.step, s.next.command),
            });
        }
    }
    open.sort();
    for (t, lid, of, what) in &open {
        out.say(format!(
            "waiting {} for the person: {lid}, {of}: {what}",
            time::waited(now - t)
        ));
    }
    out.fact(
        "waiting",
        json!(open.iter().map(|(t, lid, of, what)| json!({"id": lid, "of": of, "what": what, "seconds": now - t})).collect::<Vec<_>>()),
    );
    out.fact("under_way", json!(under_way));
    say_in_use(project, bp, now, out);
}

/// What is in use and waits for a person's verdict, oldest first. It stops nothing.
pub(super) fn say_in_use(project: &Project, bp: &Blueprint, now: i64, out: &mut Out) {
    let named = crate::closing::named(&project.read_log("blueprint"));
    let mut in_use = vec![];
    // Git is asked once, and only when something shipped.
    let mut reverts = None;
    for lid in project.log_ids() {
        let entries = project.read_log(&lid);
        let p = state::Progress::of(&lid, &entries);
        let undone = p.shipped.as_ref().and_then(|_| {
            reverts
                .get_or_insert_with(|| crate::git::Reverts::read(&project.root))
                .as_ref()
                .and_then(|r| crate::closing::undone(r, &entries))
        });
        if let Some(crate::closing::Standing::Waiting(why)) =
            crate::closing::standing(&p, bp, named.get(&lid).copied(), undone.as_ref(), now)
        {
            in_use.push((p.shipped_at.unwrap_or(now), lid, why, p));
        }
    }
    in_use.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    for (since, lid, why, p) in &in_use {
        let looked = crate::closing::last_look(p, now).map_or(String::new(), |l| format!(", {l}"));
        out.say(format!(
            "in use, waiting for a verdict: {lid}, {}{looked}; {why}",
            time::waited(now - since)
        ));
    }
    if !in_use.is_empty() {
        out.say(crate::closing::waits_note("<id>"));
    }
    out.fact(
        "in_use",
        json!(
            in_use
                .iter()
                .map(|(since, lid, why, _)| json!({"id": lid, "seconds": now - since, "why": why}))
                .collect::<Vec<_>>()
        ),
    );
}
