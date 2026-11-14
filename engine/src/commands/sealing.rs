//! The seal: what judges a change, kept on its own branch.

use super::*;

pub fn seal_cmd(ctx: &Ctx, id: &str, files: &[String], merge_main: bool) -> Out {
    run("seal", |out| {
        let l = load(ctx, id)?;
        if log_kind(id) == LogKind::Setup && !merge_main {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "the seal of {id} is made by the checks role, on gdd-seal/{id}: {id} has no brief, and nothing else is sealed there"
                ),
                format!(
                    "gdd next {id}. What the person approves outside the role's work, such as a change to the blueprint, lands on the main branch as the constitution says, and reaches this seal with: gdd seal {id} --merge-main"
                ),
            ));
        }
        need_repo(ctx, "sealing needs git")?;
        git_writes(ctx)?;
        git_commits(ctx)?;
        let main = git::main_branch(&ctx.root);
        if merge_main {
            let old = l.state.git.seal_tip.clone();
            let tip = seal::merge_main(&ctx.root, id, &main)
                .map_err(|e| {
                    Fail::refuse(
                        "conflict",
                        e,
                        format!("settle it on gdd-seal/{id}: the orchestrator its brief and scenarios, the checks role its checks; then gdd seal {id} --merge-main again"),
                    )
                })?;
            if old.as_deref() == Some(tip.as_str()) {
                out.say(format!("gdd-seal/{id} already holds {main}: nothing moved"));
            } else {
                let line = log_seal_move(
                    &l,
                    id,
                    old.as_deref(),
                    &tip,
                    &format!("merged {main}"),
                    None,
                )?;
                out.say(format!("gdd-seal/{id} is at {} with {main} merged in, written to the log, line {line}: merge it into the change's branch, and the next gdd gates judges from it", git::short(&tip)));
                out.fact("line", line);
            }
            out.fact("commit", json!(tip));
            return Ok(());
        }
        let parent = match &l.state.git.seal_tip {
            Some(t) => t.clone(),
            None => match &l.state.progress.base {
                Some(b) => b.clone(),
                None => git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
                    format!("refs/heads/{main}"),
                    format!("there is no {main} branch"),
                ))?,
            },
        };
        let msg = format!("Seal {id}: what judges it, before anyone builds");
        let s = seal::seal(&ctx.root, id, &parent, files, &msg)
            .map_err(|e| Fail::refuse("seal", e, format!("gdd next {id}")))?;
        if s.unchanged {
            out.say(format!(
                "gdd-seal/{id} already holds these files as they are, at {}",
                git::short(&s.commit)
            ));
        } else {
            let sealed: Map<String, Value> =
                s.files.iter().map(|(f, h)| (f.clone(), json!(h))).collect();
            let line = log_seal_move(
                &l,
                id,
                Some(&s.parent),
                &s.commit,
                &format!("sealed {}", files.join(", ")),
                Some(sealed),
            )?;
            out.say(format!(
                "sealed on gdd-seal/{id}: commit {} on top of {}, written to the log, line {line}",
                s.commit,
                git::short(&s.parent)
            ));
            out.fact("line", line);
        }
        for (f, h) in &s.files {
            out.say(format!("  {f} {h}"));
        }
        out.say(format!(
            "the checkout and its index were not touched; next: gdd brief {id} --paths ... {}",
            files.join(" ")
        ));
        out.fact("ref", json!(format!("gdd-seal/{id}")));
        out.fact("commit", json!(s.commit));
        out.fact(
            "files",
            json!(
                s.files
                    .iter()
                    .map(|(f, h)| (f.clone(), json!(h)))
                    .collect::<Map<_, _>>()
            ),
        );
        Ok(())
    })
}

/// Write that the seal moved: its ref, the commit it was at (or was cut from), the new one and why.
fn log_seal_move(
    l: &Loaded,
    id: &str,
    from: Option<&str>,
    to: &str,
    why: &str,
    files: Option<Map<String, Value>>,
) -> R<usize> {
    let mut m = obj(vec![
        ("ref", json!(format!("gdd-seal/{id}"))),
        ("from", json!(from)),
        ("to", json!(to)),
        ("why", json!(why)),
    ]);
    if let Some(f) = files {
        m.insert("files".into(), Value::Object(f));
    }
    let ev = new_event(id, "seal", &time::now_string(), m);
    write(l, id, &ev)
}
