//! The brief step: the color of a change, written before anyone builds.

use super::*;
use std::collections::{BTreeMap, HashMap, HashSet};

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
    Ok(())
}

/// Every product brief waits for the project-wide setup.
pub(super) fn pending_standing_checks(ctx: &Ctx) -> R<Option<state::Next>> {
    let (_, text) = read_backlog(ctx)?;
    Ok(
        (backlog::standing_checks(&backlog::items(&text)) != Some("done")).then(|| state::Next {
            step: "stopped".into(),
            blocked: Some("standing-checks".into()),
            say: "The standing checks come before any change.".into(),
            command: "gdd next standing-checks".into(),
        }),
    )
}

/// The setup items of the backlog a change would go ahead of, once changes can start: the own
/// gate of a zone it touches, or a check its color asks for and its files would run. They are
/// named and nothing is refused: whether the zone already has behavior to test, or the check
/// something to judge, is the orchestrator's to say.
pub(super) fn setup_ahead(
    ctx: &Ctx,
    bp: &Blueprint,
    change: &color::Colored,
    files: &[String],
    out: &mut Out,
) {
    let text = read_backlog(ctx).map(|(_, text)| text).unwrap_or_default();
    let items = backlog::items(&text);
    let mut ahead = vec![];
    // Before the standing checks hold no change starts, so there is nothing to go ahead of.
    let waiting = items
        .iter()
        .filter(|i| i.state != "done" && backlog::standing_checks(&items) == Some("done"));
    for item in waiting {
        let (id, state) = (&item.id, &item.state);
        let what = if let Some(check) = id.strip_prefix("standing-checks-") {
            let runs = |k: &crate::blueprint::Check| {
                k.when.is_empty()
                    || git::match_paths(&ctx.root, files, &k.when).map_or(true, |m| !m.is_empty())
            };
            (bp.color_asks(&change.color, check) && bp.checks().iter().any(|k| k.id == check && runs(k)))
                .then(|| {
                    format!(
                        "the check {check}, which {} asks for, isn't set up yet ({id}, {state}): it comes before a change that needs it, unless it still waits for something to judge",
                        change.color
                    )
                })
        } else {
            super::launching::gate_zone(id)
                .filter(|zone| change.zones.contains_key(*zone))
                .map(|zone| {
                    format!(
                        "zone {zone}'s own gate isn't set up yet ({id}, {state}): once the zone has behavior to test, it comes before any further change there"
                    )
                })
        };
        if let Some(what) = what {
            out.say(format!("{what}: gdd next {id}"));
            ahead.push(id.clone());
        }
    }
    out.fact("setup_ahead", json!(ahead));
}

struct Scenario<'a> {
    line: usize,
    title: &'a str,
    block: Vec<&'a str>,
    rule: Option<usize>,
    has_id: bool,
    whens: usize,
}

#[derive(Default)]
struct Feature<'a> {
    rules: BTreeMap<usize, (&'static str, &'a str)>,
    scenarios: Vec<Scenario<'a>>,
}

