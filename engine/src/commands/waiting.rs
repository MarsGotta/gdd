//! What waits for the person, across every log.

use super::*;

/// Each log whose last event is a question with no answer yet, with that question.
pub(super) fn open_questions(project: &Project) -> Vec<(String, state::Ask)> {
    let mut open = vec![];
    for id in project.log_ids() {
        let p = state::Progress::of(&id, &project.read_log(&id));
        if let Some(a) = p.open_ask() {
            open.push((id, a.clone()));
        }
    }
    open
}

pub fn waiting(ctx: &Ctx) -> Out {
    run("waiting", |out| {
        let project = Project::open(&ctx.root);
        let now = time::now();
        let mut open: Vec<_> = open_questions(&project)
            .into_iter()
            .map(|(id, a)| (a.ts.unwrap_or(now), id, a.line, a.of, a.what))
            .collect();
        open.sort();
        for (t, id, line, of, what) in &open {
            out.say(format!(
                "{} {id}:{line} {of}: {what}",
                time::waited(now - t)
            ));
        }
        if open.is_empty() {
            out.say("nothing waits for the person");
        }
        out.fact(
            "waiting",
            json!(open.iter().map(|(t, id, line, of, what)| json!({"id": id, "line": line, "of": of, "what": what, "since": time::format(*t), "seconds": now - t})).collect::<Vec<_>>()),
        );
        Ok(())
    })
}
