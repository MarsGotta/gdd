//! Close: what a change in use was expected to give, and the person's verdict on it.

use super::*;
use crate::closing::{self, Standing};

pub struct CloseArgs<'a> {
    /// `yes`, `not-yet` or `no`; without it the command only shows what to look at.
    pub verdict: Option<&'a str>,
    pub who: Option<&'a str>,
    pub place: Option<&'a str>,
    pub saw: Option<&'a str>,
    /// What comes next, when the person already said it: a backlog item, `harden` or `none`.
    pub then: Option<&'a str>,
}

/// Refuse a change that isn't in use, or that is already closed.
fn in_use(l: &Loaded, id: &str) -> R<Standing> {
    match l.state.standing.clone() {
        None => Err(Fail::refuse(
            "out-of-order",
            format!("{id} isn't a change in use: a change is closed once it has shipped"),
            way_on(l, id),
        )),
        Some(Standing::Closed(how)) => Err(Fail::refuse(
            "out-of-order",
            format!("{id} is closed, {how}"),
            "what comes next is a new change, with an id of its own (gdd brief <new id> --paths <path>...); if something slipped through it, load gdd-harden",
        )),
        Some(s) => Ok(s),
    }
}

/// Without a verdict: what the brief expected and how to gather what is seen. With one: the
/// `close` event, in the person's words.
pub fn close(ctx: &Ctx, id: &str, a: &CloseArgs) -> Out {
    let Some(verdict) = a.verdict else {
        return run("close", |out| look(&load(ctx, id)?, id, out));
    };
    run_change(ctx, id, "close", |out| {
        if !closing::VERDICTS.contains(&verdict) {
            return Err(Fail::usage(format!(
                "--verdict is one of {}",
                closing::VERDICTS.join(", ")
            )));
        }
        let mut fields = vec![("verdict", json!(verdict))];
        for (flag, key, value) in [
            ("--who", "who", a.who),
            ("--where", "where", a.place),
            ("--saw", "saw", a.saw),
        ] {
            let value = value.map(str::trim).unwrap_or_default();
            if value.is_empty() || value.contains('\n') || value.starts_with('<') {
                return Err(Fail::usage(format!(
                    "{flag} is one line: who gave the verdict, where they gave it, and what they saw, in their words"
                )));
            }
            fields.push((key, json!(value)));
        }
        if let Some(then) = a.then.map(str::trim).filter(|t| !t.is_empty()) {
            fields.push(("then", json!(then)));
        }
        let l = load(ctx, id)?;
        in_use(&l, id)?;
        if let Some(ask) = l.state.progress.open_ask().filter(|q| q.of == "close") {
            fields.push(("ask", json!(ask.line)));
        }
        let ev = new_event(id, "close", &time::now_string(), obj(fields));
        let line = write(&l, id, &ev)?;
        out.say(match verdict {
            "yes" => format!("wrote their yes, line {line}: {id} is closed"),
            "no" => format!(
                "wrote their no, line {line}: {id} is closed, and its log is not opened again"
            ),
            _ => format!(
                "wrote their not yet, line {line}: {id} stays in use and in sight. Nothing else to do, and don't ask again until they come back to it"
            ),
        });
        if verdict == "no" {
            out.say("A no has two ways out. Propose the one you'd take, for a yes:");
            out.say("  it works as written but didn't help: a new request in the backlog (gdd backlog add <item> --what \"<what the person gets>\" --color <color>), or nothing, if they accept it as it is");
            out.say("  it broke a rule or did harm: the fix is a normal change (load gdd-brief), and a hardening proposal, because a gate should have caught it (load gdd-harden)");
        }
        out.fact("line", line);
        out.fact("verdict", json!(verdict));
        Ok(())
    })
}

/// What to look at, for the orchestrator that gathers it: nothing is written.
fn look(l: &Loaded, id: &str, out: &mut Out) -> R<()> {
    let standing = in_use(l, id)?;
    let p = &l.state.progress;
    let now = time::now();
    out.say(format!(
        "{id}: in use for {}",
        time::waited(now - p.shipped_at.unwrap_or(now))
    ));
    let who =
        l.bp.closing_who()
            .unwrap_or_else(|| "the person who owns the change".into());
    if let Standing::Quiet(until) = standing {
        let why = p.closing.as_ref().map_or("", |c| str_of(c, "nothing"));
        out.say(format!(
            "nothing to close{}{why}: it closes by itself in {} unless something is recorded against it",
            if why.is_empty() { "" } else { ", " },
            time::waited(until - now)
        ));
        out.say(format!(
            "{who} can still give a verdict: {}",
            closing::verdict_command(id)
        ));
        return Ok(());
    }
    match p.closing.as_ref().filter(|c| c["expected"].is_string()) {
        Some(c) => {
            for key in ["expected", "signal", "where"] {
                out.say(format!("{key}: {}", str_of(c, key)));
            }
        }
        None => out.say(closing::expected(p)),
    }
    for v in &p.verdicts {
        out.say(format!(
            "{} looked {} ago and said {}",
            v.who,
            time::waited(now - v.ts.unwrap_or(now)),
            v.verdict
        ));
    }
    out.say("Gather what is seen there, each fact with its source and its date. Never from the builder's report, the change's own tests or anything the change wrote about itself. If you can't reach that place, say so and ask the person to look.");
    out.say(format!(
        "The verdict belongs to {who}, never to you: put what was expected next to what is seen, propose no verdict, and ask."
    ));
    out.fact("closing", p.closing.clone().unwrap_or(Value::Null));
    out.next = Some(state::Next {
        step: "ask".into(),
        say: "once you have gathered it, record the question and send your message".into(),
        command: format!(
            "gdd ask {id} --of close \"<what was expected>. <What is seen, and from where>. Did it help: yes, not yet or no?\""
        ),
        blocked: None,
    });
    Ok(())
}
