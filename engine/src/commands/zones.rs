//! The color of some paths, and the zones of the blueprint tried on the repository.

use super::*;

pub fn color_cmd(ctx: &Ctx, id: Option<&str>, paths: &[String]) -> Out {
    run("color", |out| {
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        let (on, list, logged, start, head) = if !paths.is_empty() {
            let logged = match id {
                Some(i) => load(ctx, i)?.state.progress.color,
                None => None,
            };
            ("planned", paths.to_vec(), logged, None, None)
        } else {
            let id = id.ok_or(Fail::usage(
                "give an id, for what it touched, or --paths, for what it will",
            ))?;
            let l = load(ctx, id)?;
            let start = l.state.progress.start.clone().ok_or(Fail::refuse(
                "out-of-order",
                format!("{id} has no brief step: no <start>"),
                format!("gdd brief {id} ..."),
            ))?;
            let head = l.state.git.head.clone().ok_or(Fail::missing(
                format!("the branch of {id}"),
                format!("no branch for {id} (work/{id}, gdd-build/{id} or build/{id}): pass --branch or --head"),
            ))?;
            let touched = changes(ctx, &start, &head)?;
            (
                "touched",
                touched,
                l.state.progress.color.clone(),
                Some(start),
                Some(head),
            )
        };
        let c = color::color_of(&ctx.root, &bp, &list, None, logged.as_deref(), "")
            .map_err(Fail::usage)?;
        let rose = logged
            .as_deref()
            .is_some_and(|l| crate::color_rank(&c.floor) > crate::color_rank(l));
        out.say(format!(
            "{on}: {} (floor {}){}",
            c.color,
            c.floor,
            if rose {
                format!(
                    ", higher than the {} logged: back to Brief",
                    logged.clone().unwrap_or_default()
                )
            } else {
                String::new()
            }
        ));
        if let (Some(s), Some(h)) = (&start, &head) {
            out.say(format!(
                "what changed from {} to {}",
                git::short(s),
                git::short(h)
            ));
        }
        for (z, ps) in &c.zones {
            out.say(format!("  zone {z}: {}", ps.join(", ")));
        }
        if !c.uncovered.is_empty() {
            out.say(format!(
                "  no zone covers: {} (never green)",
                c.uncovered.join(", ")
            ));
        }
        out.fact("on", on);
        out.fact("paths", json!(c.paths));
        out.fact("zones", json!(c.zones.keys().collect::<Vec<_>>()));
        out.fact("uncovered", json!(c.uncovered));
        out.fact("floor", json!(c.floor));
        out.fact("color", json!(c.color));
        out.fact("logged", json!(logged));
        out.fact("rose", rose);
        Ok(())
    })
}

pub fn zones_cmd(ctx: &Ctx) -> Out {
    run("zones", |out| {
        let project = Project::open(&ctx.root);
        let bp = project.blueprint()?;
        need_repo(ctx, "zones are matched by git")?;
        let r = color::zones(&ctx.root, &bp)
            .map_err(|e| Fail::missing("the files the zones' pathspecs match", e))?;
        for (z, spec, n) in &r.counts {
            out.say(format!("zone {z}: {spec} matches {n} tracked files"));
        }
        for (z, spec) in &r.empty {
            out.say(format!("the zone {z}'s {spec} matches no tracked file: a path that doesn't exist, or a pattern written wrong"));
        }
        if !r.uncovered.is_empty() {
            out.say(format!(
                "{} tracked files no zone covers, never green:",
                r.uncovered.len()
            ));
            for f in r.uncovered.iter().take(30) {
                out.say(format!("  {f}"));
            }
            if r.uncovered.len() > 30 {
                out.say(format!("  and {} more", r.uncovered.len() - 30));
            }
        }
        out.fact(
            "empty",
            json!(
                r.empty
                    .iter()
                    .map(|(z, s)| json!({"zone": z, "pathspec": s}))
                    .collect::<Vec<_>>()
            ),
        );
        out.fact("uncovered", json!(r.uncovered));
        out.say(format!(
            "{} pathspecs match nothing and {} tracked files no zone covers: a zone that names a folder that doesn't exist yet covers nothing",
            r.empty.len(),
            r.uncovered.len()
        ));
        Ok(())
    })
}
