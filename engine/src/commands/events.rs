//! The events a person or the orchestrator gives: an ask, an answer, an approval, a waiver, a correction.

use super::*;

/// The command that writes an event of the contract, and what to call that event. Every event
/// and every step of the contract has one, which checks what it records.
fn command_for(id: &str, ev: &str, st: &str) -> Option<(String, String)> {
    let (what, command) = match (ev, st) {
        ("approval", _) => (
            "an approval".to_string(),
            format!(
                "gdd approve {id} --of <kind> --version <n> --who <person> --where <where> <file>..."
            ),
        ),
        ("waiver", _) => (
            "a waiver".into(),
            format!(
                "gdd waiver {id} --who <person> --check <check or limit> --why \"<why>\" --backlog <item>"
            ),
        ),
        ("ask", _) => (
            "a question to the person".into(),
            format!("gdd ask {id} --of <kind> \"<the question>\""),
        ),
        ("answer", _) => (
            "an answer".into(),
            format!("gdd answer {id} \"<their words>\""),
        ),
        ("correction", _) => (
            "a correction".into(),
            format!("gdd correct {id} --line <n> \"<what was wrong>\""),
        ),
        ("seal", _) => (
            "a move of the seal".into(),
            format!("gdd seal {id} <file>..., or gdd seal {id} --merge-main"),
        ),
        ("step", "brief") => (
            "a brief step".into(),
            format!(
                "gdd brief {id} --paths <path>... [<files>], or to drop the change: gdd brief {id} --stop \"<why>\""
            ),
        ),
        ("step", "checks" | "build" | "review" | "reading") => (
            format!("a {st} step"),
            format!(
                "gdd report {id} {st} <the role's last message>, or gdd launch {id} {st} --run"
            ),
        ),
        ("step", "gates") => ("a gates step".into(), format!("gdd gates {id}")),
        ("step", "ship") => (
            "a ship step".into(),
            format!("gdd ship {id} --merge, or gdd ship {id} --done <where>"),
        ),
        ("step", "prepare" | "harden" | "exam") => (
            format!("a {st} step"),
            format!("gdd step blueprint {st} ..."),
        ),
        _ => return None,
    };
    Some((what, command))
}

/// `gdd log` writes nothing. Every event of the contract has a command of its own, which checks
/// what it records, so an event an agent composes is refused, with the command that writes it.
/// An event composed by hand could move `<start>` or lower a color where no command would.
pub fn log_event(ctx: &Ctx, id: &str, json_text: &str) -> Out {
    run("log", |_out| {
        check_id(id)?;
        let v: Value = serde_json::from_str(json_text)
            .map_err(|e| Fail::usage(format!("the event isn't JSON: {e}")))?;
        let Value::Object(m) = v else {
            return Err(Fail::usage("the event isn't a JSON object"));
        };
        let ev = m.get("event").and_then(Value::as_str).unwrap_or("");
        let st = m.get("step").and_then(Value::as_str).unwrap_or("");
        // A hash composed by hand is checked first, so the refusal says what was wrong with it too.
        if let Some(Value::Object(files)) = m.get("files") {
            for (p, h) in files {
                let now = git::hash_object(&ctx.root, p).map_err(|_| {
                    Fail::missing(p.to_string(), format!("{p} is not in the working copy"))
                })?;
                if h.as_str() != Some(now.as_str()) {
                    return Err(Fail::refuse(
                        "hash-mismatch",
                        format!("{p} is {now} in the working copy, not {h}"),
                        "leave the hashes to the layer: gdd approve and gdd brief compute them",
                    ));
                }
            }
        }
        Err(match command_for(id, ev, st) {
            Some((what, command)) => Fail::refuse(
                "use-the-command",
                format!(
                    "{what} has a command of its own, which checks what it records: gdd log writes nothing"
                ),
                command,
            ),
            None => Fail::refuse(
                "use-the-command",
                format!(
                    "{:?} is not an event of the contract, and each of its events has a command of its own: gdd log writes nothing",
                    if st.is_empty() { ev } else { st }
                ),
                format!("gdd next {id} names the command of the next step"),
            ),
        })
    })
}

