//! Launching a role: its model, its prompt and, when the blueprint gives a command, its run.

use super::*;

/// The environment, the model and the launch of a role, from the blueprint's `levels`.
pub(super) fn model_for(
    l: &Loaded,
    env: &Option<String>,
    role: &str,
    level: &str,
) -> R<(String, String, Option<String>)> {
    let envs = l.bp.environments();
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
            "the standing checks the blueprint's `checks` name, from the blueprint and the constitution".into(),
            "the new standing checks".into(),
        )
    }
}

/// Whether the project has code yet: a tracked file that is not one of its GDD files, its
/// scenarios, its agents' files, its CI, its documents or its data.
fn has_code(root: &Path) -> bool {
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

fn prompt_for(l: &Loaded, id: &str, role: &str, level: &str) -> String {
    let p = &l.state.progress;
    let mut s = vec![];
    let setup = log_kind(id) == LogKind::Setup;
    let root = std::fs::canonicalize(&l.project.root).unwrap_or_else(|_| l.project.root.clone());
    let log_path = root.join(Project::log_rel(id));
    // A role loads its skill from another folder, and one was seen starting its work there instead
    // of in the project: say where the project is.
    let at = format!("The project is the folder {}.", root.display());
    if setup {
        s.push(format!(
            "Load {} and do its work for the project's {id} of this GDD project (contract gdd/1), and nothing else.",
            launch::skill_ref(role)
        ));
        s.push(at);
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
                // A fix round after a trial that sent the checks back or said stop: what it found,
                // and what the person answered when they were told.
                let events = log::events(&l.entries);
                if let Some(r) = &p.last_review
                    && matches!(r.verdict.as_str(), "fix" | "stop")
                    && let Some((_, v)) = events.iter().find(|(line, _)| *line == r.line)
                {
                    s.push(format!(
                        "A fix round. The trial said {}:",
                        r.verdict
                    ));
                    for f in v["findings"].as_array().cloned().unwrap_or_default() {
                        s.push(format!(
                            "- {} {}: {}",
                            str_of(&f, "id"),
                            str_of(&f, "where"),
                            str_of(&f, "what")
                        ));
                    }
                    if let Some((_, a)) = events
                        .iter()
                        .rev()
                        .find(|(line, v)| *line > r.line && log::kind(v) == "answer")
                    {
                        s.push(format!(
                            "The person's answer, word for word: \"{}\"",
                            str_of(a, "what")
                        ));
                    }
                }
            }
            "review" => s.push(format!(
                "Try {tried} on gdd-seal/{id}{}, in a fresh clone: follow references/standing-checks.md of the skill. Each check three ways, a round each: it rejects a planted bad case, it accepts without it, and a change can't switch it off. When the blueprint names its gates, read each gate's feature against the blueprint too. Its log is {}.",
                seal.map(|c| format!(", at {c}")).unwrap_or_default(),
                log_path.display()
            )),
            _ => {}
        }
        if id == "standing-checks" && (role == "checks" || role == "review") && !has_code(&root) {
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
    } else {
        s.push(format!(
            "Load {} and do its work for the change {id} of this GDD project (contract gdd/1), and nothing else.",
            launch::skill_ref(role)
        ));
        s.push(at);
        if role == "reading" {
            s.push("You are to read a brief before anyone builds it: follow references/brief.md of the skill.".into());
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
            "checks" => s.push(format!("Seal what you write on gdd-seal/{id}, on top of its tip.")),
            "build" => s.push(format!(
                "Work on the branch {branch}, which starts at <start>, in a worktree of your own inside the project folder, and commit there."
            )),
            "review" => s.push(format!(
                "Review the change at {}, in a fresh clone. Its log is {}.",
                l.state.git.head.clone().unwrap_or_else(|| format!("the tip of {branch}")),
                log_path.display()
            )),
            _ => {}
        }
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
    if role == "build" && round > 1 {
        let entries = log::events(&l.entries);
        if let Some(g) = entries.iter().rev().find(|(_, v)| step(v) == "gates")
            && log::any_kv(g.1, "result", "fail")
        {
            s.push("A fix round. What failed at the gates:".into());
            for e in g.1["gates"].as_array().cloned().unwrap_or_default() {
                if str_of(&e, "result") == "fail" {
                    s.push(format!(
                        "- {}: `{}` exit {}; last lines:\n{}",
                        str_of(&e, "check"),
                        str_of(&e, "command"),
                        e["exit"],
                        str_of(&e, "tail")
                    ));
                }
            }
        }
        if let Some(r) = entries.iter().rev().find(|(_, v)| step(v) == "review")
            && matches!(str_of(r.1, "verdict"), "fix" | "stop")
            && p.last_gates.as_ref().is_none_or(|g| g.line < r.0)
        {
            s.push(format!(
                "A fix round. The reviewer said {}:",
                str_of(r.1, "verdict")
            ));
            for f in r.1["findings"].as_array().cloned().unwrap_or_default() {
                s.push(format!(
                    "- {} {}: {}",
                    str_of(&f, "id"),
                    str_of(&f, "where"),
                    str_of(&f, "what")
                ));
            }
            if let Some(n) = r.1["notes"].as_str() {
                s.push(format!("NOTES FOR THE BUILDER:\n{n}"));
            }
        }
    }
    // What the log can show of a role's own runs comes from a json block, like the checks
    // role's proofs: without it, a claim or a trial is only prose.
    match role {
        "build" => s.push(
            "Before the gdd-report block, give a json block with what each check gave when you ran it: [{\"check\":\"<check id>\",\"command\":\"<command>\",\"exit\":0,\"tail\":\"<its last lines>\"}]."
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

pub fn launch_cmd(ctx: &Ctx, id: &str, role: &str, a: &LaunchArgs) -> Out {
    run("launch", |out| {
        if !["checks", "build", "review", "reading"].contains(&role) {
            return Err(Fail::usage("the role is checks, build, review or reading"));
        }
        let l = load(ctx, id)?;
        let p = &l.state.progress;
        // What the state allows: the same rules as the step this launch will write.
        if role != "reading" {
            allowed(&l, id, role, None)?;
            if role == "review"
                && let Some(g) = p.last_gates.as_ref()
                && let (Some(gh), Some(h)) = (&g.head, &l.state.git.head)
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
        } else if p.brief_line.is_some() && p.last_build.is_some() {
            return Err(Fail::refuse(
                "out-of-order",
                "a brief is read before anyone builds it",
                way_on(&l, id),
            ));
        }
        if (role == "checks" || role == "build") && !l.state.limits.stop.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "no new round: {} no room left",
                    l.state.limits.stop.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
        if role == "build" && l.state.next.step != "build" {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "the next step is {}, not a build: {}",
                    l.state.next.step, l.state.next.say
                ),
                way_on(&l, id),
            ));
        }
        if role == "checks" || role == "build" {
            git_writes(ctx)?;
            git_commits(ctx)?;
        }
        let level = level_of(&l, role, a.level.as_deref());
        let (env, model, launch_text) = model_for(&l, &ctx.env, role, &level)?;
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
            out.say(format!(
                "launch it the way the blueprint says: {}",
                launch_text.clone().unwrap_or_else(|| "(no launch)".into())
            ));
            out.say(format!("with the model {model}, and this prompt:"));
            for line in prompt.lines() {
                out.say(format!("  {line}"));
            }
            out.say(format!(
                "then: gdd report {id} {role} <its last message, a file or ->"
            ));
            if a.run {
                return Err(Fail::refuse(
                    "not-a-command",
                    "the blueprint's launch is not one command with <model>: the layer can't run it",
                    "launch it yourself as printed, or pass --command \"<program> ... <model> ... <prompt>\"",
                ));
            }
            return Ok(());
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
            out.say(format!(
                "run it with --run, or launch it yourself and then: gdd report {id} {role} <file>"
            ));
            return Ok(());
        }
        let adapter = match &a.adapter {
            Some(n) => {
                Adapter::by_name(n).ok_or(Fail::usage("--adapter is claude, codex or plain"))?
            }
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
        out.say(format!("running, standard input closed, output to {}; it must show output within {} s; timeout {} s", output.display(), a.start_wait.as_secs(), timeout.as_secs()));
        let seen = launch::run(&argv, &ctx.root, &output, a.start_wait, timeout).map_err(|e| {
            Fail::refuse(
                "not-started",
                e,
                "the environment failed, not the change: retry once, or tell the person",
            )
        })?;
        let raw = std::fs::read_to_string(&output).unwrap_or_default();
        let said = launch::read_output(adapter, &raw, &model);
        let journal = json!({"launch": {
            "ran": true, "role": role, "environment": env, "level": level, "model": model, "adapter": adapter.name(),
            "started": seen.started, "first_output": seen.first_output, "seconds": seen.seconds, "timed_out": seen.timed_out,
            "exit": seen.exit, "tokens": said.tokens(), "usage": if said.usage.is_empty() { Value::Null } else { said.usage_json() },
        }});
        out.journal = Some(journal);
        out.fact("started", seen.started);
        out.fact("seconds", seen.seconds);
        out.fact("timed_out", seen.timed_out);
        out.fact("tokens", json!(said.tokens()));
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
            model: Some(model.clone()),
            level: Some(level.clone()),
            seconds: Some(seen.seconds),
            tokens: said.tokens(),
            ..ReportArgs::default()
        };
        let (line, _) = write_report(ctx, &l, id, role, &said.text, &ra, out)?;
        out.fact("line", line);
        Ok(())
    })
}
