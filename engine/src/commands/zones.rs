//! The color of some paths, and the zones of the blueprint tried on the repository.

use super::*;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

/// A draft's zone feature may not be in Git yet.
fn draft_zone_checks(project: &Project, zone: &str, file: &str, out: &mut Out) -> R<Vec<String>> {
    let root = project
        .root
        .canonicalize()
        .map_err(|e| crate::out::io(&project.root, e))?;
    let path = project.path(&color::clean_path(file).map_err(Fail::usage)?);
    match path.canonicalize() {
        Ok(p) if p.starts_with(&root) && p.is_file() => {
            let feature = std::fs::read_to_string(&p).map_err(|e| crate::out::io(&p, e))?;
            Ok(gates::zone_gate_checks(&feature))
        }
        _ => {
            out.say(format!("  zone {zone}: {file} is unavailable in the project, so its own checks could not be previewed"));
            Ok(Vec::new())
        }
    }
}

/// Compare actual files, so Git's exclusions and a change touching two zones keep their meaning.
/// With a sample, also preview the gate plan, including the checks named in draft zone features.
fn show_coverage(
    out: &mut Out,
    project: &Project,
    bp: &Blueprint,
    sample: Option<&color::Colored>,
    matching: impl Fn(&[String]) -> Result<Vec<String>, String>,
) -> R<()> {
    let mut zones = Vec::new();
    let mut zone_checks = Vec::new();
    for zone in bp.zones() {
        if zone.paths.is_empty() || sample.is_some_and(|s| !s.zones.contains_key(&zone.id)) {
            continue;
        }
        let files = matching(&zone.paths).map_err(Fail::usage)?;
        if files.is_empty() {
            continue;
        }
        if sample.is_some()
            && let Some(file) = &zone.gates
        {
            zone_checks.extend(draft_zone_checks(project, &zone.id, file, out)?);
        }
        zones.push((bp.color_checks(&zone.color), zone, files));
    }

    let mut change_checks = zone_checks.clone();
    if let Some(s) = sample {
        change_checks.extend(bp.color_checks(&s.color));
    }
    let scope = if sample.is_some() {
        "sampled"
    } else {
        "tracked"
    };
    let mut filters = BTreeMap::new();
    let mut gaps = Vec::new();
    for check in bp
        .checks()
        .into_iter()
        .filter(|c| c.kind == "machine" && !c.when.is_empty())
    {
        let asking: Vec<_> = zones
            .iter()
            .filter(|(ids, _, _)| ids.contains(&check.id))
            .collect();
        if asking.is_empty() && !change_checks.contains(&check.id) {
            continue;
        }
        let watched = match filters.entry(check.when.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let files = matching(&check.when).map_err(Fail::usage)?;
                entry.insert(files.into_iter().collect::<BTreeSet<_>>())
            }
        };
        for (_, zone, files) in asking {
            if files.iter().all(|p| !watched.contains(p)) {
                out.say(format!(
                    "  zone {} ({}): {}'s when matches no {scope} file in this zone",
                    zone.id, zone.color, check.id
                ));
                gaps.push(json!({"zone": zone.id, "color": zone.color, "check": check.id, "when": check.when}));
            }
        }
    }
    out.fact("gaps", json!(gaps));
    if let Some(s) = sample {
        let plan = gates::plan(
            bp,
            &s.color,
            &|when| !filters[when].is_empty(),
            &zone_checks,
        );
        show_plan(out, &plan);
        let other: Vec<_> = bp
            .checks()
            .into_iter()
            .filter(|c| {
                matches!(c.kind.as_str(), "person" | "reviewer") && change_checks.contains(&c.id)
            })
            .map(|c| c.id)
            .collect();
        if !other.is_empty() {
            out.say(format!("  reviewer or person: {}", other.join(", ")));
        }
        out.fact("other_checks", json!(other));
    }
    Ok(())
}

fn show_plan(out: &mut Out, plan: &[gates::Planned]) {
    let skipped = gating::show_skipped(plan, &[], out);
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
            show_coverage(out, &project, &bp, Some(&c), |specs| {
                git::match_paths(&ctx.root, &files, specs)
            })?;
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
        show_coverage(out, &project, &bp, None, |specs| {
            git::ls_files(&ctx.root, specs)
        })?;
        Ok(())
    })
}
