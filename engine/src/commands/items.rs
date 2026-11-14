//! The backlog's items.

use super::*;

pub fn backlog_list(ctx: &Ctx) -> Out {
    run("backlog", |out| {
        let (_, text) = read_backlog(ctx)?;
        let b = backlog::read(&text);
        let items = &b.items;
        for i in items {
            out.say(format!(
                "{:<8} {:<20} {:<7} {}",
                i.state, i.id, i.color, i.what
            ));
        }
        if let Some(n) = backlog::next(items) {
            out.say(format!("next: {}", n.id));
        }
        out.fact("items", json!(items.iter().map(|i| json!({"id": i.id, "color": i.color, "phase": i.phase, "brief": i.brief, "state": i.state})).collect::<Vec<_>>()));
        // What can't be read is said, never skipped: an item missed is a change nobody builds.
        for p in &b.problems {
            out.say(format!("can't read {}", p.say()));
        }
        if !b.problems.is_empty() {
            out.fact(
                "problems",
                json!(
                    b.problems
                        .iter()
                        .map(|p| json!({"line": p.line, "what": p.what}))
                        .collect::<Vec<_>>()
                ),
            );
            out.say("fix those lines in gdd/backlog.md as its table asks (Id, What, Color, Phase, Brief and State), then gdd backlog list again");
            out.code = 1;
            out.refused = Some("unreadable".into());
        }
        Ok(())
    })
}

pub fn backlog_add(ctx: &Ctx, id: &str, what: &str, color: &str, phase: &str) -> Out {
    run("backlog", |out| {
        check_id(id)?;
        if crate::color_rank(color).is_none() {
            return Err(Fail::usage("--color is green, yellow or red"));
        }
        let (path, text) = read_backlog(ctx)?;
        // The trial of a check that waits for what it watches comes after the first item that
        // brings it something to judge. The backlog's order is the order of the work: added before
        // any item of the product, the trial would be next, with nothing to try the check on.
        if id.starts_with("standing-checks-")
            && !backlog::items(&text)
                .iter()
                .any(|i| log_kind(&i.id) == LogKind::Change)
        {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} is the trial of a check that waits for what it watches, and the backlog holds no item of the product yet: it would come next, with nothing to try the check on"
                ),
                "add first the item that brings it something to judge, then this one: gdd backlog add <that item> --what \"<what the person gets>\" --color <color>",
            ));
        }
        let item = backlog::Item {
            id: id.into(),
            what: what.into(),
            color: color.into(),
            phase: phase.into(),
            state: "todo".into(),
            ..backlog::Item::default()
        };
        let t = backlog::add(&text, &item)
            .map_err(|e| Fail::refuse("exists", e, "pick another id, or gdd backlog list"))?;
        std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
        out.say(format!("added {id} to the backlog, {color}, todo"));
        Ok(())
    })
}

pub fn backlog_set(ctx: &Ctx, id: &str, state: &str, brief: Option<&str>) -> Out {
    run("backlog", |out| {
        let (path, text) = read_backlog(ctx)?;
        let mut t = backlog::set(&text, id, "state", state)
            .map_err(|e| Fail::refuse("no-item", e, "gdd backlog list"))?;
        if let Some(b) = brief {
            t = backlog::set(&t, id, "brief", b)
                .map_err(|e| Fail::refuse("no-item", e, "gdd backlog list"))?;
        }
        std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
        out.say(format!("{id} is {state} in the backlog"));
        Ok(())
    })
}