/// The rules and the scenarios of a feature file, read line by line. A scenario's block is its
/// tags, its heading and every line under it, without indentation, blank lines or comments.
fn feature(text: &str) -> Feature<'_> {
    let mut result = Feature::default();
    let mut current: Option<&mut Scenario> = None;
    let mut tags: Vec<&str> = Vec::new();
    let mut rule = None;
    let mut fence = None;
    for (i, line) in text.lines().map(str::trim).enumerate() {
        // A doc string is the text of its step: no line in it is a heading, a tag or a step.
        let mark = ["\"\"\"", "```"].into_iter().find(|m| line.starts_with(m));
        if fence.is_some() || mark.is_some() {
            if fence.is_none() {
                fence = mark;
            } else if fence == mark {
                fence = None;
            }
            if let Some(s) = current.as_mut() {
                s.block.push(line);
            }
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('@') {
            tags.push(line);
            continue;
        }
        let heading = line.split_once(':').map(|(key, _)| key);
        if matches!(heading, Some("Scenario" | "Scenario Outline" | "Example")) {
            let has_id = tags
                .iter()
                .flat_map(|tag| tag.split_whitespace())
                .filter_map(|tag| tag.strip_prefix("@id:"))
                .any(crate::valid_id);
            tags.push(line);
            result.scenarios.push(Scenario {
                line: i + 1,
                title: line,
                block: std::mem::take(&mut tags),
                rule,
                has_id,
                whens: 0,
            });
            current = result.scenarios.last_mut();
        } else if matches!(heading, Some("Feature" | "Rule")) {
            rule = None;
            // A rule with no text is no rule: there is nothing in it to approve.
            if heading == Some("Rule") && line != "Rule:" {
                let label = tags
                    .iter()
                    .flat_map(|t| t.split_whitespace())
                    .find_map(|t| match t {
                        "@assumed" => Some("yours"),
                        "@question" => Some("later"),
                        // What the change takes as true of something outside the product.
                        "@unverified" => Some("unchecked"),
                        _ => None,
                    })
                    .unwrap_or("rule");
                result.rules.insert(i + 1, (label, line));
                rule = Some(i + 1);
            }
            current = None;
        } else if let Some(s) = current.as_mut() {
            // Steps, tables and examples, with the tags of the examples.
            s.block.append(&mut tags);
            s.block.push(line);
            s.whens += usize::from(line.split_whitespace().next() == Some("When"));
        }
        tags.clear();
    }
    result
}

fn changed_feature<'a>(text: &'a str, original: &str) -> Feature<'a> {
    let old = feature(original);
    let mut copies = HashMap::new();
    for s in old.scenarios {
        *copies.entry(s.block).or_insert(0usize) += 1;
    }
    let mut changed = feature(text);
    changed.scenarios.retain(|s| {
        if let Some(count) = copies.get_mut(&s.block).filter(|n| **n > 0) {
            *count -= 1;
            return false;
        }
        true
    });
    let touched: HashSet<_> = changed.scenarios.iter().filter_map(|s| s.rule).collect();
    let old_rules: HashSet<_> = old.rules.values().map(|(_, heading)| *heading).collect();
    changed
        .rules
        .retain(|line, (_, heading)| touched.contains(line) || !old_rules.contains(heading));
    changed
}

fn check_form(ctx: &Ctx, base: &str, file: &str, out: &mut Out, faults: &mut Vec<String>) -> R<()> {
    // An old file can be in another encoding, and git gives the base the same way.
    let bytes =
        std::fs::read(ctx.root.join(file)).map_err(|e| crate::out::io(Path::new(file), e))?;
    let text = String::from_utf8_lossy(&bytes);
    let language = text
        .lines()
        .map(str::trim)
        .take_while(|line| line.is_empty() || line.starts_with('#'))
        .find_map(|line| line.strip_prefix("# language:"));
    if let Some(lang) = language.map(str::trim).filter(|lang| *lang != "en") {
        out.say(format!(
            "The layer cannot check the form of {file} because its language is {lang}."
        ));
        return Ok(());
    }
    let original = git::show(&ctx.root, base, file).unwrap_or_default();
    let changed = changed_feature(&text, &original);
    if changed.scenarios.is_empty() {
        out.say(format!("{file} has no new or changed scenarios."));
    }
    for s in changed.scenarios {
        let mut missing = Vec::new();
        if s.rule.is_none() {
            missing.push("Add a Rule: above this scenario.".into());
        }
        if !s.has_id {
            missing.push("Add a valid @id:<id> tag.".into());
        }
        if s.whens != 1 {
            missing.push(format!("Use exactly one When step (found {}).", s.whens));
        }
        if !missing.is_empty() {
            let reasons = missing.join(" ");
            faults.push(format!("{file}:{}: {}  {reasons}", s.line, s.title));
        }
    }
    Ok(())
}

