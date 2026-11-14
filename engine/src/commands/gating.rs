//! The gates: the checks a change's color asks for, run on one commit.

use super::*;

/// The private folder of one run's outputs (see [`crate::runs`]); when the usual one can't be
/// trusted, it says so and uses a fresh one.
pub(super) fn runs_dir(root: &Path, id: &str, what: &str, out: &mut Out) -> R<PathBuf> {
    let stamp = time::now_string().replace(':', "");
    let (dir, note) = crate::runs::for_run(root, id, what, &stamp)
        .map_err(|e| Fail::missing("a private folder for the outputs", e.to_string()))?;
    if let Some(n) = note {
        out.say(n);
    }
    Ok(dir)
}

/// The tip of `gdd-seal/<id>` as the remotes hold it, and the ref's name, when every remote that
/// has it agrees on one commit.
fn remote_seal(root: &Path, id: &str) -> Option<(String, String)> {
    let listed = git::out(
        root,
        &[
            "for-each-ref",
            "--format=%(objectname) %(refname:short)",
            "refs/remotes/",
        ],
    )
    .ok()?;
    let suffix = format!("/gdd-seal/{id}");
    let found: Vec<(&str, &str)> = listed
        .lines()
        .filter_map(|l| l.split_once(' '))
        .filter(|(_, name)| name.ends_with(&suffix))
        .collect();
    let (tip, name) = found.first()?;
    found
        .iter()
        .all(|(t, _)| t == tip)
        .then(|| (tip.to_string(), name.to_string()))
}

/// The `<start>` gates judge against, from git and not from the log: the tip of `gdd-seal/<id>`
/// (a remote's, when there is no local one), or, with nothing sealed, the commit of the main
/// branch the head was last brought up to (the
/// base, or a later one). The log's `<start>` must agree with it, else nothing is judged: a log
/// line can say anything, and a `<start>` that holds checks someone loosened passes the seal check.
fn judged_start(ctx: &Ctx, l: &Loaded, head: &str, out: &mut Out) -> R<String> {
    let id = &l.state.id;
    let p = &l.state.progress;
    let logged = p.start.clone().ok_or(Fail::refuse(
        "out-of-order",
        "no brief step: no <start>",
        way_on(l, id),
    ))?;
    let main = git::main_branch(&ctx.root);
    // In CI there is often no local gdd-seal/<id>: the platform's, as its remote holds it, is the one.
    let seal = l
        .state
        .git
        .seal_tip
        .clone()
        .map(|t| (t, format!("gdd-seal/{id}")))
        .or_else(|| remote_seal(&ctx.root, id));
    if let Some((tip, name)) = &seal {
        if tip != &logged {
            return Err(Fail::refuse(
                "seal-moved",
                format!(
                    "{name} is at {}, and the log's <start> is {}: the seal moved where the layer didn't record it, so nothing judges from it",
                    git::short(tip),
                    git::short(&logged)
                ),
                format!(
                    "the seal moves only through gdd seal {id} and the checks role's report (gdd report {id} checks); if nobody you launched moved it, tell the person"
                ),
            ));
        }
        return Ok(tip.clone());
    }
    if p.sealed_any || !p.sealed.is_empty() {
        return Err(Fail::refuse(
            "seal-missing",
            format!(
                "the log says gdd-seal/{id} is at {}, and git has no gdd-seal/{id}, nor one its remotes agree on: the gates judge from the seal, never from the log alone",
                git::short(&logged)
            ),
            format!(
                "fetch it as a local branch (git fetch <remote> gdd-seal/{id}:gdd-seal/{id}), then gdd gates {id}"
            ),
        ));
    }
    let mb = git::merge_base(&ctx.root, head, &main).ok_or(Fail::missing(
        format!("a commit both {} and {main} hold", git::short(head)),
        format!("{} shares no commit with {main}", git::short(head)),
    ))?;
    if mb != logged && !git::is_ancestor(&ctx.root, &logged, &mb) {
        return Err(Fail::refuse(
            "start-mismatch",
            format!(
                "nothing is sealed, so <start> is where {} meets {main}, {}; the log says {}, which is not there nor before it",
                git::short(head),
                git::short(&mb),
                git::short(&logged)
            ),
            format!("tell the person: the log of {id} doesn't match git"),
        ));
    }
    if mb != logged {
        out.say(format!(
            "nothing sealed and {main} merged in: <start> is {}",
            git::short(&mb)
        ));
    }
    Ok(mb)
}

