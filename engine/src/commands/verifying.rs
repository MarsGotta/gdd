//! The integrity of the logs, for CI.

use super::*;

pub fn verify_cmd(ctx: &Ctx, ids: &[String], all: bool, no_git: bool) -> Out {
    run("verify", |out| {
        let project = Project::open(&ctx.root);
        let bp = if project.has_blueprint() {
            Some(project.blueprint()?)
        } else {
            None
        };
        if bp.is_none() {
            out.say("there is no gdd/blueprint.md: the order of colors can't be checked");
        }
        let ids: Vec<String> = if all || ids.is_empty() {
            project.log_ids()
        } else {
            ids.to_vec()
        };
        for id in &ids {
            check_id(id)?;
            if !project.has_log(id) {
                return Err(Fail::missing(
                    format!("gdd/log/{id}.jsonl"),
                    format!("gdd/log/{id}.jsonl is missing"),
                ));
            }
        }
        let use_git = !no_git && git::is_repo(&ctx.root);
        if !no_git && !use_git {
            out.say("not a git repository: checking the logs alone, as with --no-git");
        }
        let main = git::main_branch(&ctx.root);
        let mut found = vec![];
        for id in &ids {
            let entries = project.read_log(id);
            let journal = crate::journal::read(&project.journal_path(id));
            let layer = crate::journal::lines_written(&journal, &entries);
            found.extend(verify::check_log(id, bp.as_ref(), &entries, &layer));
            if use_git {
                found.extend(verify::check_git(&ctx.root, id, &entries, &main));
            }
        }
        // The same event written in two logs.
        if ids.len() > 1 {
            let mut seen: std::collections::BTreeMap<String, (String, usize)> = Default::default();
            for id in &ids {
                for e in project.read_log(id) {
                    let Some(v) = e.event() else { continue };
                    let key = format!(
                        "{}|{}|{}|{}|{}|{}",
                        str_of(v, "ts"),
                        log::kind(v),
                        step(v),
                        str_of(v, "of"),
                        v["version"],
                        v["files"]
                    );
                    if let Some((other, line)) = seen.get(&key) {
                        if other != id {
                            found.push(verify::Finding {
                                log: id.clone(),
                                line: e.line,
                                level: "warning",
                                code: "written-twice",
                                what: format!("the same event as gdd/log/{other}.jsonl:{line}, written in two logs"),
                            });
                        }
                    } else {
                        seen.insert(key, (id.clone(), e.line));
                    }
                }
            }
        }
        let errors = found.iter().filter(|f| f.level == "error").count();
        let warnings = found.len() - errors;
        for f in &found {
            out.say(format!(
                "{} gdd/log/{}.jsonl:{}: {} ({})",
                f.level, f.log, f.line, f.what, f.code
            ));
        }
        out.say(format!(
            "{} logs, {errors} errors, {warnings} warnings{}",
            ids.len(),
            if use_git { "" } else { ", without git" }
        ));
        out.fact("logs", ids.len());
        out.fact("errors", errors);
        out.fact("warnings", warnings);
        out.fact("git", use_git);
        out.fact(
            "findings",
            json!(found.iter().map(|f| f.to_json()).collect::<Vec<_>>()),
        );
        if errors > 0 {
            out.code = 1;
            out.refused = Some("verify-failed".into());
        }
        Ok(())
    })
}
