//! Launching a role: its model, its prompt and, when the blueprint gives a command, its run.

use super::*;

/// The environment, the model and the launch of a role, from the blueprint's `levels`.
pub(super) fn model_for(
    bp: &Blueprint,
    env: &Option<String>,
    role: &str,
    level: &str,
) -> R<(String, String, Option<String>)> {
    let envs = bp.environments();
    let wanted = env.clone().or_else(|| std::env::var("GDD_ENV").ok());
    let e = match wanted {
        Some(w) => envs.iter().find(|e| e.name == w).ok_or(Fail::refuse(
            "no-environment",
            format!(
                "the blueprint's levels don't list the environment {w}: stop and ask the person"
            ),
            "the blueprint's levels name each environment, and gdd-prepare sets them: ask the person to add this one, or pass --env with one it names",
        ))?,
        None if envs.len() == 1 => &envs[0],
        None if envs.is_empty() => {
            return Err(Fail::refuse(
                "no-environment",
                "the blueprint has no levels",
                "gdd-prepare sets the blueprint's levels, which the person approves: ask them, and stop until then",
            ));
        }
        None => {
            return Err(Fail::usage(format!(
                "say which environment with --env: {}",
                envs.iter()
                    .map(|e| e.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };
    let models = match (&e.reviewer, role) {
        (Some(r), "review" | "reading") => r,
        _ => &e.models,
    };
    let model = models.of(level).ok_or(Fail::refuse(
        "no-model",
        format!(
            "the blueprint gives {} no model for {level}: stop and ask, never use another",
            e.name
        ),
        "ask the person to name the model in the blueprint's levels (gdd-prepare), or pass --level with one it names",
    ))?;
    let launch = models.launch.clone().or(e.models.launch.clone());
    Ok((e.name.clone(), model.to_string(), launch))
}

/// A fresh role can run and report in one call. A returned build or set of checks needs
/// the prompt for the agent that did the work, which may have been launched by hand.
pub(super) fn prefer_run(ctx: &Ctx, bp: &Blueprint, state: &mut State) {
    let next = &mut state.next;
    if !next.command.starts_with("gdd launch ") {
        return;
    }
    if let Some(env) = &ctx.env {
        next.command
            .push_str(&format!(" --env {}", gates::quote(env)));
    }
    let p = &state.progress;
    let role = next.step.as_str();
    if returned_role(p, role) {
        return;
    }
    let level = level_of(bp, p, role, None);
    if let Ok((_, _, Some(template))) = model_for(bp, &ctx.env, role, &level)
        && launch::is_command(&template)
    {
        next.command.push_str(" --run");
    }
}

fn returned_role(p: &state::Progress, role: &str) -> bool {
    (role == "build" && p.builds > 0) || (role == "checks" && p.checks_runs > 0)
}

/// What a setup log sets up, in words for its roles: `standing-checks`, `standing-checks-<check>`
/// or `gates-<zone>`.
fn setup_subject(id: &str) -> (String, String) {
    if let Some(check) = id.strip_prefix("standing-checks-") {
        (
            format!("the check {check}, which the color of a new zone asks for"),
            format!("the new check {check}"),
        )
    } else if let Some(zone) = id.strip_prefix("gates-") {
        let zone = match zone.rsplit_once('-') {
            Some((z, n)) if n.chars().all(|c| c.is_ascii_digit()) => z,
            _ => zone,
        };
        (
            format!(
                "the gate of the zone {zone}: the check of each new `Rule:` in gdd/gates/{zone}.feature, built from its scenarios"
            ),
            format!("the new gate of the zone {zone}"),
        )
    } else {
        (
            "the standing checks the blueprint's `checks` name whose `when` is empty or matches a tracked file, from the blueprint and the constitution".into(),
            "the new standing checks".into(),
        )
    }
}

/// Whether the project has code yet: a tracked file that is not one of its GDD files, its
/// scenarios, its agents' files, its CI, its documents or its data.
pub(super) fn has_code(root: &Path) -> bool {
    let Ok(files) = git::ls_files(root, &[]) else {
        return true;
    };
    files.iter().any(|path| {
        const NOT_CODE: [&str; 8] = [
            "gdd/",
            "features/",
            "data/",
            ".agents/",
            ".claude/",
            ".codex/",
            ".gdd/",
            ".github/",
        ];
        if NOT_CODE.iter().any(|d| path.starts_with(d)) {
            return false;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        match name.rsplit_once('.') {
            // README, LICENSE, CODEOWNERS: documents, not code.
            None => !name.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            Some(("", _)) => false,
            Some((_, ext)) => !matches!(
                ext.to_ascii_lowercase().as_str(),
                "md" | "txt"
                    | "json"
                    | "csv"
                    | "tsv"
                    | "yaml"
                    | "yml"
                    | "xml"
                    | "toml"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "svg"
                    | "webp"
                    | "pdf"
            ),
        }
    })
}

/// What every part of a role's prompt names: the change, the role and where things are.
struct PromptFor<'a> {
    l: &'a Loaded,
    id: &'a str,
    role: &'a str,
    level: &'a str,
    at: &'a str,
    root: &'a Path,
    log_path: &'a Path,
}

/// What a role gets when the standing checks are set up or tried.
fn setup_prompt(s: &mut Vec<String>, x: &PromptFor) {
    let PromptFor {
        l,
        id,
        role,
        level,
        at,
        root,
        log_path,
    } = *x;
    let p = &l.state.progress;
    s.push(format!(
        "Load {} and do its work for the project's {id} of this GDD project (contract gdd/1), and nothing else.",
        launch::skill_ref(role)
    ));
    s.push(at.to_string());
    let (what, tried) = setup_subject(id);
    let seal = l.state.git.seal_tip.clone().or(p.start.clone());
    match role {
        "checks" => {
            let on = if l.state.git.seal_tip.is_some() {
                format!("on gdd-seal/{id}, on top of its tip")
            } else {
                format!("on gdd-seal/{id}, cut from the main branch")
            };
            s.push(format!(
                "Set up {what}: each one with a tool that already exists before a script of the project's own, proved once in a throwaway clone, its bad case planted and seen failing, then seen passing without it, and sealed {on}."
            ));

        }
        "review" => s.push(format!(
            "Try {tried} on gdd-seal/{id}{}, in a fresh clone: follow references/standing-checks.md of the skill. Each check three ways, a round each: it rejects a planted bad case, it accepts without it, and a change can't switch it off. When the blueprint names its gates, read each gate's feature against the blueprint too. Its log is {}.",
            seal.map(|c| format!(", at {c}")).unwrap_or_default(),
            log_path.display()
        )),
        _ => {}
    }
    if id == "standing-checks" && (role == "checks" || role == "review") && !has_code(root) {
        s.push(
            "The project has no code yet: only the seal and the secret scan are set up now. Every other check waits with its `when` in the blueprint, and is set up by its own item, standing-checks-<check>, once a change brings what it watches."
                .into(),
        );
    }
    if let Some(b) = l.state.git.main_tip.clone() {
        s.push(format!("<base> is {b}, the tip of the main branch."));
    }
    s.push(
        "It has no brief, and its color is the one its paths give: the files of the checks, of CI and of each tool's configuration."
            .into(),
    );
    s.push(format!("Work at the {level} level."));
}

/// What a role gets for a change: its brief, its seal, and what its own role needs.
fn change_prompt(s: &mut Vec<String>, x: &PromptFor) {
    let PromptFor {
        l,
        id,
        role,
        level,
        at,
        log_path,
        ..
    } = *x;
    let p = &l.state.progress;
    s.push(format!(
        "Load {} and do its work for the change {id} of this GDD project (contract gdd/1), and nothing else.",
        launch::skill_ref(role)
    ));
    s.push(at.to_string());
    if role == "reading" {
        s.push("You are to read a brief before anyone builds it: follow references/brief.md of the skill.".into());
    }
    if let Some(decisions) = &p.decisions
        && !decisions.is_empty()
        && matches!(role, "review" | "build")
    {
        let instruction = if role == "review" {
            "Within this review's scope, check these product choices against the product rules and the brief or request. If a choice contradicts a written rule, report the finding and cite the rule."
        } else {
            "These were the last builder's product choices. Update the list in your report to match what you built."
        };
        s.push(instruction.into());
        s.extend(decisions.iter().map(|choice| format!("- {choice}")));
    }
    if role == "review"
        && let Some(size) = counting::size_warning(l)
    {
        s.push(if p.reviews == 0 {
            format!("{size} Review it whole. Size alone is no finding.")
        } else {
            format!("{size} Size alone is no finding.")
        });
    }
    let brief = format!("gdd/briefs/{id}.md");
    if l.project.path(&brief).is_file() || !p.brief_files.is_empty() {
        let scen: Vec<&String> = p
            .brief_files
            .keys()
            .filter(|f| f.starts_with("features/"))
            .collect();
        s.push(format!(
            "The brief: {brief}{}{}.",
            p.version
                .map(|v| format!(", version {v}"))
                .unwrap_or_default(),
            if scen.is_empty() {
                String::new()
            } else {
                format!(
                    ", with its scenarios in {}",
                    scen.iter()
                        .map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ));
        if let Some(r) = &p.request
            && (role == "build" || role == "review")
        {
            s.push(format!("The person's request, word for word: \"{r}\""));
        }
    } else {
        match &p.request {
            Some(r) => s.push(format!("There is no brief. The person's request, word for word: \"{r}\"")),
            None => s.push("There is no brief, and the log keeps no request: ask the orchestrator for it, word for word, before you start.".into()),
        }
        if !p.paths.is_empty() {
            s.push(format!("The paths it touches: {}.", p.paths.join(", ")));
        }
    }
    if let Some(b) = &p.base {
        s.push(format!("<base> is {b}."));
    }
    if let Some(st) = &p.start {
        s.push(format!("<start> is {st}."));
    }
    match &p.color {
        Some(c) => s.push(format!("The color is {c}, at the {level} level.")),
        None => s.push(format!("Work at the {level} level.")),
    }
    let branch = l
        .state
        .git
        .branch
        .clone()
        .unwrap_or_else(|| format!("work/{id}"));
    match role {
        "checks" => s.push(format!(
            "Before sealing, read each scenario test and strengthen it if a wrong solution would pass. Read each effect where its scenario says it can be seen. In each rejecting json proof, name the test in `test` and the wrong solution it rejects in `wrong`. Seal what you write on gdd-seal/{id}, on top of its tip. Don't read gdd/log/: what you need is here, and an earlier round's stop isn't yours."
        )),
        "build" if p.builds == 0 => s.push(format!(
            "Work on the branch {branch}, which starts at <start>, in a worktree of your own inside the project folder, and commit there."
        )),
        // A later round goes on where the earlier ones left it, and the seal may have moved since.
        "build" => s.push(format!(
            "Work on the branch {branch}, where the earlier rounds are, in a worktree of your own inside the project folder, and commit there. It has to hold <start>: merge <start> into it when it doesn't."
        )),
        "review" => s.push(format!(
            "Review the change at {}, in a fresh clone. Its log is {}.",
            review_head(l).map(str::to_owned).unwrap_or_else(|| format!("the tip of {branch}")),
            log_path.display()
        )),
        _ => {}
    }
}

/// A command can leave its whole diagnostic on one long line.
fn output_tail(text: &str, width: usize) -> &str {
    match text.char_indices().rev().nth(width) {
        Some((i, c)) => &text[i + c.len_utf8()..],
        None => text,
    }
}

/// Include the builder's own claims, never the proofs that could give its solution away.
fn last_attempt(s: &mut Vec<String>, build: &Value) {
    if let Some(reason) = build["stop"]["reason"].as_str() {
        s.push(format!("The builder stopped: {reason}"));
    }
    let claims = build["claims"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if claims.is_empty() {
        s.push("The last build has no recorded claims.".into());
        return;
    }
    // Show failed attempts first; a long suite of passing checks can hide them.
    let mut runs: Vec<_> = claims.iter().collect();
    runs.sort_by_key(|c| c["exit"].as_i64() == Some(0));
    s.push("The last builder reported these runs:".into());
    for c in runs.iter().take(3) {
        s.push(format!(
            "- {}: `{}` exit {}; {}",
            str_of(c, "check"),
            str_of(c, "command"),
            c["exit"],
            output_tail(str_of(c, "tail"), 100)
        ));
    }
    if runs.len() > 3 {
        s.push(format!("{} other reported runs omitted.", runs.len() - 3));
    }
}

fn list_findings(s: &mut Vec<String>, review: &Value) {
    for f in review["findings"].as_array().into_iter().flatten() {
        s.push(format!(
            "- {} {}: {}",
            str_of(f, "id"),
            str_of(f, "where"),
            str_of(f, "what")
        ));
    }
}

/// A fresh reviewer gets the last verdict's findings, never the builder's conversation.
fn review_after_fix(s: &mut Vec<String>, l: &Loaded) {
    let Some((line, r)) = log::events(&l.entries)
        .into_iter()
        .rev()
        .find(|(_, v)| step(v) == "review")
    else {
        return;
    };
    s.push("First report how each previous finding ended. Retest only what changed.".into());
    if let Some(before) = r["color"]["head"].as_str() {
        s.push(format!("Previous review at {before}."));
    }
    list_findings(s, r);
    if l.state.progress.brief_line.is_some_and(|b| b > line) {
        s.push("The brief changed. Include its revised requirements and the checks its color now asks for.".into());
    }
    if let Some(paths) = review_head(l).and_then(|h| changed_tests(l, h)) {
        s.push("The product has not changed since that review.".into());
        if paths.is_empty() {
            s.push("No test changed. Check only whether the previous findings still hold.".into());
        } else {
            s.push(format!(
                "Changed tests: {}. Review only those tests against the previous findings.",
                paths.join(", ")
            ));
        }
    }
}

fn fix_round_notes(s: &mut Vec<String>, l: &Loaded, role: &str, setup: bool) {
    let p = &l.state.progress;
    let events: Vec<_> = log::events(&l.entries)
        .into_iter()
        .filter(|(line, _)| *line > p.brief_line.unwrap_or(0))
        .collect();
    match l.state.git.seal_tip.as_deref() {
        Some(tip) => s.push(format!("Current seal tip: {tip}.")),
        None if p.sealed_any => {
            s.push("The seal branch is absent here. <start> is the recorded tip.".into())
        }
        None => s.push("No seal yet.".into()),
    }
    if role == "build"
        && let Some(g) = &p.last_gates
        && !g.passed
        && let Some((_, v)) = events.iter().find(|(line, _)| *line == g.line)
    {
        s.push("A fix round. What failed at the gates:".into());
        for e in v["gates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["result"] == "fail")
        {
            s.push(format!(
                "- {}: `{}` exit {}; last lines: {}",
                str_of(e, "check"),
                str_of(e, "command"),
                e["exit"],
                output_tail(str_of(e, "tail"), 320)
            ));
        }
    }
    if let Some((line, r)) = events
        .iter()
        .rev()
        .find(|(_, v)| step(v) == "review")
        .filter(|(line, v)| {
            matches!(str_of(v, "verdict"), "fix" | "stop")
                && p.last_gates.as_ref().is_none_or(|g| g.line < *line)
        })
    {
        s.push(format!(
            "A fix round. The reviewer said {}.",
            str_of(r, "verdict")
        ));
        if setup
            && let Some((_, a)) = events
                .iter()
                .rev()
                .find(|(n, v)| n > line && log::kind(v) == "answer" && v["of"] == "stop")
        {
            s.push(format!(
                "The person's answer, word for word: \"{}\".",
                str_of(a, "what")
            ));
        }
        list_findings(s, r);
        if role == "checks" {
            s.push("What a finding asks of a check is yours to answer, with a new seal; the rest is the builder's.".into());
        } else {
            if let Some(n) = r["notes"].as_str()
                && p.last_build.is_none_or(|(last, _)| last < *line)
            {
                s.push(format!("NOTES FOR THE BUILDER:\n{n}"));
            }
            if events
                .iter()
                .any(|(n, v)| n > line && step(v) == "checks" && v["seal"]["commit"].is_string())
            {
                s.push("The checks were sealed again after that review. What a finding asked of a check was answered there. Make the checks pass as they are now.".into());
            }
        }
    }
    if let Some((_, build)) = events.iter().rev().find(|(_, v)| step(v) == "build") {
        last_attempt(s, build);
    }
    s.push("Use these results to diagnose the fix. Run the required checks on the fix.".into());
}

fn prompt_for(l: &Loaded, id: &str, role: &str, level: &str) -> String {
    let p = &l.state.progress;
    let mut s = vec![];
    let setup = log_kind(id) == LogKind::Setup;
    let root = std::fs::canonicalize(&l.project.root).unwrap_or_else(|_| l.project.root.clone());
    let log_path = root.join(Project::log_rel(id));
    // A role loads its skill from another folder, and one was seen starting its work there instead
    // of in the project: say where the project is.
    let at = format!("The project is the folder {}.", root.display());
    let x = PromptFor {
        l,
        id,
        role,
        level,
        at: &at,
        root: &root,
        log_path: &log_path,
    };
    if setup {
        setup_prompt(&mut s, &x);
    } else {
        change_prompt(&mut s, &x);
    }
    let zones: Vec<String> = p.zones.clone();
    let ctx_files: Vec<String> =
        l.bp.context()
            .into_iter()
            .filter(|c| {
                c.targets.iter().any(|t| {
                    zones
                        .iter()
                        .any(|z| t == z || t.starts_with(&format!("{z}/")))
                })
            })
            .map(|c| format!("{} {}", c.kind, c.name))
            .collect();
    if !ctx_files.is_empty() {
        s.push(format!(
            "The context the blueprint gives its gates: {}.",
            ctx_files.join(", ")
        ));
    }
    let round = 1 + match role {
        "checks" => p.checks_runs,
        "build" => p.builds,
        "review" => p.reviews,
        _ => p.readings_n,
    };
    s.push(if log_kind(id) == LogKind::Setup {
        format!("This is round {round} of {role} for {id}.")
    } else {
        format!("This is round {round} of {role} for this change.")
    });
    if round > 1 && matches!(role, "build" | "checks") {
        fix_round_notes(&mut s, l, role, setup);
    }
    if round > 1 && role == "review" && !setup {
        review_after_fix(&mut s, l);
    }
    // What the log can show of a role's own runs comes from a json block, like the checks
    // role's proofs: without it, a claim or a trial is only prose.
    match role {
        "build" => s.push(
            "Before the gdd-report block, give a json block with one claim per check you ran this round. Count every failed run in failures, including a final failure. Use 0 when it never failed. Give the last run's evidence: [{\"check\":\"<check id>\",\"failures\":0,\"command\":\"<command>\",\"exit\":0,\"tail\":\"<its last lines>\"}]."
                .into(),
        ),
        "review" => s.push(
            "Before the gdd-report block, give a json block with each command you ran: [{\"command\":\"<command>\",\"exit\":0,\"tail\":\"<its last lines>\"}]."
                .into(),
        ),
        _ => {}
    }
    s.push("End your last message with its gdd-report block, filled in.".into());
    s.join("\n")
}

#[derive(Debug, Clone)]
pub struct LaunchArgs {
    pub run: bool,
    pub command: Option<String>,
    pub level: Option<String>,
    pub start_wait: Duration,
    pub timeout: Option<Duration>,
    pub adapter: Option<String>,
}

/// What the state lets start: the same rules as the step this launch will write.
fn may_launch(ctx: &Ctx, l: &Loaded, id: &str, role: &str) -> R<()> {
    let p = &l.state.progress;
    if role != "reading" {
        allowed(l, id, role, None)?;
        if role == "review"
            && let Some(g) = p.last_gates.as_ref()
            && let (Some(gh), Some(h)) = (g.head.as_deref(), review_head(l))
            && gh != h
        {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "the gates passed on {}, and the head is now {}",
                    git::short(gh),
                    git::short(h)
                ),
                format!("gdd gates {id}"),
            ));
        }
    } else if p.built_since_brief() {
        return Err(Fail::refuse(
            "out-of-order",
            "a brief is read before anyone builds it",
            way_on(l, id),
        ));
    }
    if (role == "checks" || role == "build") && !l.state.limits.stop.is_empty() {
        return Err(Fail::refuse(
            "cap-spent",
            format!(
                "no new round: {} no room left",
                l.state.limits.stop.join(", ")
            ),
            cap_way_on(l),
        ));
    }
    if role == "build" && l.state.next.step != "build" {
        return Err(Fail::refuse(
            "out-of-order",
            format!(
                "the next step is {}, not a build: {}",
                l.state.next.step, l.state.next.say
            ),
            way_on(l, id),
        ));
    }
    if role == "checks" || role == "build" {
        git_writes(ctx)?;
        git_commits(ctx)?;
    }
    Ok(())
}

/// Without one command to run, the layer says how to launch the role by hand.
fn say_by_hand(
    out: &mut Out,
    id: &str,
    role: &str,
    model: &str,
    prompt: &str,
    launch_text: Option<&str>,
    run: bool,
) -> R<()> {
    out.say(format!(
        "launch it the way the blueprint says: {}",
        launch_text.unwrap_or("(no launch)")
    ));
    out.say(format!("with the model {model}, and this prompt:"));
    for line in prompt.lines() {
        out.say(format!("  {line}"));
    }
    out.say(format!(
        "then: gdd report {id} {role} <its last message, a file or ->"
    ));
    if run {
        return Err(Fail::refuse(
            "not-a-command",
            "the blueprint's launch is not one command with <model>: the layer can't run it",
            "launch it yourself as printed, or pass --command \"<program> ... <model> ... <prompt>\"",
        ));
    }
    Ok(())
}

/// Who is launched, and how the blueprint named it.
struct Launched<'a> {
    id: &'a str,
    role: &'a str,
    env: &'a str,
    level: &'a str,
    model: &'a str,
}

/// Run the role's command, journal what was seen of it and write its step from its last message.
fn run_role(
    ctx: &Ctx,
    l: &Loaded,
    who: &Launched,
    a: &LaunchArgs,
    argv: &[String],
    runs: &Path,
    out: &mut Out,
) -> R<()> {
    let Launched {
        id,
        role,
        env,
        level,
        model,
    } = *who;
    let adapter = match &a.adapter {
        Some(n) => Adapter::by_name(n).ok_or(Fail::usage("--adapter is claude, codex or plain"))?,
        None => Adapter::of_program(&argv[0]),
    };
    let left = l
        .state
        .limits
        .line("minutes_per_change", "minutes")
        .and_then(|m| m.cap.map(|c| (c - m.used as i64) * 60))
        .filter(|s| *s > 0);
    let timeout = a
        .timeout
        .unwrap_or(Duration::from_secs(left.unwrap_or(15 * 60) as u64));
    let output = runs.join("output.txt");
    let errors = runs.join("errors.txt");
    out.say(format!("running, standard input closed, output to {} and errors to {}; it must write to one within {} s; timeout {} s", output.display(), errors.display(), a.start_wait.as_secs(), timeout.as_secs()));
    let seen =
        launch::run(argv, &ctx.root, &output, &errors, a.start_wait, timeout).map_err(|e| {
            Fail::refuse(
                "not-started",
                e,
                "the environment failed, not the change: retry once, or tell the person",
            )
        })?;
    // A role answers on its standard output. One that wrote only to the other still said something.
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    let raw = Some(read(&output))
        .filter(|o| !o.trim().is_empty())
        .unwrap_or_else(|| read(&errors));
    // A voided step still counts against the cap and still took the thread to its recorded reading.
    let previous = l
        .entries
        .iter()
        .rev()
        .filter_map(|e| e.event())
        .filter(|v| matches!(role, "build" | "checks") && step(v) == role)
        .map(|v| &v["report"]["codex_thread"]);
    let said = launch::read_round(adapter, &raw, model, previous);
    let tokens = said.tokens();
    if adapter == Adapter::Codex && tokens.is_none() {
        out.say("The token cost of this Codex round is unknown. The thread has no comparable reading for the cap, so tokens is null.");
    }
    if matches!(role, "build" | "checks")
        && let Some(session) = &said.session_id
    {
        out.say(format!(
            "session for {id} {role}: {session} (keep it for a fix round)"
        ));
        out.fact("session_id", session.as_str());
    }
    let journal = json!({"launch": {
        "ran": true, "role": role, "environment": env, "level": level, "model": model, "adapter": adapter.name(),
        "started": seen.started, "first_output": seen.first_output, "seconds": seen.seconds, "timed_out": seen.timed_out,
        "exit": seen.exit, "tokens": tokens, "usage": if said.usage.is_empty() { Value::Null } else { said.usage_json() },
    }});
    out.journal = Some(journal);
    out.fact("started", seen.started);
    out.fact("seconds", seen.seconds);
    out.fact("timed_out", seen.timed_out);
    out.fact("tokens", json!(tokens));
    if !seen.started {
        return Err(Fail::refuse(
            "not-started",
            format!(
                "the {role} wrote nothing within {} s{}: it didn't start, and it was stopped",
                a.start_wait.as_secs(),
                seen.exit
                    .map(|e| format!(" (exit {e})"))
                    .unwrap_or_default()
            ),
            "the environment failed, not the change: retry once, or tell the person",
        ));
    }
    if seen.timed_out {
        return Err(Fail::refuse(
            "timeout",
            format!(
                "the {role} ran past {} s and was stopped",
                timeout.as_secs()
            ),
            "the environment failed, not the change: retry once, or tell the person",
        ));
    }
    out.say(format!(
        "it ran {} s, first output after {} s, exit {}",
        seen.seconds,
        seen.first_output.unwrap_or(0.0),
        seen.exit
            .map(|e| e.to_string())
            .unwrap_or_else(|| "none".into())
    ));
    let ra = ReportArgs {
        model: Some(model.to_string()),
        level: Some(level.to_string()),
        seconds: Some(seen.seconds),
        tokens,
        tokens_read: said.tokens_read,
        codex_thread: said.codex_thread,
        ..ReportArgs::default()
    };
    // The role sealed or committed while it ran: its report is judged against the project as it
    // left it, not as it was when it was launched.
    let l = &load_attempt(ctx, id, role, out)?;
    let (line, _) = write_report(ctx, l, id, role, &said.text, &ra, out)?;
    out.fact("line", line);
    Ok(())
}

pub fn launch_cmd(ctx: &Ctx, id: &str, role: &str, a: &LaunchArgs) -> Out {
    run_change(ctx, id, "launch", |out| {
        if !["checks", "build", "review", "reading"].contains(&role) {
            return Err(Fail::usage("the role is checks, build, review or reading"));
        }
        let l = load_attempt(ctx, id, role, out)?;
        may_launch(ctx, &l, id, role)?;
        if a.run && a.command.is_none() && returned_role(&l.state.progress, role) {
            return Err(Fail::refuse(
                "resume-required",
                format!("the {role} round goes back to the session that did the work"),
                format!(
                    "resume it with gdd launch {id} {role} --run --command \"<resume command>\". If it cannot resume, get the prompt with gdd launch {id} {role}, launch by hand, then gdd report {id} {role} <file>"
                ),
            ));
        }
        let level = level_of(&l.bp, &l.state.progress, role, a.level.as_deref());
        let (env, model, launch_text) = model_for(&l.bp, &ctx.env, role, &level)?;
        let prompt = prompt_for(&l, id, role, &level);
        let template = a
            .command
            .clone()
            .or_else(|| launch_text.clone().filter(|t| launch::is_command(t)));
        out.say(format!("{role} for {id}: {env}, {level}, model {model}"));
        out.fact("role", role);
        out.fact("environment", json!(env));
        out.fact("level", json!(level));
        out.fact("model", json!(model));
        out.fact("prompt", json!(prompt));
        let Some(template) = template else {
            return say_by_hand(
                out,
                id,
                role,
                &model,
                &prompt,
                launch_text.as_deref(),
                a.run,
            );
        };
        let runs = runs_dir(&ctx.root, id, role, out)?;
        let prompt_file = runs.join("prompt.txt");
        crate::runs::write_new(&prompt_file, prompt.as_bytes())
            .map_err(|e| crate::out::io(&prompt_file, e))?;
        let argv = launch::argv(&template, &model, &prompt, &prompt_file.to_string_lossy())
            .map_err(Fail::usage)?;
        let shown: Vec<String> = argv
            .iter()
            .map(|w| {
                if w == &prompt {
                    "<prompt>".to_string()
                } else {
                    gates::quote(w)
                }
            })
            .collect();
        out.say(format!("command: {}", shown.join(" ")));
        out.say(format!("prompt: {}", prompt_file.display()));
        out.fact("argv", json!(shown));
        if !a.run {
            if returned_role(&l.state.progress, role) {
                out.say(format!("resume the session that did this {role} with this prompt on {model}. With its command line, use --run --command with that session's id. If you resume it by hand: gdd report {id} {role} <file>"));
            } else {
                out.say(format!("run it with --run, or launch it yourself and then: gdd report {id} {role} <file>"));
            }
            return Ok(());
        }
        let who = Launched {
            id,
            role,
            env: &env,
            level: &level,
            model: &model,
        };
        run_role(ctx, &l, &who, a, &argv, &runs, out)
    })
}