pub fn gates_cmd(ctx: &Ctx, id: &str, at: Option<&Path>) -> Out {
    run("gates", |out| {
        let l = load(ctx, id)?;
        need_repo(ctx, "the gates judge a commit")?;
        if at.is_none() {
            git_writes(ctx)?;
        }
        // The commit it would judge goes into what the state is asked: gates that failed don't
        // run again on another commit before the round that moved the branch is reported.
        allowed(&l, id, "gates", l.state.git.head.as_deref())?;
        let head = l.state.git.head.clone().ok_or(Fail::missing(
            format!("the branch of {id}"),
            format!("no branch for {id} (work/{id}, gdd-build/{id} or build/{id}): pass --branch or --head"),
        ))?;
        if !l.state.limits.stop.is_empty() {
            return Err(Fail::refuse(
                "cap-spent",
                format!(
                    "{} no room left, and no finished build waits to be judged",
                    l.state.limits.stop.join(", ")
                ),
                cap_way_on(id, &l.state.limits),
            ));
        }
        if l.state.limits.spent {
            out.say("a cap has no room left, but a finished build is judged all the same: no new round starts after it");
        }
        let start = judged_start(ctx, &l, &head, out)?;
        if !git::is_ancestor(&ctx.root, &start, &head) {
            return Err(Fail::refuse(
                "wrong-start",
                format!(
                    "{} doesn't start from <start> {}",
                    git::short(&head),
                    git::short(&start)
                ),
                "merge the seal's tip into the change's branch first",
            ));
        }
        let (plan, commands) = plan_for(ctx, &l, id, &start, &head)?;
        let outputs = runs_dir(&ctx.root, id, "gates", out)?;
        let (dir, place, _worktree) = place_for(ctx, at, &head)?;
        // A fresh copy has only what git holds: the blueprint's `install` gives it what the checks
        // need. Whatever comes of it, the worktree is removed once, here.
        let fresh = at.is_none();
        let install = l.bp.install().filter(|_| fresh);
        let judged = (|| -> R<_> {
            let installed = match &install {
                Some(cmd) => Some(install_in(&dir, &outputs, cmd, id, &head, out)?),
                None => None,
            };
            let (ran, skipped) = run_checks(&dir, &outputs, &plan, &commands, out);
            Ok((installed, ran, skipped))
        })();
        if fresh {
            let _ = git::worktree_remove(&ctx.root, &dir);
        }
        let (installed, ran, skipped) = judged?;
        // "Command not found" in a fresh copy nobody prepared is the copy lacking what the check
        // needs, not the change failing: nothing is written, and no round is counted.
        if fresh
            && install.is_none()
            && let Some(r) = ran
                .iter()
                .find(|r| r.check != "seal" && r.exit == Some(127))
        {
            return Err(Fail::refuse(
                "environment",
                format!(
                    "the check {} ended with exit 127, command not found: the fresh copy lacks what it needs, such as the project's dependencies. It judged nothing, and nothing was written",
                    r.check
                ),
                format!(
                    "run the gates where the dependencies are installed: gdd gates {id} --at <a directory with {} checked out, and clean>. For good, the blueprint names the command that installs them (install: <command>), a change to the blueprint the person approves",
                    git::short(&head)
                ),
            ));
        }
        let passed = ran.iter().all(|r| r.passed);
        let mut m = obj(vec![
            ("step", json!("gates")),
            ("start", json!(start)),
            ("where", json!(place)),
            ("head", json!(head)),
            (
                "gates",
                json!(ran.iter().map(|r| r.to_json()).collect::<Vec<_>>()),
            ),
        ]);
        if !skipped.is_empty() {
            m.insert("skipped".into(), json!(skipped));
        }
        if let Some(i) = installed {
            m.insert("install".into(), i);
        }
        let ev = new_event(id, "step", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "judged {} from {} in {place}; wrote the gates step, line {line}; outputs in {}",
            git::short(&head),
            git::short(&start),
            outputs.display()
        ));
        let after = after(ctx, &l, id);
        let lim_after = &after.limits;
        if !lim_after.slow.is_empty() {
            out.say(format!(
                "past its seconds, for the backlog: {}",
                lim_after.slow.join(", ")
            ));
        }
        say_next(out, &after.next);
        out.fact("head", json!(head));
        out.fact("start", json!(start));
        out.fact("passed", passed);
        out.fact("line", line);
        out.fact("checks", json!(ran.iter().map(|r| json!({"check": r.check, "result": if r.passed {"pass"} else {"fail"}, "seconds": r.seconds, "exit": r.exit})).collect::<Vec<_>>()));
        out.fact("next", json!(after.next.step));
        if !passed {
            out.code = 1;
        }
        Ok(())
    })
}

