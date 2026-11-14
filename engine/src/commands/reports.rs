//! A role's report, turned into its step of the log.

use super::*;

#[derive(Debug, Clone, Default)]
pub struct ReportArgs {
    pub model: Option<String>,
    pub level: Option<String>,
    pub seconds: Option<f64>,
    pub tokens: Option<i64>,
    pub proofs: Option<String>,
    pub claims: Option<String>,
    pub closed: Vec<(String, String)>,
    pub went: Vec<(String, String)>,
}

fn json_file(path: &Option<String>) -> R<Value> {
    match path {
        None => Ok(json!([])),
        Some(p) => {
            let t = std::fs::read_to_string(p)
                .map_err(|_| Fail::missing(p.to_string(), format!("{p} is missing")))?;
            let v: Value = serde_json::from_str(&t)
                .map_err(|e| Fail::usage(format!("{p} isn't JSON: {e}")))?;
            if !v.is_array() {
                return Err(Fail::usage(format!("{p} isn't a JSON list")));
            }
            Ok(v)
        }
    }
}

pub fn report(ctx: &Ctx, id: &str, st: &str, text: &str, a: &ReportArgs) -> Out {
    run("report", |out| {
        let l = load(ctx, id)?;
        let (line, _) = write_report(ctx, &l, id, st, text, a, out)?;
        out.fact("line", line);
        Ok(())
    })
}

pub(super) fn level_of(l: &Loaded, role: &str, asked: Option<&str>) -> String {
    if let Some(a) = asked {
        return a.to_string();
    }
    let color = l
        .state
        .progress
        .color
        .clone()
        .unwrap_or_else(|| "yellow".into());
    let base =
        l.bp.color_level(&color)
            .unwrap_or_else(|| "standard".into());
    if role == "checks" && l.state.progress.checks_stopped == 1 && !l.state.progress.checks_done {
        return "deep".into();
    }
    base
}

pub(super) fn write_report(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    st: &str,
    text: &str,
    a: &ReportArgs,
    out: &mut Out,
) -> R<(usize, RoleReport)> {
    let skill = match st {
        "checks" => "gdd-checks",
        "build" => "gdd-build",
        "review" | "reading" => "gdd-review",
        _ => return Err(Fail::usage("the step is checks, build, review or reading")),
    };
    let r = report::parse(text).map_err(|e| {
        Fail::refuse(
            "no-report",
            format!("the role's last message: {e}"),
            format!("launch it again, naming its skill: gdd launch {id} {st}"),
        )
    })?;
    if r.skill != skill {
        return Err(Fail::refuse(
            "wrong-skill",
            format!("its report names {}, not {skill}", r.skill),
            format!("gdd launch {id} {st}"),
        ));
    }
    // What the state allows comes before anything git says about the step.
    allowed(l, id, st, None)?;
    // A round `gdd launch` would not have started isn't written either: a role run without it, with
    // a cap already spent. What counts is how the caps stood at the log's last line, which is when
    // the round began, so a build that ended after its time ran out is still written and judged.
    if (st == "checks" || st == "build")
        && let Some(then) = l.state.progress.last_ts
    {
        let lim = limits::count_entries(&l.bp, &l.entries, then);
        let full = lim.no_room();
        if !full.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "no new round: {} had no room left when this one began",
                    full.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
    }
    let level = level_of(l, st, a.level.as_deref());
    let model = a
        .model
        .clone()
        .or_else(|| model_for(l, &ctx.env, st, &level).ok().map(|(_, m, _)| m));
    let p = &l.state.progress;
    let round = 1 + match st {
        "checks" => p.checks_runs,
        "build" => p.builds,
        "review" => p.reviews,
        _ => p.readings_n,
    };
    let stopped = stopped(&r);
    let result = if stopped {
        "stopped".to_string()
    } else if st == "review" || st == "reading" {
        r.verdict.clone().unwrap_or(r.result.clone())
    } else {
        r.result.clone()
    };
    let mut m = obj(vec![
        ("step", json!(st)),
        (
            "report",
            report_obj(
                skill,
                r.version.as_deref(),
                Some(&level),
                model.as_deref(),
                round,
                &result,
                &r.caught,
                a.seconds,
                a.tokens,
            ),
        ),
    ]);
    if let Some((reason, needs)) = &r.cannot {
        m.insert("stop".into(), json!({"reason": reason, "needs": needs}));
    }
    match st {
        "checks" if !stopped => report_checks(ctx, l, id, a, &r, out, &mut m)?,
        "build" => report_build(l, id, a, &r, &mut m)?,
        "review" => report_review(ctx, l, id, a, &r, out, &mut m)?,
        "reading" => report_reading(a, &r, &mut m),
        _ => {}
    }
    let ev = new_event(id, "step", &time::now_string(), m);
    let line = write(l, id, &ev)?;
    out.say(format!(
        "wrote the {st} step, line {line}: {result}, round {round}, {level}{}",
        model
            .as_deref()
            .map(|m| format!(" on {m}"))
            .unwrap_or_default()
    ));
    if let Some((reason, _)) = &r.cannot {
        out.say(format!("it stopped: CANNOT: {reason}"));
    }
    for f in &r.findings {
        out.say(format!("  {} {} {}", f.id, f.place, f.what));
    }
    if !r.later.is_empty() {
        out.say(format!("LATER, for the backlog: {}", r.later.join("; ")));
    }
    if !r.missing.is_empty() {
        out.say(format!("MISSING, for the brief: {}", r.missing.join("; ")));
    }
    let after = after(ctx, l, id);
    say_next(out, &after.next);
    out.fact("result", json!(result));
    out.fact("round", round);
    out.fact("next", json!(after.next.step));
    Ok((line, r))
}