/// Whether the person's yes to the brief is what the change waits for: the next step asks for it,
/// or its question is already open.
fn approval_is_due(l: &Loaded, id: &str) -> bool {
    let s = &l.state;
    s.next
        .command
        .starts_with(&format!("gdd ask {id} --of brief "))
        || s.next.step == "wait" && s.progress.open_ask().is_some_and(|a| a.of == "brief")
}

/// What a yes to the brief covers, from the sealed version the log names: a line for each rule the
/// change adds or changes, and where to read them. No word count goes with them: with one, agents
/// merged rules to fit it.
pub(super) fn show_approval(l: &Loaded, id: &str, out: &mut Out) {
    if !approval_is_due(l, id) {
        return;
    }
    let p = &l.state.progress;
    let Some(brief) = p.brief_line.and_then(|n| l.entries[n - 1].event()) else {
        return;
    };
    let (Some(seal), Some(files)) = (brief["seal"].as_str(), brief["files"].as_object()) else {
        return;
    };
    let root = &l.project.root;
    let base = p.base.as_deref();
    let mut lines = Vec::new();
    let mut found = false;
    for file in files.keys().filter(|f| f.ends_with(".feature")) {
        let Some(text) = git::show(root, seal, file) else {
            continue;
        };
        let original = base
            .and_then(|b| git::show(root, b, file))
            .unwrap_or_default();
        let changed = changed_feature(&text, &original);
        let mut headings = changed.rules;
        for s in changed.scenarios.into_iter().filter(|s| s.rule.is_none()) {
            headings.insert(s.line, ("no rule", s.title));
        }
        for (label, heading) in headings.into_values() {
            let title = heading.split_once(':').unwrap().1.trim();
            found = true;
            lines.push(format!("{label}: {title}"));
        }
        lines.push(format!("read: {file}"));
    }
    if let Some(line) = brief.get("closing").and_then(crate::closing::approval_line) {
        found = true;
        lines.push(line);
    }
    if !found {
        return;
    }
    out.say("Their yes covers these lines. Tell them all in their words and language, in whole sentences: what they will notice, then what you chose, marking each 'yours' as yours and grouping the technical ones by what they protect while naming each, and what closes it. Name each file, and end with one question they can answer with a yes.");
    if lines.iter().any(|line| line.starts_with("unchecked: ")) {
        out.say("Mark each 'unchecked' as something you took as true of the outside without seeing it.");
    }
    // What a reader who saw only the brief took the change to do goes first, and only of the
    // version they are asked to approve: an earlier reading says nothing of what came after it.
    let understood = log::events(&l.entries)
        .into_iter()
        .rev()
        .filter(|(line, v)| p.brief_line.is_some_and(|b| *line > b) && step(v) == "reading")
        .find_map(|(_, v)| v["understood"].as_str());
    if let Some(understood) = understood {
        out.say("Give 'understood' first: it is what a reader who saw only the brief took the change to do.");
        lines.insert(0, format!("understood: {understood}"));
    }
    out.lines.extend(lines);
}

