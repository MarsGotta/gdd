//! A role's report, turned into its step of the log.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct ReportArgs {
    pub model: Option<String>,
    pub level: Option<String>,
    pub seconds: Option<f64>,
    pub tokens: Option<i64>,
    pub tokens_read: Option<u64>,
    pub codex_thread: Option<Value>,
    pub proofs: Option<String>,
    pub claims: Option<String>,
    pub closed: Option<String>,
    pub launch_reviewed: Option<String>,
    pub went: Vec<(String, String)>,
}

fn json_file(path: &str) -> R<Vec<Value>> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| Fail::missing(path.to_string(), format!("{path} is missing")))?;
    let value =
        serde_json::from_str(&text).map_err(|e| Fail::usage(format!("{path} isn't JSON: {e}")))?;
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(Fail::usage(format!("{path} isn't a JSON list"))),
    }
}

pub fn report(ctx: &Ctx, id: &str, st: &str, text: &str, a: &ReportArgs) -> Out {
    run_change(ctx, id, "report", |out| {
        let l = load_attempt(ctx, id, st, out)?;
        let (line, _) = write_report(ctx, &l, id, st, text, a, out)?;
        out.fact("line", line);
        Ok(())
    })
}

pub(super) fn level_of(
    bp: &Blueprint,
    p: &state::Progress,
    role: &str,
    asked: Option<&str>,
) -> String {
    if let Some(a) = asked {
        return a.to_string();
    }
    if role == "checks" && p.checks_stopped == 1 && !p.checks_done {
        return "deep".into();
    }
    bp.color_level(p.color.as_deref().unwrap_or("yellow"))
        .unwrap_or_else(|| "standard".into())
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
    let r = report::parse_with_blueprint(text, Some(&l.bp)).map_err(|e| {
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
    let p = &l.state.progress;
    let seal_only = st == "build"
        && p.merging_checks_fix()
        && !stopped(&r)
        && l.state
            .git
            .head
            .as_deref()
            .and_then(|h| changed_tests(l, h))
            .is_some();
    // A round `gdd launch` would not have started isn't written either: a role run without it, with
    // a cap already spent. What counts is how the caps stood at the log's last line, which is when
    // the round began, so a build that ended after its time ran out is still written and judged.
    if (st == "checks" || st == "build")
        && let Some(then) = l.state.progress.last_ts
    {
        let lim = limits::count_entries(&l.bp, id, &l.entries, then);
        let full = lim.no_room_for(st == "checks" && p.correcting_tests() || seal_only);
        if !full.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "no new round: {} had no room left when this one began",
                    full.join(", ")
                ),
                cap_way_on(l),
            ));
        }
    }
    let level = level_of(&l.bp, &l.state.progress, st, a.level.as_deref());
    let model = a.model.clone().or_else(|| {
        model_for(&l.bp, &ctx.env, st, &level)
            .ok()
            .map(|(_, m, _)| m)
    });
    // A role on another model than its level's isn't the role the blueprint asks for: in one run,
    // two rounds asked at the deep level ran on the orchestrator's own model.
    if let Some(said) = &a.model
        && let Ok((env, wanted, _)) = model_for(&l.bp, &ctx.env, st, &level)
        && !said
            .to_ascii_lowercase()
            .starts_with(&wanted.to_ascii_lowercase())
    {
        return Err(Fail::refuse(
            "wrong-model",
            format!(
                "the {st} role ran on {said}, and the {level} level of {env} is {wanted}: it isn't the role the blueprint asks for"
            ),
            format!(
                "launch it again on {wanted}: gdd launch {id} {st}; if your environment can't, stop and ask the person, never use another"
            ),
        ));
    }
    let round = 1 + match st {
        "checks" => p.checks_runs,
        "build" => p.builds,
        "review" => p.reviews,
        _ => p.readings_n,
    };
    let stopped = stopped(&r);
    // A stop of the checks role names what stops it: in one run, five rounds stopped on "the
    // eleven scenarios" and wrote no test. Many scenarios are no reason to stop.
    if st == "checks" && stopped {
        let (reason, needs) = r.cannot.clone().unwrap_or_default();
        if !names_a_blocker(&ctx.root, &format!("{reason} {needs}")) {
            return Err(Fail::refuse(
                "vague-stop",
                format!(
                    "a stop of the checks role names what stops it, a scenario by its @id or a file it needs, and this one names neither: \"{reason}\". Many scenarios are no reason to stop"
                ),
                format!(
                    "launch it again, to write and seal them one after another: gdd launch {id} checks"
                ),
            ));
        }
    }
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
    if seal_only {
        m.insert("seal_only".into(), json!(true));
    }
    m["report"]["tokens_read"] = json!(a.tokens_read);
    if matches!(st, "build" | "checks")
        && let Some(thread) = &a.codex_thread
    {
        m["report"]["codex_thread"] = thread.clone();
    }
    if let Some((reason, needs)) = &r.cannot {
        m.insert("stop".into(), json!({"reason": reason, "needs": needs}));
    }
    match st {
        "checks" if !stopped => report_checks(ctx, l, id, a, &r, out, &mut m)?,
        "build" => report_build(l, id, a, &r, &mut m)?,
        "review" => report_review(ctx, l, id, a, &r, out, &mut m)?,
        "reading" => report_reading(ctx, l, a, &r, &mut m)?,
        _ => {}
    }
    if matches!(st, "review" | "reading") && !r.later.is_empty() {
        m.insert("later".into(), json!(r.later));
    }
    // Evidence is for the person who approves a change that passed.
    let evidence = st == "review" && result == "pass" && !r.evidence.is_empty();
    if evidence {
        m.insert("evidence".into(), json!(r.evidence));
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
        let destination = if st == "reading" { "log" } else { "backlog" };
        out.say(format!(
            "LATER, for the {destination}: {}",
            r.later.join("; ")
        ));
    }
    if !r.missing.is_empty() {
        out.say(format!("MISSING, for the brief: {}", r.missing.join("; ")));
    }
    if evidence {
        out.say(format!(
            "EVIDENCE, for the person who approves: {}",
            r.evidence.join("; ")
        ));
    }
    out.fact("result", json!(result));
    out.fact("round", round);
    Ok((line, r))
}