pub fn ask(ctx: &Ctx, id: &str, of: &str, what: &str) -> Out {
    run("ask", |out| {
        if !log::ASK_OF.contains(&of) {
            return Err(Fail::usage(format!(
                "--of is one of {}",
                log::ASK_OF.join(", ")
            )));
        }
        if what.trim().is_empty() || what.contains('\n') {
            return Err(Fail::usage("what is one line"));
        }
        let l = load(ctx, id)?;
        // A question about a brief, or about the change, waits for a brief to be logged; one about
        // a rule of the product is how a brief starts, and goes any time.
        let kind = log_kind(id);
        if (of == "brief" || of == "change")
            && kind == LogKind::Change
            && l.state.progress.brief_line.is_none()
        {
            return Err(Fail::refuse(
                "out-of-order",
                format!("no brief is logged for {id}: run gdd brief first"),
                format!(
                    "gdd brief {id} [<brief and scenario files>] --paths <path>... [--request \"<their words>\", or --request-file <a file with them>]"
                ),
            ));
        }
        if of == "brief" && kind == LogKind::Setup {
            return Err(Fail::refuse(
                "out-of-order",
                format!(
                    "{id} sets up checks and has no brief: the person reviews it with --of change, once its trial passes"
                ),
                format!("gdd next {id}"),
            ));
        }
        let ev = new_event(
            id,
            "ask",
            &time::now_string(),
            obj(vec![("of", json!(of)), ("what", json!(what.trim()))]),
        );
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the ask, line {line}: send the message now, and the wait counts from here"
        ));
        let then = crate::next::follow_up(id, of, l.state.progress.version.unwrap_or(1));
        out.say(format!("when they answer: {then}"));
        out.fact("then", json!(then));
        out.fact("line", line);
        Ok(())
    })
}

pub fn answer(ctx: &Ctx, id: &str, words: &str) -> Out {
    run("answer", |out| {
        let l = load(ctx, id)?;
        let Some(a) = l.state.progress.open_ask().cloned() else {
            return Err(Fail::refuse(
                "no-open-ask",
                format!("{id} has no question waiting for an answer"),
                "gdd waiting",
            ));
        };
        let ev = new_event(
            id,
            "answer",
            &time::now_string(),
            obj(vec![
                ("of", json!(a.of)),
                ("what", json!(words.trim())),
                ("ask", json!(a.line)),
            ]),
        );
        let line = write(&l, id, &ev)?;
        let waited = a.ts.map(|t| time::now() - t).unwrap_or(0);
        out.say(format!(
            "wrote their answer to line {}, line {line}, after {}",
            a.line,
            time::waited(waited)
        ));
        out.fact("line", line);
        out.fact("waited_seconds", waited);
        Ok(())
    })
}

pub struct Approval<'a> {
    pub of: &'a str,
    pub version: i64,
    pub who: &'a str,
    pub place: &'a str,
    pub files: &'a [String],
}

