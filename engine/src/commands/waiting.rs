//! What waits for the person, across every log.

use super::*;

pub fn waiting(ctx: &Ctx) -> Out {
    run("waiting", |out| {
        let project = Project::open(&ctx.root);
        let now = time::now();
        let mut open = vec![];
        for id in project.log_ids() {
            let e = project.read_log(&id);
            let p = state::Progress::of(&id, &e);
            if let Some(a) = p.open_ask() {
                open.push((
                    a.ts.unwrap_or(now),
                    id.clone(),
                    a.line,
                    a.of.clone(),
                    a.what.clone(),
                ));
            }
        }
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