/// Whether a stop names something concrete: a scenario by its `@id` in `features/`, or a file
/// (a word with a path or an extension, such as `tools/fakepay.mjs`). A ref such as
/// `gdd-seal/<id>` or a hash names nothing.
fn names_a_blocker(root: &Path, text: &str) -> bool {
    let ids: Vec<String> = std::fs::read_dir(root.join("features"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .flat_map(|t| {
            t.split_whitespace()
                .filter_map(|w| w.strip_prefix("@id:").map(str::to_string))
                .collect::<Vec<_>>()
        })
        .collect();
    let file_like = |w: &str| {
        let name = w.rsplit('/').next().unwrap_or(w);
        name.rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty()
                && ext.len() <= 6
                && ext.starts_with(|c: char| c.is_ascii_alphabetic())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
    };
    text.split(|c: char| c.is_whitespace() || "`'\"()[]{},;<>".contains(c))
        .map(|w| w.trim_end_matches(['.', ':']).trim_start_matches("gdd:"))
        .any(|w| !w.is_empty() && (ids.iter().any(|i| i == w) || file_like(w)))
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
    let cut = || git::merge_base(&ctx.root, &tip, &git::branch_ref(&main));
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
    // A new version of the brief sends its checks back to the role, which may find that what it
    // sealed before still covers it: in one run the brief only gained a port, and the layer refused
    // the role's word that nothing had to change, round after round.
    let standing = if tip == from && pending.is_empty() {
        let s = (!p.checks_done).then(|| standing_checks(ctx, l, id, &tip));
        let Some(s) = s.flatten() else {
            return Err(Fail::refuse(
                "not-sealed",
                format!("gdd-seal/{id} didn't move: the checks role sealed nothing"),
                format!("gdd launch {id} checks"),
            ));
        };
        Some(s)
    } else {
        None
    };
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
    let reused = standing.is_some();
    let mut proofs = reported_proofs(id, a, r)?;
    if let Some((previous_checks, mut previous_proofs)) = standing {
        checks = previous_checks;
        for proof in proofs {
            if !previous_proofs.contains(&proof) {
                previous_proofs.push(proof);
            }
        }
        proofs = previous_proofs;
    }
    check_proofs(l, id, &proofs)?;
    if reused {
        out.say(
            "nothing new on the seal: the checks sealed before stand for this version of the brief",
        );
    }
    let n = proofs.len();
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
    Ok(())
}

fn reported_proofs(id: &str, a: &ReportArgs, r: &RoleReport) -> R<Vec<Value>> {
    // Its proofs: --proofs overrides the json block of its last message.
    if let Some(path) = &a.proofs {
        return json_file(path);
    }
    if let Some(err) = &r.proofs_error {
        return Err(Fail::refuse(
            "bad-proofs",
            format!("the role's proofs can't be read: {err}"),
            format!("pass them as a file: gdd report {id} checks <file> --proofs <json file>"),
        ));
    }
    Ok(r.proofs
        .as_ref()
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// A check takes proof when it is set up, or when the blueprint asks for cases on this change.
pub(super) fn required_proofs(l: &Loaded, id: &str) -> R<BTreeSet<String>> {
    let p = &l.state.progress;
    let color = p.color.as_deref().unwrap_or("");
    let checks = l.bp.checks();
    let machine = checks.iter().filter(|c| c.kind == "machine");
    let mut required = BTreeSet::new();
    if id == "standing-checks" {
        let root = &l.project.root;
        let code = super::launching::has_code(root);
        for c in machine.filter(|c| code || matches!(c.id.as_str(), "seal" | "secrets")) {
            if c.when.is_empty()
                || !git::ls_files(root, &c.when)
                    .map_err(Fail::usage)?
                    .is_empty()
            {
                required.insert(c.id.clone());
            }
        }
        return Ok(required);
    }
    if let Some(check) = id.strip_prefix("standing-checks-") {
        return Ok(BTreeSet::from([check.to_owned()]));
    }
    if !p.has_brief() {
        return Ok(required);
    }
    let paths: Vec<String> = color::planned_paths(&l.project.root, &p.paths)
        .map_err(Fail::usage)?
        .into_iter()
        .flat_map(|(_, files)| files)
        .collect();
    for c in machine.filter(|c| c.cases_each_change && l.bp.color_asks(color, &c.id)) {
        if !c.when.is_empty()
            && git::match_paths(&l.project.root, &paths, &c.when)
                .map_err(Fail::usage)?
                .is_empty()
        {
            continue;
        }
        required.insert(c.id.clone());
    }
    Ok(required)
}

fn missing_proof(id: &str, why: impl Into<String>) -> Fail {
    Fail::refuse(
        "missing-proofs",
        why,
        format!(
            "have the checks role supply the missing runs, then gdd report {id} checks <file> (or add --proofs <json file>)"
        ),
    )
}

#[derive(Default)]
struct CheckRuns<'a> {
    rejects: BTreeSet<&'a str>,
    passes: BTreeSet<&'a str>,
}

fn proof_runs<'a>(
    id: &str,
    required: &'a BTreeSet<String>,
    proofs: &'a [Value],
) -> R<BTreeMap<&'a str, CheckRuns<'a>>> {
    let bad = |why: String| {
        Fail::refuse(
            "bad-proofs",
            why,
            format!(
                "report the actual run in the json block, then gdd report {id} checks <file> (or add --proofs <json file>)"
            ),
        )
    };
    let mut runs: BTreeMap<&str, CheckRuns> = required
        .iter()
        .map(|id| (id.as_str(), CheckRuns::default()))
        .collect();
    for proof in proofs {
        let field = |key: &str| proof[key].as_str().map(str::trim).unwrap_or("");
        let check = field("check");
        let case = field("case");
        let Some(exit) = proof["exit"].as_i64() else {
            return Err(bad(format!(
                "the {case} proof for {check} needs the command's exit code"
            )));
        };
        if check.is_empty() || field("command").is_empty() || !proof["tail"].is_string() {
            return Err(bad(format!(
                "a {case} proof needs its check, command, exit and tail"
            )));
        }
        let runs = runs.entry(check).or_default();
        let test = field("test");
        match (case, exit) {
            ("rejects", n) if n != 0 => {
                if required.contains(check)
                    && log_kind(id) != LogKind::Setup
                    && (test.is_empty() || field("wrong").is_empty())
                {
                    return Err(bad(format!(
                        "the rejects proof for {check} needs the test name and the wrong solution it rejects"
                    )));
                }
                runs.rejects.insert(test);
            }
            ("passes", 0) => {
                runs.passes.insert(test);
            }
            _ => {
                return Err(bad(format!(
                    "the proof for {check} must say rejects with a failing exit or passes with exit 0"
                )));
            }
        }
    }
    Ok(runs)
}

fn check_proofs(l: &Loaded, id: &str, proofs: &[Value]) -> R<()> {
    let p = &l.state.progress;
    let setup = log_kind(id) == LogKind::Setup;
    let required = required_proofs(l, id)?;
    let runs = proof_runs(id, &required, proofs)?;
    let zone_trial = id.starts_with("gates-");
    if zone_trial && runs.is_empty() {
        return Err(missing_proof(
            id,
            "setting up a check needs both a rejects proof and a passes proof",
        ));
    }
    let mut scenario_tests = 0;
    for (check, runs) in runs {
        if !zone_trial && !required.contains(check) {
            continue;
        }
        if runs.rejects.is_empty() {
            return Err(missing_proof(
                id,
                format!("missing the rejects proof for {check}"),
            ));
        }
        scenario_tests += runs.rejects.len();
        if !setup && p.color.as_deref() != Some("red") {
            continue;
        }
        let missing_pass =
            |name: &str| missing_proof(id, format!("missing the passes proof for {name}"));
        if runs.passes.is_empty() {
            return Err(missing_pass(check));
        }
        // A passing run of the whole check covers all its tests.
        if !setup
            && !runs.passes.contains("")
            && let Some(test) = runs.rejects.difference(&runs.passes).next()
        {
            return Err(missing_pass(&format!("{check} {test}")));
        }
    }
    if !setup && !required.is_empty() {
        let start = p.start.as_deref().unwrap_or("");
        let count = p
            .scenario_count(id, |file| git::show(&l.project.root, start, file))
            .unwrap_or(0);
        if scenario_tests < count {
            return Err(missing_proof(
                id,
                format!(
                    "the sealed brief lists {count} scenarios but only {scenario_tests} distinct scenario tests have a rejects proof"
                ),
            ));
        }
    }
    Ok(())
}

/// The checks sealed before, when they still stand for a new version of the brief: since the last
/// checks step, the seal changed nothing but the brief's prose, and the change's color didn't rise.
/// What a merge of the main branch brought counts as no change: a file the seal holds as main had
/// it when the seal last took main in, unless it is one of this change's checks or brief files.
/// A scenario file another change shares still needs the role: telling its scenarios apart would
/// mean reading them one by one.
/// A new scenario, a check that changed or a higher color need the role to seal again.
fn standing_checks(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    tip: &str,
) -> Option<(Map<String, Value>, Vec<Value>)> {
    let p = &l.state.progress;
    let (line, then) = p.checked.as_ref()?;
    let seal = &l.entries.get(line - 1)?.event()?["seal"];
    let at = seal["commit"].as_str()?;
    let rank = |c: &Option<String>| c.as_deref().and_then(crate::color_rank);
    let prose = format!("gdd/briefs/{id}.md");
    // The main branch as the seal last took it in: the second parent of its newest merge since
    // then, not the main ref, which may have moved or come to hold the seal. A merge made by hand
    // brings nothing from main when main never held its parent, or when the seal already held it.
    let main_ref = git::branch_ref(&git::main_branch(&ctx.root));
    let main = git::first_parents(&ctx.root, at, tip)
        .ok()?
        .iter()
        .rev()
        .find_map(|c| match git::parents(&ctx.root, c).as_slice() {
            [before, taken, ..] => Some((before.clone(), taken.clone())),
            _ => None,
        })
        .filter(|(before, taken)| {
            !git::is_ancestor(&ctx.root, taken, before)
                && git::is_ancestor(&ctx.root, taken, &main_ref)
        })
        .map(|(_, taken)| taken);
    // A file the seal no longer holds is always a change.
    let from_main = |f: &str| {
        !p.checks_sealed.contains_key(f)
            && !p.brief_files.contains_key(f)
            && main.as_deref().is_some_and(|m| {
                let held = git::blob_at(&ctx.root, tip, f);
                held.is_some() && held == git::blob_at(&ctx.root, m, f)
            })
    };
    if rank(&p.color) > rank(then)
        || !git::is_ancestor(&ctx.root, at, tip)
        || changes(ctx, at, tip)
            .ok()?
            .iter()
            .any(|f| *f != prose && !from_main(f))
    {
        return None;
    }
    let s: Map<String, Value> = p
        .checks_sealed
        .keys()
        .filter_map(|f| git::blob_at(&ctx.root, tip, f).map(|h| (f.clone(), json!(h))))
        .collect();
    (!s.is_empty()).then(|| (s, seal["proofs"].as_array().cloned().unwrap_or_default()))
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
    // A build that stopped on a branch without <start> judged checks that are no longer the
    // change's: in one run its CANNOT blamed a test already fixed on the seal, and the checks role,
    // sent back for it, had nothing to seal.
    if (stopped || p.merging_checks_fix())
        && let (Some(h), Some(s)) = (&l.state.git.head, &p.start)
        && !git::is_ancestor(&l.project.root, s, h)
    {
        return Err(Fail::refuse(
            "start-missing",
            format!(
                "the build on {} doesn't hold <start> ({}): what it ran judged checks that are no longer the change's",
                git::short(h),
                git::short(s)
            ),
            format!(
                "launch it again, which merges <start> into its branch first: gdd launch {id} build"
            ),
        ));
    }
    // What it says each check gave: --claims, or the json block of its last message.
    let mut claims = if let Some(path) = &a.claims {
        json_file(path)?
    } else {
        commands_ran(r)
    };
    for claim in &mut claims {
        if let Some(count) = claim.get_mut("failures")
            && !count.is_u64()
        {
            *count = Value::Null;
        }
    }
    m.insert("claims".into(), Value::Array(claims));
    if let Some(decisions) = &r.decisions {
        m.insert("decisions".into(), json!(decisions));
    }
    Ok(())
}

/// What a role ran, from the json block of its last message: its commands, as it gave them.
fn commands_ran(r: &RoleReport) -> Vec<Value> {
    r.list
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter(|x| x.get("command").is_some_and(|c| c.is_string()))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The words of the refusal that sends a report back, `unbacked-fix` or `unbacked-pass`, for the
/// orchestrator and for the reviewer's prompt when it is resumed.
pub(super) fn backed(reason: &str) -> &'static str {
    if reason == "unbacked-pass" {
        BACKED_PASS
    } else {
        BACKED_FIX
    }
}

const BACKED_FIX: &str = "Each numbered finding of a fix ends with its citation: | @id:<scenario>, | check <id>, | zone <name>: <rule>, | constitution: <rule> or | request: <the person's words>. On red, after it, | ran: <the command of your json block that shows it fail>.";

const BACKED_PASS: &str = "On red, a pass gives, right above the gdd-report block, the json block of each command it ran, with its exit code and last lines.";

/// Whether the reviewer runs the change: on a red change, not on the setup of checks.
fn runs_the_change(l: &Loaded, id: &str) -> bool {
    log_kind(id) == LogKind::Change && l.state.progress.color.as_deref() == Some("red")
}

/// What each finding of a fix lacks to back it, a line each. A fix holds the change for what its
/// findings say: each one cites its source and, on red, where the reviewer runs the change, names
/// the command of its json block that shows it fail. One that isn't goes back to the reviewer who
/// wrote it, never to a fresh one, which could miss what this one found (decision 107).
fn unbacked(l: &Loaded, id: &str, r: &RoleReport) -> Vec<String> {
    let red = runs_the_change(l, id);
    let ran: Vec<String> = commands_ran(r)
        .iter()
        .map(|x| report::plain_command(str_of(x, "command")))
        .collect();
    let mut lacks = vec![];
    if r.findings.is_empty() {
        lacks.push("no numbered finding: f1 <file>:<line> <what> | <its citation>".to_string());
    }
    for f in &r.findings {
        let mut what = vec![];
        if r.uncited.contains(&f.id) {
            what.push("no citation".to_string());
        }
        match report::ran_of(&f.what).map(|(_, c)| report::plain_command(c)) {
            _ if !red => {}
            None => what.push("no last | ran: <command>".into()),
            Some(c) if !ran.contains(&c) => what.push(format!(
                "its | ran: {c} isn't a command of its json block{}",
                closest(&ran, &c)
            )),
            Some(_) => {}
        }
        if !what.is_empty() {
            lacks.push(format!("{}: {}", f.id, what.join(", and ")));
        }
    }
    lacks
}

/// The command of the json block that shares the most words with the one a finding named.
fn closest(ran: &[String], named: &str) -> String {
    let shared = |c: &&String| {
        c.split(' ')
            .filter(|w| named.split(' ').any(|n| n == *w))
            .count()
    };
    match ran.iter().rev().max_by_key(shared) {
        Some(c) => format!(" (the closest there: {c})"),
        None => ", which lists none".into(),
    }
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
    let launch_review = || {
        if l.state.next.step == "gates" {
            format!("gdd gates {id}, then gdd launch {id} review")
        } else {
            format!("gdd launch {id} review")
        }
    };
    let read = if verdict == "pass" {
        r.reviewed.as_ref().or(a.launch_reviewed.as_ref())
    } else {
        r.reviewed.as_ref()
    };
    if let Some(read) = read {
        let (current, bare_commit) = review_stamp(l).unwrap_or_else(|| ("none".into(), None));
        if !read.eq_ignore_ascii_case(&current)
            && !bare_commit.is_some_and(|head| read.eq_ignore_ascii_case(head))
        {
            return Err(Fail::refuse(
                "stale-review",
                format!("reviewed: {read}\ncurrent: {current}"),
                launch_review(),
            ));
        }
    } else if verdict == "pass" && review_head(l).is_some() {
        // Only its reviewer can say what it read, so the refusal doesn't print the value to copy.
        // While the gates are pending, the line of its prompt is stale already.
        let way = if l.state.next.step == "gates" {
            launch_review()
        } else {
            format!(
                "Ask the reviewer who wrote it to send its whole report again with the reviewed: line from its prompt, or run {}",
                launch_review()
            )
        };
        return Err(Fail::refuse(
            "missing-reviewed",
            "This pass doesn't say what its reviewer read: its reviewed: line is missing or has no commit.",
            way,
        ));
    }
    let head = review_head(l).map(str::to_owned);
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
    // A color that rose sends the change back to Brief whatever its findings say: it is recorded.
    let rank = |c: Option<&str>| c.and_then(crate::color_rank);
    let rose = rank(colorobj["color"].as_str()) > rank(p.color.as_deref());
    let ran = commands_ran(r);
    // On red the reviewer runs the change: a pass that ran nothing didn't look (decision 112).
    let ran_one = ran
        .iter()
        .any(|x| !str_of(x, "command").trim().is_empty() && x["exit"].is_i64());
    let back = if verdict == "fix" && !rose {
        Some(("unbacked-fix", unbacked(l, id, r)))
    } else if verdict == "pass" && !ran_one && runs_the_change(l, id) {
        Some((
            "unbacked-pass",
            vec![
                "a pass on red with no json block of the commands it ran and their exit codes"
                    .into(),
            ],
        ))
    } else {
        None
    };
    if let Some((reason, lacks)) = back.filter(|(_, lacks)| !lacks.is_empty()) {
        // The journal keeps them, and the resumed reviewer's prompt repeats them.
        let mut journal = out.journal.take().unwrap_or_else(|| json!({}));
        journal["unbacked"] = json!(lacks);
        out.journal = Some(journal);
        return Err(Fail::refuse(
            reason,
            format!(
                "this {verdict} goes back to the reviewer who wrote it:\n- {}\n{}",
                lacks.join("\n- "),
                backed(reason)
            ),
            format!(
                "send these lines to that reviewer, in its own session, for its whole report again: gdd launch {id} review --run --command \"<the command that resumes it>\", or by hand, then gdd report {id} review <file>. Never a fresh reviewer, which would start over"
            ),
        ));
    }
    m.insert("color".into(), colorobj);
    m.insert("verdict".into(), json!(verdict));
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
    if verdict == "fix" && log_kind(id) == LogKind::Change {
        m.insert("fix_role".into(), json!(p.fix_role(&m["findings"])));
    }
    let previous = log::events(&l.entries)
        .into_iter()
        .rev()
        .find(|(_, v)| step(v) == "review")
        .and_then(|(_, v)| v["findings"].as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let closed = a.closed.as_deref().or(r.closed.as_deref()).unwrap_or("");
    let outcomes = closed_findings(closed, previous).unwrap_or_else(|| {
        out.say(format!(
            "no previous findings were closed because the layer could not read closed: {closed}"
        ));
        m.insert("unparsed_closed".into(), json!(closed));
        Vec::new()
    });
    m.insert("closed".into(), json!(outcomes));
    if !r.notes.is_empty() {
        m.insert("notes".into(), json!(r.notes));
    }
    Ok(())
}

fn closed_findings(text: &str, previous: &[Value]) -> Option<Vec<Value>> {
    let mut closed = Vec::new();
    if text.trim().is_empty() || text.trim().eq_ignore_ascii_case("none") {
        return Some(closed);
    }
    let mut seen = BTreeSet::new();
    for group in text.split(',') {
        let spaced = group.replace('=', " = ");
        let mut words = spaced.split_whitespace().peekable();
        words.peek()?;
        while let Some(id) = words.next() {
            let id = id.to_ascii_lowercase();
            if words.next()? != "="
                || !seen.insert(id.clone())
                || !previous.iter().any(|f| f["id"] == id)
            {
                return None;
            }
            let outcome = words.next()?;
            if !matches!(outcome, "fixed" | "false" | "intended" | "minor") {
                return None;
            }
            closed.push(json!({"id":id, "outcome":outcome}));
        }
    }
    Some(closed)
}

/// What a reading of a brief adds: its findings, and where each one went. The orchestrator who
/// wrote the brief drops a finding only with the line that answers it, one the reader could read:
/// the reader backs each finding, and the author each drop (decision 111).
fn report_reading(
    ctx: &Ctx,
    l: &Loaded,
    a: &ReportArgs,
    r: &RoleReport,
    m: &mut Map<String, Value>,
) -> R<()> {
    if r.findings.len() > 3 {
        return Err(Fail::refuse(
            "too-many-additions",
            "the reading proposes more than three additions",
            "number up to three blocking additions and put the remaining observations under LATER: for the log, then report the reading again",
        ));
    }
    if !r.missing.is_empty() {
        m.insert("missing".into(), json!(r.missing));
    }
    let mut went: BTreeMap<&str, (&str, Option<String>)> = BTreeMap::new();
    for (k, w) in &a.went {
        if !r.findings.iter().any(|f| f.id == *k) || went.contains_key(k.as_str()) {
            return Err(Fail::usage(format!(
                "--went {k}={w}: give each numbered finding of the reading once"
            )));
        }
        let to = if matches!(w.as_str(), "brief" | "person" | "log") {
            (w.as_str(), None)
        } else if w == "dropped" || w.starts_with("dropped:") {
            let line = w
                .strip_prefix("dropped:")
                .and_then(|at| readable_line(ctx, l, at));
            let Some(line) = line else {
                return Err(Fail::refuse(
                    "unbacked-drop",
                    format!(
                        "{k} is dropped without a line the reader could read that answers it: {w}"
                    ),
                    format!(
                        "--went {k}=dropped:<file>:<line>, a line of the sealed brief or of the project; if only the person knows the answer, --went {k}=person"
                    ),
                ));
            };
            ("dropped", Some(line))
        } else {
            return Err(Fail::usage(format!(
                "--went {k}={w}: a finding goes to brief, person, log or dropped:<file>:<line>"
            )));
        };
        went.insert(k, to);
    }
    let mut findings = vec![];
    for (i, f) in r.findings.iter().enumerate() {
        let (to, by) = went.get(f.id.as_str()).cloned().unwrap_or(("log", None));
        let mut finding = json!({"id": f.id, "question": i + 1, "what": f.what, "went": to});
        if let Some(by) = by {
            finding["refuted_by"] = json!(by);
        }
        findings.push(finding);
    }
    m.insert("findings".into(), json!(findings));
    Ok(())
}

/// `<file>:<line>` as `file:n`, when that line holds something in what the reader read: the
/// commit its brief was sealed at or, for a brief with no seal, its base. Only with neither, the
/// working copy.
fn readable_line(ctx: &Ctx, l: &Loaded, at: &str) -> Option<String> {
    let (file, n) = at.rsplit_once(':')?;
    let (file, n) = (color::clean_path(file).ok()?, n.parse::<usize>().ok()?);
    let p = &l.state.progress;
    let read = p
        .brief_line
        .and_then(|i| l.entries[i - 1].event())
        .and_then(|b| b["seal"].as_str().map(str::to_owned))
        .or_else(|| p.base.clone());
    let bytes = match read {
        Some(commit) => git::blob_bytes(&ctx.root, &commit, &file)?,
        None => std::fs::read(ctx.root.join(&file)).ok()?,
    };
    let text = String::from_utf8_lossy(&bytes);
    let line = text.lines().nth(n.checked_sub(1)?)?;
    (!line.trim().is_empty()).then(|| format!("{file}:{n}"))
}
