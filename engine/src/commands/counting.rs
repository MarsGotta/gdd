//! What the limits still allow a change.

use super::*;

pub fn limits_cmd(ctx: &Ctx, id: &str) -> Out {
    run("limits", |out| {
        check_id(id)?;
        check_refs(ctx)?;
        check_head(ctx)?;
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        if !project.has_log(id) {
            return Err(Fail::missing(
                format!("gdd/log/{id}.jsonl"),
                format!("gdd/log/{id}.jsonl is missing"),
            ));
        }
        let entries = project.read_log(id);
        let lim = limits::count_entries(&bp, &entries, time::now());
        for t in lim.text() {
            out.say(t);
        }
        let skipped = entries
            .iter()
            .filter(|e| e.error.is_some() && !e.blank)
            .count();
        if skipped > 0 {
            out.say(format!(
                "{skipped} lines aren't events and weren't counted: gdd verify {id}"
            ));
        }
        if !log::voided(&entries).is_empty() {
            out.say("lines voided by a correction decide nothing, but what they used still counts: a voided round is a round");
        }
        if let Value::Object(m) = lim.facts() {
            for (k, v) in m {
                out.fact(&k, v);
            }
        }
        // The builder's lines, which the counter leaves to git: from <start> to the change's head,
        // without the bookkeeping. One more line, beside the counter's, and never in `stop`.
        let p = state::Progress::of(id, &entries);
        let gv = GitView::of(&ctx.root, id, ctx.branch.as_deref(), ctx.head.as_deref());
        if let (Some(start), Some(head)) = (&p.start, &gv.head)
            && let Some(used) = git::changed_lines(&ctx.root, start, head)
        {
            let (cap, raised) = match lim.raised.get("lines_per_change") {
                Some(r) => (Some(*r), true),
                None => (bp.cap(&lim.color, "lines_per_change"), false),
            };
            let past = cap.is_some_and(|c| used > c);
            out.say(match cap {
                Some(c) => format!(
                    "lines {used} of {c}{}{}",
                    if raised { ", raised by a waiver" } else { "" },
                    if past {
                        ", past it: a change this big is split"
                    } else {
                        ""
                    }
                ),
                None => format!("lines {used} (no cap)"),
            });
            out.fact("lines", json!({"used": used, "cap": cap, "raised": raised, "past": past, "start": start, "head": head}));
        }
        out.code = if lim.stop.is_empty() { 0 } else { 1 };
        if out.code == 1 {
            out.refused = Some("cap-spent".into());
        }
        Ok(())
    })
}
