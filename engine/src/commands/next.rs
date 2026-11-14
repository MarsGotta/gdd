//! What comes next: for one change, and for the project.

use super::*;

pub fn next(ctx: &Ctx, id: Option<&str>) -> Out {
    run("next", |out| {
        let Some(id) = id else {
            return next_project(ctx, out);
        };
        let mut l = load(ctx, id)?;
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
        if !s.limits.stop.is_empty() {
            out.say(format!("no room left: {}", s.limits.stop.join(", ")));
        }
        say_next(out, &s.next);
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
    let entries = project.read_log("blueprint");
    let approvals: Vec<&Value> = log::events(&entries)
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| log::kind(v) == "approval" && str_of(v, "of") == "blueprint")
        .collect();
    let now_hash = git::hash_object(&ctx.root, "gdd/blueprint.md").ok();
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
    let text = std::fs::read_to_string(project.path("gdd/backlog.md")).unwrap_or_default();
    let read = backlog::read(&text);
    if project.path("gdd/backlog.md").is_file() {
        for p in &read.problems {
            out.say(format!(
                "can't read {}: fix it, then gdd backlog list",
                p.say()
            ));
        }
        if !read.problems.is_empty() {
            out.fact(
                "backlog_problems",
                json!(
                    read.problems
                        .iter()
                        .map(|p| json!({"line": p.line, "what": p.what}))
                        .collect::<Vec<_>>()
                ),
            );
        }
    }
    let items = read.items;
    let standing = items
        .iter()
        .find(|i| i.id == "standing-checks")
        .map(|i| i.state.clone());
    let set_up = standing.as_deref() == Some("done");
    out.say(match standing.as_deref() {
        Some("done") => "the standing checks are set up".to_string(),
        Some(s) => format!("the standing checks are {s}: they come before any change"),
        None => "the backlog has no standing-checks item".into(),
    });
    out.fact("standing_checks", json!(standing));
    let now = time::now();
    let mut open = vec![];
    let mut under_way = vec![];
    for lid in project.log_ids() {
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
        if log_kind(&lid) != LogKind::Blueprint && p.shipped.is_none() && p.events > 0 {
            let s = state::derive(
                &lid,
                &bp,
                &e,
                GitView::for_log(&ctx.root, &lid, None, None, &e),
                now,
            );
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
    match backlog::next(&items) {
        Some(i) if set_up || i.id == "standing-checks" => {
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