/// The checks the change's color asks for on what it touched, in the order they run, and the
/// command of each one filled with what the layer knows: `<start>`, `<base>` and the sealed paths.
/// A placeholder left over, or a check whose tier is misspelled, is refused before anything runs.
fn plan_for(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    start: &str,
    head: &str,
) -> R<(
    Vec<gates::Planned>,
    std::collections::BTreeMap<String, String>,
)> {
    let color = l
        .state
        .progress
        .color
        .clone()
        .unwrap_or_else(|| "yellow".into());
    let touched = changes(ctx, start, head)?;
    let touched_by = |specs: &[String]| {
        git::match_paths(&ctx.root, &touched, specs)
            .map(|m| !m.is_empty())
            .unwrap_or(true)
    };
    // The standing gates of every zone it touches.
    let mut zone_checks = vec![];
    let (hit, _) = color::sort_paths(&ctx.root, &l.bp, &touched)
        .map_err(|e| Fail::missing("the zones of the paths it touched", e))?;
    for z in l.bp.zones() {
        if let (true, Some(file)) = (hit.contains_key(&z.id), &z.gates)
            && let Some(text) = git::show(&ctx.root, start, file)
        {
            zone_checks.extend(gates::zone_gate_checks(&text));
        }
    }
    let plan = gates::plan(&l.bp, &color, &touched_by, &zone_checks);
    // A check whose `runs` is neither tier would be left out without a word, and the gates
    // could pass without it: a slip of the pen in the blueprint is no way to drop a check.
    if let Some(p) = plan
        .iter()
        .find(|p| p.check != "seal" && p.tier != "each round" && p.tier != "before review")
    {
        return Err(Fail::refuse(
            "bad-blueprint",
            format!(
                "the check {} runs {:?} in the blueprint, which is neither `each round` nor `before review`: as written it would never run",
                p.check, p.tier
            ),
            format!(
                "write it right in gdd/blueprint.md, which the person approves; then gdd gates {id}"
            ),
        ));
    }
    let sealed: Vec<String> = l.state.progress.sealed.keys().cloned().collect();
    // Every check that runs gets what the layer knows in its command: <start> and <base>,
    // hashes git resolved, and the sealed paths. A placeholder left over is refused before
    // anything runs: as written, it would fail on every change, or judge nothing.
    let base = l
        .state
        .progress
        .base
        .clone()
        .filter(|b| log::is_hash(b) && git::commit_of(&ctx.root, b).is_some());
    let fill = gates::Fill {
        start,
        base: base.as_deref(),
        sealed: &sealed,
    };
    let mut commands: std::collections::BTreeMap<String, String> = Default::default();
    for p in plan.iter().filter(|p| p.skipped.is_none()) {
        let raw = if p.check == "seal" {
            gates::seal_command(&l.bp, start, &sealed)
        } else {
            p.command.clone()
        };
        let filled = gates::fill(&raw, &fill).map_err(|held| {
            Fail::refuse(
                "unfilled",
                format!(
                    "the command of the check {} holds {held}, which the layer doesn't fill (it fills <start>, <base> and <sealed paths>): run as written, it would judge nothing",
                    p.check
                ),
                format!(
                    "write the check's real command in gdd/blueprint.md, from the constitution, and have the person approve the blueprint; then gdd gates {id}"
                ),
            )
        })?;
        commands.insert(p.check.clone(), filled);
    }
    Ok((plan, commands))
}