/// Whether a role stopped: it said `CANNOT:`, or its report's result says so.
fn stopped(r: &RoleReport) -> bool {
    r.cannot.is_some() || r.result == "stopped"
}

/// What a checks step adds: the seal as it stands, the checks it sealed with their hashes, its
/// proofs and, in a setup log, the color of what it set up.
fn report_checks(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    a: &ReportArgs,
    r: &RoleReport,
    out: &mut Out,
    m: &mut Map<String, Value>,
) -> R<()> {
    let p = &l.state.progress;
    let tip = l.state.git.seal_tip.clone().ok_or(Fail::refuse(
        "not-sealed",
        format!("gdd-seal/{id} doesn't exist: the checks role seals its checks there"),
        format!(
            "launch the checks role again, which seals on gdd-seal/{id}: gdd launch {id} checks"
        ),
    ))?;
    // A setup log has no brief step: its seal was cut from the main branch, and that
    // commit is where it starts.
    let main = git::main_branch(&ctx.root);
    let cut = || git::merge_base(&ctx.root, &tip, &main);
    let start = match p.start.clone() {
        Some(s) => s,
        None => cut().ok_or(Fail::missing(
            format!("a commit both gdd-seal/{id} and {main} hold"),
            format!("gdd-seal/{id} shares no commit with {main}: it is cut from the main branch"),
        ))?,
    };
    // After a trial that sent a setup's checks back, what counts is what the seal holds
    // that the trial didn't judge. A merge of the main branch moves <start>: the fix can
    // be below it, sealed before the merge, or have come through main itself.
    let judged = p
        .last_review
        .as_ref()
        .and_then(|r| r.head.clone())
        .filter(|h| log_kind(id) == LogKind::Setup && git::is_ancestor(&ctx.root, h, &tip));
    let from = judged.unwrap_or_else(|| start.clone());
    // And what the seal held, unrecorded, when a `seal` event moved <start> past it: a
    // checks step voided by a correction and then a merge of the main branch, say.
    let pending: Vec<&(String, String)> = p
        .unrecorded
        .iter()
        .filter(|(a, b)| git::is_ancestor(&ctx.root, a, b) && git::is_ancestor(&ctx.root, b, &tip))
        .collect();
    if tip == from && pending.is_empty() {
        return Err(Fail::refuse(
            "not-sealed",
            format!("gdd-seal/{id} didn't move: the checks role sealed nothing"),
            format!("gdd launch {id} checks"),
        ));
    }
    if !start.is_empty() && !git::is_ancestor(&ctx.root, &start, &tip) {
        return Err(Fail::refuse(
            "not-sealed",
            format!(
                "gdd-seal/{id} no longer contains {}: a seal only moves forward",
                git::short(&start)
            ),
            format!(
                "what was sealed was taken off the seal: tell the person, gdd ask {id} --of stop \"<what the seal lost>\""
            ),
        ));
    }
    let mut changed = changes(ctx, &from, &tip)?;
    for (a, b) in pending {
        for f in changes(ctx, a, b)? {
            if !changed.contains(&f) {
                changed.push(f);
            }
        }
    }
    let mut checks = Map::new();
    for f in changed.iter().filter(|f| !p.brief_files.contains_key(*f)) {
        if let Some(h) = git::blob_at(&ctx.root, &tip, f) {
            checks.insert(f.clone(), json!(h));
        }
    }
    // Its proofs: --proofs overrides the json block of its last message.
    let proofs = if a.proofs.is_some() {
        json_file(&a.proofs)?
    } else if let Some(err) = &r.proofs_error {
        return Err(Fail::refuse(
            "bad-proofs",
            format!("the role's proofs can't be read: {err}"),
            format!("pass them as a file: gdd report {id} checks <file> --proofs <json file>"),
        ));
    } else {
        r.proofs.clone().unwrap_or_else(|| json!([]))
    };
    let n = proofs.as_array().map(Vec::len).unwrap_or(0);
    m.insert(
        "seal".into(),
        json!({"ref": format!("gdd-seal/{id}"), "commit": tip, "checks": checks, "hidden": null, "proofs": proofs}),
    );
    // A setup log's color is the one the paths it sealed give, from where it was cut.
    if log_kind(id) == LogKind::Setup {
        let base = p.base.clone().or_else(cut).unwrap_or_else(|| start.clone());
        let paths = changes(ctx, &base, &tip)?;
        let c = color::color_of(&ctx.root, &l.bp, &paths, None, p.color.as_deref(), "")
            .map_err(Fail::usage)?;
        out.say(format!("what it set up is {} (floor {})", c.color, c.floor));
        m.insert(
            "color".into(),
            json!({"on": "sealed", "base": base, "head": tip, "paths": paths, "zones": c.zones.keys().collect::<Vec<_>>(), "uncovered": c.uncovered, "floor": c.floor, "color": c.color, "why": c.why}),
        );
    }
    out.say(format!(
        "sealed at {}: {}; {n} proofs",
        git::short(&tip),
        checks.keys().cloned().collect::<Vec<_>>().join(", ")
    ));
    if n == 0 {
        out.say("its last message has no json block of proofs: they aren't in the log (or pass --proofs <file>)");
    }
    Ok(())
}

