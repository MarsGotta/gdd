//! The brief step: the color of a change, written before anyone builds.

use super::*;

pub struct BriefArgs<'a> {
    pub paths: &'a [String],
    pub color: Option<&'a str>,
    pub why: &'a str,
    pub files: &'a [String],
    pub level: Option<&'a str>,
    pub skill_version: Option<&'a str>,
    /// The person's request, word for word.
    pub request: Option<&'a str>,
    /// A file that holds it, for words a shell would change.
    pub request_file: Option<&'a str>,
    /// Why the change is dropped: the brief step then ends it, with its `result` `stopped`.
    pub stop: Option<&'a str>,
}

/// A dropped change: a `brief` step whose `result` is `stopped`, with why. It moves nothing: no
/// color, no seal, no `<start>`. A new brief takes the change up again.
fn brief_stopped(l: &Loaded, id: &str, why: &str, a: &BriefArgs, out: &mut Out) -> R<()> {
    if !a.paths.is_empty() || !a.files.is_empty() || a.color.is_some() {
        return Err(Fail::usage(
            "--stop drops the change: it takes no --paths, no --color and no files",
        ));
    }
    let why = why.trim();
    if why.is_empty() {
        return Err(Fail::usage("--stop says why the change is dropped"));
    }
    let version = l.state.progress.version.unwrap_or(1);
    let level = a.level.unwrap_or("standard");
    let ev = new_event(
        id,
        "step",
        &time::now_string(),
        obj(vec![
            ("step", json!("brief")),
            ("version", json!(version)),
            (
                "report",
                report_obj(
                    "gdd-brief",
                    a.skill_version.or(Some(env!("CARGO_PKG_VERSION"))),
                    Some(level),
                    None,
                    round_of(&l.state),
                    "stopped",
                    "none",
                    None,
                    None,
                ),
            ),
            ("files", json!({})),
            ("seal", Value::Null),
            (
                "stop",
                json!({"reason": why, "needs": "a new brief, to take it up again"}),
            ),
        ]),
    );
    let line = write(l, id, &ev)?;
    out.say(format!(
        "{id} is dropped: wrote the brief step, stopped, line {line}"
    ));
    out.fact("line", line);
    out.fact("next", "stopped");
    Ok(())
}

