//! Changes open at once: how many the blueprint's `parallel` lets in, and whether a new one shares
//! a planned path with one already open (decision 135).

use super::*;

/// A change briefed and neither shipped nor stopped, with the paths its brief planned.
pub(super) struct Open {
    pub id: String,
    pub paths: Vec<String>,
}

/// Whether a change is open: briefed, and neither shipped nor stopped.
fn is_open(p: &state::Progress) -> bool {
    p.briefs > 0 && p.shipped.is_none() && p.brief_stopped.is_none()
}

fn cleaned(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| color::clean_path(p).ok())
        .collect()
}

/// Two planned paths collide when they are the same, or one is a folder that holds the other: a
/// brief's paths are literal, so this needs no git.
fn collide(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

pub(super) fn open_changes(ctx: &Ctx, except: Option<&str>) -> Vec<Open> {
    let project = Project::open(&ctx.root);
    project
        .log_ids()
        .into_iter()
        .filter(|id| log_kind(id) == LogKind::Change && Some(id.as_str()) != except)
        .filter_map(|id| {
            let p = state::Progress::of(&id, &project.read_log(&id));
            is_open(&p).then(|| Open {
                paths: cleaned(&p.paths),
                id,
            })
        })
        .collect()
}

/// The open changes that plan a path these paths collide with, each with the paths they share.
fn shared(open: &[Open], paths: &[String]) -> Vec<(String, Vec<String>)> {
    let mine = cleaned(paths);
    open.iter()
        .filter_map(|o| {
            let both: Vec<String> = mine
                .iter()
                .filter(|m| o.paths.iter().any(|p| collide(m, p)))
                .cloned()
                .collect();
            (!both.is_empty()).then(|| (o.id.clone(), both))
        })
        .collect()
}

fn full(open: &[Open], most: usize) -> String {
    let ids: Vec<&str> = open.iter().map(|o| o.id.as_str()).collect();
    format!(
        "{} changes are open and the blueprint's parallel allows {most}: {}",
        open.len(),
        ids.join(", ")
    )
}

/// A new change, or one taken up again after a stop, starts alongside the open ones only while
/// the blueprint's `parallel` has room and it shares no planned path with any of them. Without
/// `parallel`, nothing is held back.
pub(super) fn room_for(ctx: &Ctx, bp: &Blueprint, id: &str, paths: &[String]) -> R<()> {
    let Some(most) = bp.parallel() else {
        return Ok(());
    };
    let open = open_changes(ctx, Some(id));
    if open.len() >= most {
        return Err(Fail::refuse(
            "parallel-full",
            full(&open, most),
            format!(
                "{id} starts once one of them ships or stops (gdd next <id>; a change the person dropped: gdd brief <id> --stop \"<their words>\"), or the person raises parallel in gdd/blueprint.md"
            ),
        ));
    }
    if let Some((other, files)) = shared(&open, paths).into_iter().next() {
        return Err(Fail::refuse(
            "paths-shared",
            format!(
                "{id} would touch {}, which the open change {other} plans to touch too",
                files.join(", ")
            ),
            format!("{id} starts once {other} ships: gdd next {other}"),
        ));
    }
    Ok(())
}

/// `gdd next <id>` for a change that isn't open yet, while the blueprint's `parallel` has no room:
/// it waits, and nobody is asked anything.
pub(super) fn no_room(
    ctx: &Ctx,
    bp: &Blueprint,
    id: &str,
    p: &state::Progress,
) -> Option<state::Next> {
    let most = bp.parallel()?;
    if is_open(p) {
        return None;
    }
    let open = open_changes(ctx, Some(id));
    (open.len() >= most).then(|| state::Next {
        step: "wait".into(),
        blocked: Some("parallel-full".into()),
        say: format!("{}: {id} starts once one of them ships or stops", full(&open, most)),
        command: format!("gdd next {}", open[0].id),
    })
}

/// What `gdd next` and `gdd color` say about running alongside the open changes. A change already
/// open is held back by nothing, so nothing is said for it.
pub(super) fn say(ctx: &Ctx, bp: &Blueprint, id: Option<&str>, paths: &[String], out: &mut Out) {
    let Some(most) = bp.parallel() else {
        return;
    };
    let project = Project::open(&ctx.root);
    if let Some(i) = id
        && is_open(&state::Progress::of(i, &project.read_log(i)))
    {
        return;
    }
    let open = open_changes(ctx, id);
    let room = most.saturating_sub(open.len());
    out.fact("parallel", json!({"allows": most, "open": open.len(), "room": room}));
    if paths.is_empty() {
        out.say(if room == 0 {
            format!(
                "no room for another change: {} open, and the blueprint's parallel allows {most}",
                open.len()
            )
        } else {
            format!(
                "room for {room} more change{} at once: {} open, and the blueprint's parallel allows {most}",
                if room == 1 { "" } else { "s" },
                open.len()
            )
        });
        return;
    }
    for (other, files) in shared(&open, paths) {
        out.say(format!(
            "it shares {} with the open change {other}: it starts once {other} ships",
            files.join(", ")
        ));
    }
    if room == 0 {
        out.say(format!(
            "no room alongside: {} open, and the blueprint's parallel allows {most}",
            open.len()
        ));
    }
}