/// What a build step adds: what the builder says each check gave. A build that left nothing to
/// judge is refused.
fn report_build(
    l: &Loaded,
    id: &str,
    a: &ReportArgs,
    r: &RoleReport,
    m: &mut Map<String, Value>,
) -> R<()> {
    let p = &l.state.progress;
    let stopped = stopped(r);
    if !stopped && l.state.git.available && l.state.git.head.is_none() {
        return Err(Fail::missing(
            format!("the branch of {id}"),
            format!(
                "no branch for {id} (work/{id}, gdd-build/{id} or build/{id}, or the one --branch names): the build left nothing the gates can judge"
            ),
        ));
    }
    if !stopped
        && let (Some(h), Some(s)) = (&l.state.git.head, &p.start)
        && h == s
    {
        return Err(Fail::refuse(
            "nothing-built",
            "the build ended without CANNOT: and its branch has nothing past <start>: nothing was built",
            format!("give it the request again, word for word, once: gdd launch {id} build"),
        ));
    }
    // What it says each check gave: --claims, or the json block of its last message.
    let claims = if a.claims.is_some() {
        json_file(&a.claims)?
    } else {
        let c: Vec<Value> = r
            .list
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter(|x| x.get("command").is_some_and(|c| c.is_string()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        json!(c)
    };
    m.insert("claims".into(), claims);
    Ok(())
}

/// What a review step adds: its verdict, the color of what the change touched, what it ran and
/// what it found.
fn report_review(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    a: &ReportArgs,
    r: &RoleReport,
    out: &mut Out,
    m: &mut Map<String, Value>,
) -> R<()> {
    let p = &l.state.progress;
    let stopped = stopped(r);
    let Some(verdict) = r.verdict.clone().or(stopped.then(|| "stop".to_string())) else {
        return Err(Fail::refuse(
            "no-verdict",
            "its first word isn't pass, fix or stop: that isn't a pass",
            format!("launch it again, once: gdd launch {id} review"),
        ));
    };
    let head = l.state.git.head.clone();
    // What a setup log's trial judges is all it sealed, from where its seal was cut.
    let start = if log_kind(id) == LogKind::Setup {
        p.base.clone().or(p.start.clone()).unwrap_or_default()
    } else {
        p.start.clone().unwrap_or_default()
    };
    let raise = r.color.as_ref().map(|c| c.0.as_str());
    let colorobj = match (&head, start.is_empty()) {
        (Some(h), false) => {
            let touched = changes(ctx, &start, h)?;
            let c = color::color_of(
                &ctx.root,
                &l.bp,
                &touched,
                raise,
                None,
                r.color.as_ref().map(|c| c.1.as_str()).unwrap_or(""),
            )
            .map_err(Fail::usage)?;
            json!({"on": "touched", "start": start, "head": h, "zones": c.zones.keys().collect::<Vec<_>>(), "uncovered": c.uncovered, "floor": c.floor, "color": c.color, "why": c.why})
        }
        _ => {
            json!({"on": "touched", "start": start, "head": head, "zones": [], "uncovered": [], "floor": null, "color": raise, "why": "no git: the reviewer's word"})
        }
    };
    m.insert("color".into(), colorobj);
    m.insert("verdict".into(), json!(verdict));
    // What it ran, from the json block of its last message: its commands, as it gave them.
    let ran: Vec<Value> = r
        .list
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter(|x| x.get("command").is_some_and(|c| c.is_string()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if ran.is_empty() {
        out.say("its last message has no json block of what it ran: the log can't show it");
    }
    m.insert("ran".into(), json!(ran));
    m.insert(
        "findings".into(),
        json!(
            r.findings
                .iter()
                .map(|f| json!({"id": f.id, "where": f.place, "what": f.what}))
                .collect::<Vec<_>>()
        ),
    );
    m.insert(
        "closed".into(),
        json!(
            a.closed
                .iter()
                .map(|(i, o)| json!({"id": i, "outcome": o}))
                .collect::<Vec<_>>()
        ),
    );
    if !r.notes.is_empty() {
        m.insert("notes".into(), json!(r.notes));
    }
    if !r.later.is_empty() {
        m.insert("later".into(), json!(r.later));
    }
    Ok(())
}

/// What a reading of a brief adds: its findings, and where each one went.
fn report_reading(a: &ReportArgs, r: &RoleReport, m: &mut Map<String, Value>) {
    m.insert(
        "findings".into(),
        json!(
            r.findings
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let went = a
                        .went
                        .iter()
                        .find(|(k, _)| *k == f.id)
                        .map(|(_, w)| w.clone())
                        .unwrap_or_else(|| "log".into());
                    json!({"id": f.id, "question": i + 1, "what": f.what, "went": went})
                })
                .collect::<Vec<_>>()
        ),
    );
}