/// The `brief` step: the planned color from the paths, the base, the files with their hashes and
/// the seal's tip, which must hold those same files.
pub fn brief(ctx: &Ctx, id: &str, a: &BriefArgs) -> Out {
    run("brief", |out| {
        let l = load(ctx, id)?;
        if log_kind(id) == LogKind::Setup {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} sets up checks and has no brief: its seal is made by the checks role, on gdd-seal/{id}"
                ),
                format!("gdd next {id}"),
            ));
        }
        if let Some(why) = a.stop {
            return brief_stopped(&l, id, why, a, out);
        }
        if a.paths.is_empty() {
            return Err(Fail::usage(
                "--paths lists every path the change will touch",
            ));
        }
        git_writes(ctx)?;
        let main = git::main_branch(&ctx.root);
        let seal_tip = l.state.git.seal_tip.clone();
        let base = match (&l.state.progress.base, &seal_tip) {
            (Some(b), _) => b.clone(),
            (None, Some(t)) => git::merge_base(&ctx.root, t, &main).ok_or(Fail::missing(
                format!("a commit both gdd-seal/{id} and {main} hold"),
                format!("the seal and {main} share no commit"),
            ))?,
            (None, None) => git::commit_of(&ctx.root, &main).ok_or(Fail::missing(
                format!("refs/heads/{main}"),
                format!("there is no {main} branch"),
            ))?,
        };
        let logged = l.state.progress.color.clone();
        let c = color::color_of(&ctx.root, &l.bp, a.paths, a.color, logged.as_deref(), a.why)
            .map_err(Fail::usage)?;
        if let Some(asked) = a.color
            && crate::color_rank(asked) < crate::color_rank(&c.color)
        {
            out.say(format!(
                "the color is {}, not {asked}: judgment can raise the floor, never lower it",
                c.color
            ));
        }
        let mut files = Map::new();
        for f in a.files {
            let h = git::hash_object(&ctx.root, f).map_err(|_| {
                Fail::missing(f.to_string(), format!("{f} is not in the working copy"))
            })?;
            if let Some(tip) = &seal_tip {
                match git::blob_at(&ctx.root, tip, f) {
                    Some(b) if b == h => {}
                    Some(b) => {
                        return Err(Fail::refuse(
                            "seal-mismatch",
                            format!(
                                "{f} is {} in the working copy and {} on the seal",
                                git::short(&h),
                                git::short(&b)
                            ),
                            format!("gdd seal {id} {f}"),
                        ));
                    }
                    None => {
                        return Err(Fail::refuse(
                            "seal-mismatch",
                            format!("{f} is not on gdd-seal/{id}"),
                            format!("gdd seal {id} {}", a.files.join(" ")),
                        ));
                    }
                }
            }
            files.insert(f.clone(), json!(h));
        }
        if !files.is_empty() && seal_tip.is_none() {
            return Err(Fail::refuse(
                "not-sealed",
                "the brief and its scenarios are written but not sealed",
                format!("gdd seal {id} {}", a.files.join(" ")),
            ));
        }
        let version = if l.state.progress.brief_line.is_some() {
            let prev = l.state.progress.version.unwrap_or(1);
            if files.is_empty() { prev } else { prev + 1 }
        } else {
            1
        };
        let version = l
            .project
            .brief(id)
            .and_then(|(f, _)| f["version"].as_i64())
            .filter(|_| !files.is_empty())
            .unwrap_or(version);
        let level = a.level.map(str::to_string).unwrap_or_else(|| {
            if c.color == "red" {
                "deep".into()
            } else {
                "standard".into()
            }
        });
        let colorobj = json!({
            "on": "planned",
            "base": base,
            "paths": a.paths,
            "zones": c.zones.keys().collect::<Vec<_>>(),
            "uncovered": c.uncovered,
            "floor": c.floor,
            "color": c.color,
            "why": c.why,
        });
        let seal_field = if files.is_empty() {
            Value::Null
        } else {
            json!(seal_tip)
        };
        let mut ev = new_event(
            id,
            "step",
            &time::now_string(),
            obj(vec![
                ("step", json!("brief")),
                ("version", json!(version)),
                (
                    "report",
                    report_obj(
                        "gdd-brief",
                        // The skills and the layer are released together: its version stands for
                        // theirs when the orchestrator names none.
                        a.skill_version.or(Some(env!("CARGO_PKG_VERSION"))),
                        Some(&level),
                        None,
                        round_of(&l.state),
                        &c.color,
                        "none",
                        None,
                        None,
                    ),
                ),
                ("color", colorobj),
                ("files", Value::Object(files.clone())),
                ("seal", seal_field),
            ]),
        );
        let from_file = match a.request_file {
            Some(f) => Some(
                std::fs::read_to_string(f)
                    .map_err(|_| Fail::missing(f.to_string(), format!("{f} is missing")))?,
            ),
            None => None,
        };
        if let Some(r) = a
            .request
            .or(from_file.as_deref())
            .map(str::trim)
            .filter(|r| !r.is_empty())
        {
            ev["request"] = json!(r);
        }
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "{id} is {} (floor {}), brief version {version}, base {}",
            c.color,
            c.floor,
            git::short(&base)
        ));
        for (z, ps) in &c.zones {
            out.say(format!("  zone {z}: {}", ps.join(", ")));
        }
        if !c.uncovered.is_empty() {
            out.say(format!(
                "  no zone covers {}: never green; propose its zone in the backlog",
                c.uncovered.join(", ")
            ));
        }
        out.say(format!("wrote the brief step, line {line}"));
        let after = after(ctx, &l, id);
        say_next(out, &after.next);
        out.fact("color", json!(c.color));
        out.fact("line", line);
        out.fact("next", json!(after.next.step));
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn report_obj(
    skill: &str,
    version: Option<&str>,
    level: Option<&str>,
    model: Option<&str>,
    round: i64,
    result: &str,
    caught: &str,
    seconds: Option<f64>,
    tokens: Option<i64>,
) -> Value {
    json!({
        "skill": skill,
        "version": version,
        "level": level,
        "model": model,
        "round": round,
        "result": result,
        "caught": caught,
        "seconds": seconds,
        "tokens": tokens,
    })
}

/// Which brief this is for the change: one more than the brief steps so far.
fn round_of(s: &State) -> i64 {
    1 + s.progress.briefs
}