/// Where the gates run: a directory checked to hold exactly that commit, or a fresh worktree of
/// it, which the caller removes. Returns the directory, what the log calls it and the worktree's
/// temporary folder, kept alive while the gates run.
fn place_for(
    ctx: &Ctx,
    at: Option<&Path>,
    head: &str,
) -> R<(PathBuf, &'static str, Option<tempfile::TempDir>)> {
    match at {
        Some(d) => {
            git::clean_at(d, head).map_err(|e| {
                Fail::refuse(
                    "wrong-commit",
                    format!("{}: {e}", d.display()),
                    "check out the change's head in a clean directory, or leave out --at",
                )
            })?;
            let place = if std::env::var("CI").is_ok() {
                "ci"
            } else {
                "a checked directory"
            };
            Ok((d.to_path_buf(), place, None))
        }
        None => {
            let tmp = tempfile::Builder::new()
                .prefix("gdd-gates-")
                .tempdir()
                .map_err(|e| Fail::missing("a temporary folder for the gates", e.to_string()))?;
            let d = tmp.path().join("tree");
            git::worktree_add(&ctx.root, &d, head)
                .map_err(|e| Fail::missing(format!("a worktree of {head}"), e))?;
            Ok((d, "a fresh worktree", Some(tmp)))
        }
    }
}

/// Give a fresh copy what the checks need, with the blueprint's `install`. If it fails, the copy
/// can't judge anything, and that isn't the change failing: nothing is written, no round counts.
fn install_in(
    dir: &Path,
    outputs: &Path,
    cmd: &str,
    id: &str,
    head: &str,
    out: &mut Out,
) -> R<Value> {
    let r = gates::run_check(dir, "install", cmd, &outputs.join("install.out"));
    out.say(format!(
        "{:<10} {} exit {} in {} s",
        "install",
        if r.passed { "done" } else { "FAILED" },
        r.exit
            .map(|e| e.to_string())
            .unwrap_or_else(|| "none".into()),
        r.seconds.unwrap_or(0.0),
    ));
    if !r.passed {
        return Err(Fail::refuse(
            "environment",
            format!(
                "the fresh copy couldn't be prepared: the blueprint's install, `{cmd}`, ended with exit {}: {}. No check ran and nothing was written: its output is in {}",
                r.exit
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "none".into()),
                r.tail.lines().last().unwrap_or(""),
                outputs.join("install.out").display()
            ),
            format!(
                "if it is the environment (no network, a registry that is down), try once more, or run the gates where the dependencies are installed: gdd gates {id} --at <a directory with {} checked out, and clean>. If the change broke it, it is the builder's to fix: tell the person",
                git::short(head)
            ),
        ));
    }
    Ok(json!({"command": cmd, "seconds": r.seconds, "exit": r.exit}))
}

/// Run the planned checks in their order: the seal, then each round's, then the ones before the
/// review, which wait for the rest to pass. A seal that fails stops everything: what judges the
/// change was touched. Returns what ran and what was left out, with why.
fn run_checks(
    dir: &Path,
    outputs: &Path,
    plan: &[gates::Planned],
    commands: &std::collections::BTreeMap<String, String>,
    out: &mut Out,
) -> (Vec<gates::Ran>, Vec<Value>) {
    let mut ran: Vec<gates::Ran> = vec![];
    let mut skipped = vec![];
    let mut stop_rest = false;
    for tier in ["seal", "each round", "before review"] {
        if stop_rest {
            break;
        }
        for p in plan.iter().filter(|p| {
            if tier == "seal" {
                p.check == "seal"
            } else {
                p.check != "seal" && p.tier == tier
            }
        }) {
            if let Some(why) = &p.skipped {
                skipped.push(json!({"check": p.check, "why": why}));
                out.say(format!("{:<10} skipped: {why}", p.check));
                continue;
            }
            if tier == "before review" && ran.iter().any(|r| !r.passed) {
                break;
            }
            let command = commands.get(&p.check).cloned().unwrap_or_default();
            let r = gates::run_check(
                dir,
                &p.check,
                &command,
                &outputs.join(format!("{}.out", p.check)),
            );
            out.say(format!(
                "{:<10} {} exit {} in {} s{}",
                r.check,
                if r.passed { "pass" } else { "FAIL" },
                r.exit
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "none".into()),
                r.seconds.unwrap_or(0.0),
                if r.passed || r.tail.is_empty() {
                    String::new()
                } else {
                    format!(": {}", r.tail.lines().last().unwrap_or(""))
                }
            ));
            let failed_seal = r.check == "seal" && !r.passed;
            ran.push(r);
            if failed_seal {
                out.say(
                    "the seal failed: what judges the change was touched, so nothing else counts",
                );
                stop_rest = true;
                break;
            }
        }
        if tier == "each round" && ran.iter().any(|r| !r.passed) {
            break;
        }
    }
    (ran, skipped)
}
