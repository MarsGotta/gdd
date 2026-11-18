//! What the limits still allow a change.

use super::*;

/// The builder's diff is advice for the review and summary, independent of the round budget.
fn change_size(
    root: &Path,
    bp: &Blueprint,
    p: &state::Progress,
    git: &GitView,
) -> Option<(i64, Option<i64>)> {
    let used = git::changed_lines(root, p.start.as_deref()?, git.head.as_deref()?)?;
    let expected = bp.cap(p.color.as_deref().unwrap_or(""), "lines_per_change");
    Some((used, expected))
}

pub(super) fn size_warning(l: &Loaded) -> Option<String> {
    let (used, expected) = change_size(&l.project.root, &l.bp, &l.state.progress, &l.state.git)?;
    let expected = expected.filter(|n| used > *n)?;
    Some(format!(
        "This change covers {used} lines, above the expected {expected}."
    ))
}

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
        let lim = limits::count_entries(&bp, id, &entries, time::now());
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
        if let Some((used, cap)) = change_size(&ctx.root, &bp, &p, &gv) {
            let past = cap.is_some_and(|n| used > n);
            out.say(match cap {
                Some(n) if past => format!("lines {used} of {n}, larger than expected: tell the reviewer and mention it in the summary"),
                Some(n) => format!("lines {used} of {n}"),
                None => format!("lines {used} (no cap)"),
            });
            out.fact(
                "lines",
                json!({"used": used, "cap": cap, "past": past, "start": p.start, "head": gv.head}),
            );
        }
        out.code = if lim.stop.is_empty() { 0 } else { 1 };
        if out.code == 1 {
            out.refused = Some("cap-spent".into());
        }
        Ok(())
    })
}