pub fn approve(ctx: &Ctx, id: &str, a: &Approval) -> Out {
    run("approve", |out| {
        if !log::APPROVAL_OF.contains(&a.of) {
            return Err(Fail::usage(format!(
                "--of is one of {}",
                log::APPROVAL_OF.join(", ")
            )));
        }
        let l = load(ctx, id)?;
        // A change is approved as it was judged: its files as its judged head holds them, which
        // the orchestrator's copy, on the main branch, may not have at all.
        let judged = (a.of == "change")
            .then(|| {
                judged_commit(&l)
                    .map(|(_, j)| j)
                    .or(l.state.git.head.clone())
            })
            .flatten();
        let mut files = Map::new();
        for f in a.files {
            let on_head = judged.as_ref().and_then(|j| git::blob_at(&ctx.root, j, f));
            let h = match on_head {
                Some(b) => {
                    if git::hash_object(&ctx.root, f).is_ok_and(|w| w != b) {
                        out.say(format!(
                            "{f}: the change's version, at {}, not the working copy's",
                            git::short(judged.as_deref().unwrap_or(""))
                        ));
                    }
                    b
                }
                None => git::hash_object(&ctx.root, f).map_err(|_| {
                    Fail::missing(
                        f.to_string(),
                        format!(
                            "{f} is not in the working copy{}",
                            if a.of == "change" {
                                ", nor at the change's head"
                            } else {
                                ""
                            }
                        ),
                    )
                })?,
            };
            if a.of == "brief"
                && let Some(sealed) = l.state.progress.brief_files.get(f)
                && sealed != &h
            {
                return Err(Fail::refuse(
                    "hash-mismatch",
                    format!(
                        "{f} in the working copy ({}) is not the version sealed in the brief step ({})",
                        git::short(&h),
                        git::short(sealed)
                    ),
                    "show the person the sealed version, or seal a new version first",
                ));
            }
            files.insert(f.clone(), json!(h));
        }
        if a.of == "brief" && !l.state.progress.brief_files.is_empty() {
            let missing: Vec<&String> = l
                .state
                .progress
                .brief_files
                .keys()
                .filter(|p| p.starts_with("features/") && !a.files.contains(p))
                .collect();
            if !missing.is_empty() {
                out.say(format!(
                    "note: the approval leaves out {}, which the brief seals",
                    missing
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let ev = new_event(
            id,
            "approval",
            &time::now_string(),
            obj(vec![
                ("who", json!(a.who)),
                ("of", json!(a.of)),
                ("version", json!(a.version)),
                ("files", Value::Object(files.clone())),
                ("where", json!(a.place)),
            ]),
        );
        let line = write(&l, id, &ev)?;
        out.say(format!(
            "wrote the approval of {}, line {line}, with {} files as the person saw them",
            a.of,
            files.len()
        ));
        for (f, h) in &files {
            out.say(format!("  {f} {}", h.as_str().unwrap_or("")));
        }
        out.fact("line", line);
        out.fact("files", Value::Object(files));
        Ok(())
    })
}

pub struct Waiver<'a> {
    pub who: &'a str,
    pub check: &'a str,
    pub why: &'a str,
    pub backlog: &'a str,
    pub limit: Option<&'a str>,
    pub to: Option<i64>,
}

pub fn waiver(ctx: &Ctx, id: &str, w: &Waiver) -> Out {
    run("waiver", |out| {
        if w.limit.is_some() != w.to.is_some() {
            return Err(Fail::usage(
                "a waiver of a limit takes both --limit and --to",
            ));
        }
        if let Some(lim) = w.limit
            && !crate::blueprint::CAP_KEYS.contains(&lim)
            && lim != "lines_per_change"
        {
            return Err(Fail::usage(format!(
                "--limit is a cap of the blueprint: {}",
                crate::blueprint::CAP_KEYS.join(", ")
            )));
        }
        if w.check == "review" && w.limit.is_none() {
            out.say("note: a reviewer's stop is never waived: it gets a fix and a new review");
        }
        check_id(w.backlog)?;
        let l = load(ctx, id)?;
        let mut m = obj(vec![
            ("who", json!(w.who)),
            ("check", json!(w.check)),
            ("why", json!(w.why)),
            ("backlog", json!(w.backlog)),
        ]);
        if let (Some(lim), Some(to)) = (w.limit, w.to) {
            m.insert("limit".into(), json!(lim));
            m.insert("to".into(), json!(to));
        }
        let ev = new_event(id, "waiver", &time::now_string(), m);
        let line = write(&l, id, &ev)?;
        out.say(format!("wrote the waiver, line {line}"));
        let path = l.project.path("gdd/backlog.md");
        if let Ok(text) = std::fs::read_to_string(&path)
            && !backlog::items(&text).iter().any(|i| i.id == w.backlog)
        {
            let item = backlog::Item {
                id: w.backlog.into(),
                what: format!(
                    "Go through what {id} skipped: {}",
                    w.limit.unwrap_or(w.check)
                ),
                color: l.state.progress.color.clone().unwrap_or_default(),
                state: "todo".into(),
                ..backlog::Item::default()
            };
            if let Ok(t) = backlog::add(&text, &item) {
                std::fs::write(&path, t).map_err(|e| crate::out::io(&path, e))?;
                out.say(format!("added {} to the backlog, so the change goes through what it skipped afterwards", w.backlog));
            }
        }
        out.fact("line", line);
        Ok(())
    })
}

pub fn correct(ctx: &Ctx, id: &str, line: usize, what: &str, void: bool) -> Out {
    run("correct", |out| {
        let l = load(ctx, id)?;
        if !l.entries.iter().any(|e| e.line == line) {
            return Err(Fail::usage(format!(
                "gdd/log/{id}.jsonl has no line {line}"
            )));
        }
        if void {
            // A closed log is closed: no line of it stops counting once the change shipped.
            if l.state.progress.shipped.is_some() {
                return Err(Fail::refuse(
                    "out-of-order",
                    format!("{id} shipped and its log is closed: no line of it is voided now"),
                    format!(
                        "say what was wrong without --void, which changes nothing of what counted: gdd correct {id} --line {line} \"<what was wrong>\""
                    ),
                ));
            }
            // A line that sent the change back is never voided: that would waive it.
            let target = l
                .entries
                .iter()
                .find(|e| e.line == line)
                .and_then(|e| e.event());
            if let Some(why) = target.and_then(log::never_voided) {
                return Err(Fail::refuse(
                    "never-voided",
                    format!("line {line} is {why}. Voiding it would waive it"),
                    way_on(&l, id),
                ));
            }
        }
        let mut m = obj(vec![("line", json!(line)), ("what", json!(what))]);
        if void {
            m.insert("void".into(), json!(true));
        }
        let ev = new_event(id, "correction", &time::now_string(), m);
        let n = log::append(&l.project.log_path(id), &ev).map_err(|e| {
            Fail::refuse(
                "bad-shape",
                e,
                format!("gdd verify {id}: it shows the log as it is, line by line"),
            )
        })?;
        out.say(format!(
            "wrote a correction of line {line}, line {n}{}; no line was rewritten",
            if void {
                ", which takes it out of the change's state. What it used still counts against the caps"
            } else {
                ""
            }
        ));
        out.fact("line", n);
        Ok(())
    })
}
