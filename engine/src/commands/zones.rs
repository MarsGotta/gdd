//! The color of some paths, and the zones of the blueprint tried on the repository.

use super::*;
use std::collections::BTreeSet;

/// Git can exclude files from either a zone or a check's pathspecs.
fn show_gaps(
    out: &mut Out,
    bp: &Blueprint,
    scope: &str,
    matching: impl Fn(&[String]) -> Result<Vec<String>, String>,
) -> R<()> {
    let mut zones = Vec::new();
    for zone in bp.zones().into_iter().filter(|z| !z.paths.is_empty()) {
        let files = matching(&zone.paths).map_err(Fail::usage)?;
        if !files.is_empty() {
            zones.push((zone, files));
        }
    }
    let mut gaps = Vec::new();
    for check in bp.checks() {
        if check.kind != "machine" || check.when.is_empty() {
            continue;
        }
        let asking: Vec<_> = zones
            .iter()
            .filter(|(z, _)| bp.color_asks(&z.color, &check.id))
            .collect();
        if asking.is_empty() {
            continue;
        }
        let watched: BTreeSet<_> = matching(&check.when)
            .map_err(Fail::usage)?
            .into_iter()
            .collect();
        for (z, files) in asking {
            if !files.iter().any(|p| watched.contains(p)) {
                out.say(format!(
                    "  zone {} ({}): {}'s when matches no {scope} file in this zone",
                    z.id, z.color, check.id
                ));
                gaps.push(
                    json!({"zone": z.id, "color": z.color, "check": check.id, "when": check.when}),
                );
            }
        }
    }
    out.fact("gaps", json!(gaps));
    Ok(())
}

fn preview(
    out: &mut Out,
    root: &Path,
    bp: &Blueprint,
    sample: &color::Colored,
    files: &[String],
) -> R<()> {
    let root = root.canonicalize().map_err(|e| crate::out::io(root, e))?;
    let mut extra = Vec::new();
    for z in bp.zones() {
        let Some(file) = z.gates.filter(|_| sample.zones.contains_key(&z.id)) else {
            continue;
        };
        let path = root.join(color::clean_path(&file).map_err(Fail::usage)?);
        if let Ok(p) = path.canonicalize()
            && p.starts_with(&root)
            && p.is_file()
        {
            let text = std::fs::read_to_string(&p).map_err(|e| crate::out::io(&p, e))?;
            extra.extend(gates::zone_gate_checks(&text));
        } else {
            out.say(format!("  zone {}: {file} is unavailable in the project, so its own checks could not be previewed", z.id));
        }
    }
    let matching = |when: &[String]| git::match_paths(&root, files, when);
    show_gaps(out, bp, "sampled", matching)?;
    let mut other = Vec::new();
    let mut matched = Vec::new();
    for c in bp.checks() {
        if !bp.color_asks(&sample.color, &c.id) && !extra.contains(&c.id) {
            continue;
        }
        match c.kind.as_str() {
            "machine" if !matching(&c.when).map_err(Fail::usage)?.is_empty() => {
                matched.push(c.when)
            }
            "person" | "reviewer" => other.push(c.id),
            _ => {}
        }
    }
    let touched = |when: &[String]| matched.iter().any(|m| m == when);
    show_plan(out, &gates::plan(bp, &sample.color, &touched, &extra));
    if !other.is_empty() {
        out.say(format!("  reviewer or person: {}", other.join(", ")));
    }
    out.fact("other_checks", json!(other));
    Ok(())
}

fn show_plan(out: &mut Out, plan: &[gates::Planned]) {
    let skipped = gating::show_skipped(plan, &[], &[], out);
    let checks: Vec<_> = plan
        .iter()
        .filter(|p| p.skipped.is_none())
        .map(|p| p.check.as_str())
        .collect();
    if checks.is_empty() {
        out.say("  no machine checks apply");
    } else {
        out.say(format!(
            "  machine checks that apply: {}",
            checks.join(", ")
        ));
    }
    out.fact("checks", json!(checks));
    out.fact("skipped", json!(skipped));
}

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
        if !paths.is_empty() {
            let mut files: Vec<_> = color::planned_paths(&ctx.root, paths)
                .map_err(Fail::usage)?
                .into_iter()
                .flat_map(|(_, files)| files)
                .collect();
            files.sort();
            files.dedup();
            preview(out, &ctx.root, &bp, &c, &files)?;
            if super::brief::provisional_colors(ctx, &bp).contains(&c.color) {
                out.say(format!(
                    "{}: {}, before its brief",
                    super::brief::provisional_said(&c.color),
                    super::brief::provisional_way(&c.color)
                ));
                out.fact("provisional", true);
            }
            super::alongside::say(ctx, &bp, id, paths, out);
            if id.is_none_or(|i| log_kind(i) == LogKind::Change) {
                super::alongside::say_may_fix(ctx, id, out);
            }
            // Before the standing checks hold, a try with no change, as Prepare's, has nothing to
            // go ahead of; a change going alongside them does (decision 140).
            let trying = id.is_none() && super::brief::standing_pending(ctx).unwrap_or(false);
            if id.is_none_or(|i| log_kind(i) == LogKind::Change) && !trying {
                super::brief::setup_ahead(ctx, &bp, &c, &files, out);
            }
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
        show_gaps(out, &bp, "tracked", |specs| git::ls_files(&ctx.root, specs))?;
        Ok(())
    })
}
