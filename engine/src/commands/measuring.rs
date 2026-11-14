//! The metrics of the logs.

use super::*;

pub struct MetricsArgs<'a> {
    pub ids: &'a [String],
    pub format: &'a str,
    pub out: Option<&'a Path>,
    pub summary: bool,
}

pub fn metrics_cmd(ctx: &Ctx, a: &MetricsArgs) -> Out {
    run("metrics", |out| {
        let project = Project::open(&ctx.root);
        // Without a blueprint, the tables still come out, with no caps and no tiers.
        let bp = if project.has_blueprint() {
            project.blueprint()?
        } else {
            out.say("there is no gdd/blueprint.md: the tables have no caps and no tiers");
            Blueprint::parse("---\n---\n").map_err(|e| Fail::missing("an empty blueprint", e))?
        };
        let ids: Vec<String> = if a.ids.is_empty() {
            project.log_ids()
        } else {
            a.ids.to_vec()
        };
        let use_git = git::is_repo(&ctx.root);
        let mut stash = vec![];
        for id in &ids {
            check_id(id)?;
            let entries = project.read_log(id);
            let journal = crate::journal::read(&project.journal_path(id));
            let p = state::Progress::of(id, &entries);
            let mut cg = metrics::ChangeGit::default();
            if use_git {
                let head = p
                    .last_gates
                    .as_ref()
                    .and_then(|g| g.head.clone())
                    .or_else(|| GitView::of(&ctx.root, id, None, None).head);
                if let (Some(s), Some(h)) = (&p.start, head)
                    && let Some((f, ad, de)) = git::shortstat(&ctx.root, s, &h)
                {
                    cg = metrics::ChangeGit {
                        files: Some(f),
                        added: Some(ad),
                        deleted: Some(de),
                    };
                }
            }
            let scenarios = project
                .brief(id)
                .and_then(|(f, _)| f["scenarios"].as_array().map(|a| a.len() as i64))
                .or_else(|| {
                    let n: usize = p
                        .brief_files
                        .keys()
                        .filter(|k| k.starts_with("features/"))
                        .filter_map(|k| std::fs::read_to_string(project.path(k)).ok())
                        .map(|t| t.matches(&format!("@brief:{id}")).count())
                        .sum();
                    (n > 0).then_some(n as i64)
                });
            stash.push((id.clone(), entries, journal, cg, scenarios));
        }
        let inputs: Vec<metrics::Input> = stash
            .iter()
            .map(|(id, e, j, g, s)| metrics::Input {
                id,
                entries: e,
                journal: j,
                git: g.clone(),
                scenarios: *s,
            })
            .collect();
        let mut t = metrics::tables(&bp, &inputs, time::now());
        // Journals of ids with no log of their own, such as blueprint.engine.jsonl, or a call
        // refused before its change wrote anything: every call of the layer is in `engine`.
        if a.ids.is_empty() {
            for extra in project.journal_ids() {
                let extra = extra.as_str();
                if !ids.iter().any(|i| i == extra) {
                    let j = crate::journal::read(&project.journal_path(extra));
                    let more = metrics::tables(
                        &bp,
                        &[metrics::Input {
                            id: extra,
                            entries: &[],
                            journal: &j,
                            git: Default::default(),
                            scenarios: None,
                        }],
                        time::now(),
                    );
                    t.engine.extend(more.engine);
                }
            }
        }
        let counts: Map<String, Value> = metrics::TABLES
            .iter()
            .map(|n| (n.to_string(), json!(t.get(n).len())))
            .collect();
        match (a.format, a.out) {
            (f, Some(dir)) => {
                std::fs::create_dir_all(dir).map_err(|e| crate::out::io(dir, e))?;
                for name in metrics::TABLES {
                    let (file, text) = match f {
                        "csv" => (format!("{name}.csv"), metrics::csv(t.get(name))),
                        "jsonl" => (format!("{name}.jsonl"), metrics::jsonl(&t, name)),
                        _ => (format!("{name}.json"), serde_json::to_string_pretty(&json!({"schema": metrics::SCHEMA, "table": name, "rows": t.get(name)})).unwrap_or_default()),
                    };
                    std::fs::write(dir.join(&file), text)
                        .map_err(|e| crate::out::io(&dir.join(&file), e))?;
                }
                out.say(format!(
                    "wrote {} tables as {f} in {}",
                    metrics::TABLES.len(),
                    dir.display()
                ));
            }
            ("csv", None) => {
                if !a.summary {
                    for name in metrics::TABLES {
                        out.say(format!("# {name}"));
                        for l in metrics::csv(t.get(name)).lines() {
                            out.say(l.to_string());
                        }
                    }
                }
            }
            ("jsonl", None) => {
                if !a.summary {
                    for name in metrics::TABLES {
                        for l in metrics::jsonl(&t, name).lines() {
                            out.say(l.to_string());
                        }
                    }
                }
            }
            (_, None) => {
                if !a.summary {
                    out.say(
                        serde_json::to_string_pretty(&metrics::json_all(&t)).unwrap_or_default(),
                    );
                }
            }
        }
        if a.summary {
            for l in metrics::summary(&t) {
                out.say(l);
            }
        }
        out.fact("schema", json!(metrics::SCHEMA));
        out.fact("rows", Value::Object(counts));
        out.fact("git", use_git);
        Ok(())
    })
}