/// The `brief` step: the planned color from the paths, the base, the files with their hashes and
/// the seal's tip, which must hold those same files.
pub fn brief(ctx: &Ctx, id: &str, a: &BriefArgs) -> Out {
    run_change(ctx, id, "brief", |out| {
        let l = load_attempt(ctx, id, "brief", out)?;
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
        if log_kind(id) == LogKind::Change
            && let Some(wait) = pending_standing_checks(ctx)?
        {
            return Err(Fail::refuse("standing-checks", wait.say, wait.command));
        }
        git_writes(ctx)?;
        let main = git::main_branch(&ctx.root);
        let seal_tip = l.state.git.seal_tip.clone();
        let base = match (&l.state.progress.base, &seal_tip) {
            (Some(b), _) => b.clone(),
            (None, Some(t)) => {
                git::merge_base(&ctx.root, t, &git::branch_ref(&main)).ok_or(Fail::missing(
                    format!("a commit both gdd-seal/{id} and {main} hold"),
                    format!("the seal and {main} share no commit"),
                ))?
            }
            (None, None) => {
                git::commit_of(&ctx.root, &git::branch_ref(&main)).ok_or(Fail::missing(
                    format!("refs/heads/{main}"),
                    format!("there is no {main} branch"),
                ))?
            }
        };
        let (sealed, paths): (Vec<_>, Vec<_>) = a.paths.iter().partition(|f| {
            l.state
                .progress
                .sealed
                .contains_key(f.trim_start_matches("./"))
        });
        let retry_commands = || {
            let files = a
                .files
                .iter()
                .chain(sealed.iter().copied())
                .map(|p| gates::quote(p))
                .collect::<Vec<_>>()
                .join(" ");
            let paths = if paths.is_empty() {
                "<path the change will touch>...".into()
            } else {
                paths
                    .iter()
                    .map(|p| gates::quote(p))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let mut command = format!("gdd brief {id} {files} --paths {paths}");
            for (flag, value) in [
                ("--color", a.color),
                ("--why", Some(a.why)),
                ("--level", a.level),
                ("--skill-version", a.skill_version),
                ("--request", a.request),
                ("--request-file", a.request_file),
            ] {
                if let Some(value) = value.filter(|v| !v.is_empty()) {
                    command.push_str(&format!(" {flag} {}", gates::quote(value)));
                }
            }
            (files, command)
        };
        if let Some(&f) = sealed.first() {
            return Err(Fail::refuse(
                "sealed-path",
                format!(
                    "{f} is sealed for {id}: it judges the change and is not one of its paths. What follows --paths counts as a path"
                ),
                retry_commands().1,
            ));
        }
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
        // A file the log already holds with this content stays valid: only what is new is checked.
        let logged = |f: &String| {
            let held = &l.state.progress.brief_files;
            let had = held.get(f).or_else(|| held.get(f.trim_start_matches("./")));
            git::hash_object(&ctx.root, f).ok().as_ref() == had
        };
        let mut faults = Vec::new();
        for file in a
            .files
            .iter()
            .filter(|f| f.ends_with(".feature") && !logged(f))
        {
            check_form(ctx, &base, file, out, &mut faults)?;
        }
        if !faults.is_empty() {
            let (files, retry) = retry_commands();
            return Err(Fail::refuse(
                "scenario-form",
                format!("Some scenarios need a correction.\n{}", faults.join("\n")),
                format!(
                    "Fix the listed scenarios and run these commands.\ngdd seal {id} {files}\n{retry}"
                ),
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
        // Above green, a change with no brief has no scenarios, so nobody would write its checks.
        if c.color != "green" && files.is_empty() && !l.state.progress.has_brief() {
            let named = "<the brief and its scenarios>";
            let retry = retry_commands().1.replacen(
                &format!("gdd brief {id} "),
                &format!("gdd brief {id} {named}"),
                1,
            );
            return Err(Fail::refuse(
                "no-brief",
                format!(
                    "{id} is {}: a change above green needs its brief and its scenarios, written and sealed before this step",
                    c.color
                ),
                format!(
                    "Write gdd/briefs/{id}.md and its scenarios as gdd-brief says, then run these commands.\ngdd seal {id} {named}\n{retry}"
                ),
            ));
        }
        let closing = match closing_of(ctx, &l, id, a.files, logged) {
            Ok(c) => c,
            Err(why) => {
                let brief = format!("gdd/briefs/{id}.md");
                let (mut files, mut retry) = retry_commands();
                if !a.files.iter().any(|f| f.trim_start_matches("./") == brief) {
                    files = format!("{brief} {files}");
                    retry = retry.replacen(
                        &format!("gdd brief {id} "),
                        &format!("gdd brief {id} {brief} "),
                        1,
                    );
                }
                return Err(Fail::refuse(
                    "closing-form",
                    why,
                    format!(
                        "Add to {brief}, in the person's words:\n{}\nThen run these commands.\ngdd seal {id} {files}\n{retry}",
                        crate::closing::FORM
                    ),
                ));
            }
        };
        // Where no approval is due, the person hears what will close it all the same.
        let closes = closing
            .as_ref()
            .filter(|_| !l.bp.color_asks(&c.color, "brief-approval"))
            .and_then(crate::closing::approval_line);
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
        // A step that names no file keeps the brief the change has: writing none would drop its
        // scenarios, and with them the checks step.
        let kept_files = if files.is_empty() {
            let held = &l.state.progress.brief_files;
            held.iter().map(|(f, h)| (f.clone(), json!(h))).collect()
        } else {
            files.clone()
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
                ("files", Value::Object(kept_files)),
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
        if let Some(c) = closing {
            ev["closing"] = c;
        }
        // What it takes as true of the outside without anyone having checked it stays in the
        // step: the commit of the seal may be gone by the time the change is closed.
        let unchecked: Vec<String> = if files.is_empty() {
            l.state.progress.unchecked.clone()
        } else {
            a.files
                .iter()
                .filter(|f| f.ends_with(".feature"))
                .flat_map(|file| {
                    let text = std::fs::read_to_string(ctx.root.join(file)).unwrap_or_default();
                    let original = git::show(&ctx.root, &base, file).unwrap_or_default();
                    changed_feature(&text, &original)
                        .rules
                        .into_values()
                        .filter(|(label, _)| *label == "unchecked")
                        .filter_map(|(_, heading)| heading.split_once(':'))
                        .map(|(_, title)| title.trim().to_string())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        if !unchecked.is_empty() {
            ev["unchecked"] = json!(unchecked);
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
        // Said once, when the change starts: a later version of its brief starts nothing.
        if log_kind(id) == LogKind::Change && l.state.progress.brief_line.is_none() {
            let files: Vec<String> = color::planned_paths(&ctx.root, a.paths)
                .map(|stands| stands.into_iter().flat_map(|(_, files)| files).collect())
                .unwrap_or_default();
            setup_ahead(ctx, &l.bp, &c, &files, out);
        }
        if let Some(closes) = closes {
            out.say(format!(
                "Tell the person what will close it, in a line: {closes}"
            ));
        }
        out.fact("color", json!(c.color));
        out.fact("line", line);
        Ok(())
    })
}

/// How the change closes, from its brief file as sealed. A brief the log already holds with this
/// content was written before the layer asked for it, and keeps what it had, and so does a brief
/// step that names no brief file. A first brief with files and without the brief is refused:
/// nothing would say how a change that promised something closes.
fn closing_of(
    ctx: &Ctx,
    l: &Loaded,
    id: &str,
    files: &[String],
    logged: impl Fn(&String) -> bool,
) -> Result<Option<Value>, String> {
    let brief = format!("gdd/briefs/{id}.md");
    let kept = l.state.progress.closing.clone();
    let Some(f) = files.iter().find(|f| f.trim_start_matches("./") == brief) else {
        // A brief logged with files and no closing was written before the layer asked for one.
        if files.is_empty() || kept.is_some() || l.state.progress.has_brief() {
            return Ok(kept);
        }
        return Err(format!(
            "the brief step names no {brief}, which says how the change closes"
        ));
    };
    let text = std::fs::read_to_string(ctx.root.join(f)).unwrap_or_default();
    match crate::closing::read(&text) {
        Ok(c) => Ok(Some(c)),
        Err(_) if logged(f) => Ok(kept),
        Err(e) => Err(format!("{f} doesn't say how the change closes: {e}")),
    }
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
