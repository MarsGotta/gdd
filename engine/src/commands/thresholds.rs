//! The figures the blueprint's `harden` names: once a change ships and one is crossed, the layer
//! adds a proposal of hardening to the backlog, with the numbers (decision 144). Nothing changes
//! the blueprint here: the item says what crossed, and what it becomes is the person's to approve.

use super::*;

/// A figure crossed, as an item of the backlog.
struct Crossed {
    id: String,
    what: String,
}

/// The change logs in the order they started, each with its color and its events.
fn changes(project: &Project) -> Vec<(String, String, Vec<Value>)> {
    let mut all: Vec<(i64, String, String, Vec<Value>)> = project
        .log_ids()
        .into_iter()
        .filter(|id| log_kind(id) == LogKind::Change)
        .filter_map(|id| {
            let entries = project.read_log(&id);
            let p = state::Progress::of(&id, &entries);
            let color = p.color.clone()?;
            let events: Vec<Value> = log::events(&entries).into_iter().map(|(_, v)| v.clone()).collect();
            let first = events.iter().find_map(log::ts).unwrap_or(0);
            Some((first, id, color, events))
        })
        .collect();
    all.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    all.into_iter().map(|(_, id, c, e)| (id, c, e)).collect()
}

/// "1 change", "3 changes": how many, said as a person would.
fn changes_said(n: usize) -> String {
    format!("{n} change{}", if n == 1 { "" } else { "s" })
}

fn strings_of(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn sent_back(v: &Value) -> bool {
    matches!(step(v), "review" | "gates")
        && matches!(v["fix_role"].as_str(), Some("checks" | "checks-then-build"))
}

fn crossed(project: &Project, bp: &Blueprint) -> Vec<Crossed> {
    let figure = |key: &str| bp.harden_figure(key);
    let all = changes(project);
    // Each figure counts what came after hardening last looked: a record resets it.
    let hardened: Vec<(i64, String)> = log::events(&project.read_log("blueprint"))
        .into_iter()
        .filter(|(_, v)| log::is_step(v, "harden"))
        .filter_map(|(_, v)| Some((log::ts(v)?, str_of(v, "expected_check").to_string())))
        .collect();
    let since = hardened.iter().map(|(t, _)| *t).max().unwrap_or(i64::MIN);
    let after = |events: &[Value]| events.iter().find_map(log::ts).is_some_and(|t| t > since);
    let mut out = vec![];
    for color in ["green", "yellow", "red"] {
        let mine: Vec<&(String, String, Vec<Value>)> = all.iter().filter(|c| c.1 == color).collect();
        // Changes of one color whose tests came back to whoever wrote them.
        if let Some(n) = figure("sent_back") {
            let back = mine
                .iter()
                .filter(|c| c.2.iter().any(|v| sent_back(v) && log::ts(v).is_some_and(|t| t > since)))
                .count();
            if back >= n {
                out.push(Crossed {
                    id: format!("harden-tests-{color}"),
                    what: format!("{} of {color} came back to whoever wrote their checks, against a figure of {n}: read what sent them back and propose stronger tests or a check of planted faults", changes_said(back)),
                });
            }
        }
        // Briefs of one color approved in a row, the latest ones, with no correction. Never red:
        // the layer doesn't nudge toward a weaker red.
        if let Some(n) = figure("approved_unchanged").filter(|_| color != "red") {
            let streak = mine
                .iter()
                .filter(|c| after(&c.2))
                .rev()
                .filter_map(|c| {
                    let events: Vec<(usize, &Value)> = c.2.iter().enumerate().map(|(i, v)| (i + 1, v)).collect();
                    crate::metrics::brief_corrections(&events).as_u64()
                })
                .take_while(|n| *n == 0)
                .count();
            if streak >= n {
                out.push(Crossed {
                    id: format!("harden-approval-{color}"),
                    what: format!("the last {streak} {color} briefs were approved with no correction, against a figure of {n}: propose keeping or taking brief-approval out of {color}, with what those yeses cost"),
                });
            }
        }
    }
    // Changes that touched a zone and shipped since it was last explored (decision 147).
    if let Some(n) = figure("explore_after") {
        let explored: Vec<(String, i64)> = log::events(&project.read_log("blueprint"))
            .into_iter()
            .filter(|(_, v)| log::is_step(v, "explore"))
            .filter_map(|(_, v)| Some((str_of(v, "zone").to_string(), log::ts(v)?)))
            .collect();
        for zone in bp.zones() {
            let since = explored
                .iter()
                .filter(|(z, _)| *z == zone.id)
                .map(|(_, t)| *t)
                .max()
                .unwrap_or(i64::MIN);
            let touched = all
                .iter()
                .filter(|(_, _, events)| {
                    let zones: Vec<String> = events
                        .iter()
                        .rev()
                        .find(|v| step(v) == "brief" && v["color"]["zones"].is_array())
                        .map(|v| strings_of(&v["color"]["zones"]))
                        .unwrap_or_default();
                    let shipped = events.iter().find(|v| step(v) == "ship").and_then(log::ts);
                    zones.contains(&zone.id) && shipped.is_some_and(|t| t > since)
                })
                .count();
            if touched >= n {
                out.push(Crossed {
                    id: format!("explore-{}", zone.id),
                    what: format!("{} touched {} since it was last explored, against a figure of {n}: explore it with a charter, as gdd-harden's references/exploring.md says", changes_said(touched), zone.id),
                });
            }
        }
    }
    // A check that only reports, and caught something the builder then fixed: it earned blocking.
    for check in bp.checks().into_iter().filter(|c| c.reports()) {
        let caught = all
            .iter()
            .filter(|(_, _, events)| {
                let results: Vec<bool> = events
                    .iter()
                    .filter(|v| step(v) == "gates")
                    .flat_map(|v| v["gates"].as_array().into_iter().flatten())
                    .filter(|g| str_of(g, "check") == check.id)
                    .map(|g| str_of(g, "result") == "pass")
                    .collect();
                results.iter().position(|p| !p).is_some_and(|i| results[i..].contains(&true))
            })
            .count();
        if caught > 0 {
            out.push(Crossed {
                id: format!("harden-blocks-{}", check.id),
                what: format!("{} only reports, and in {} it failed and then the builder made it pass: propose making it block", check.id, changes_said(caught)),
            });
        }
    }
    // Runs in a row of a check that rejected nothing, the latest ones.
    if let Some(n) = figure("silent_runs") {
        let mut runs: Vec<(i64, String, bool)> = vec![];
        for (_, _, events) in &all {
            for v in events.iter().filter(|v| step(v) == "gates") {
                let t = log::ts(v).unwrap_or(0);
                for g in v["gates"].as_array().into_iter().flatten() {
                    // A reported failure rejected something too.
                    runs.push((t, str_of(g, "check").to_string(), str_of(g, "result") != "pass"));
                }
            }
        }
        runs.sort();
        for check in bp.checks().into_iter().filter(|c| c.kind == "machine" && c.id != "seal") {
            let looked = hardened
                .iter()
                .filter(|(_, c)| *c == check.id)
                .map(|(t, _)| *t)
                .max()
                .unwrap_or(i64::MIN);
            let silent = runs
                .iter()
                .rev()
                .filter(|r| r.1 == check.id && r.0 > looked)
                .take_while(|r| !r.2)
                .count();
            if silent >= n {
                out.push(Crossed {
                    id: format!("harden-silent-{}", check.id),
                    what: format!("{} ran {silent} times in a row and rejected nothing, against a figure of {n}: plant its bad case to see it still rejects it, then propose keeping, relaxing or retiring it", check.id),
                });
            }
        }
    }
    out
}

/// The id an item of this proposal takes: the bare one, or the next `<id>-<n>` once every earlier
/// one is done; none while one of them is still to do.
fn free_id(base: &str, items: &[backlog::Item]) -> Option<String> {
    let same: Vec<&backlog::Item> = items
        .iter()
        .filter(|i| {
            i.id == base
                || i.id
                    .strip_prefix(base)
                    .and_then(|rest| rest.strip_prefix('-'))
                    .is_some_and(|n| n.parse::<u32>().is_ok())
        })
        .collect();
    if same.iter().any(|i| i.state != "done") {
        return None;
    }
    Some(match same.len() {
        0 => base.to_string(),
        n => format!("{base}-{}", n + 1),
    })
}

/// One proposal of hardening the layer found by itself, added to the backlog unless it holds it.
pub(super) fn add(root: &Path, bp: &Blueprint, id: &str, what: &str, out: &mut Out) {
    let found = vec![Crossed {
        id: id.into(),
        what: what.into(),
    }];
    add_all(root, bp, found, "the layer found something to harden", out);
}

/// After a ship: each figure crossed whose item the backlog doesn't hold yet goes there, in the
/// color a change to the blueprint takes.
pub(super) fn open(root: &Path, bp: &Blueprint, out: &mut Out) {
    let found = crossed(&Project::open(root), bp);
    add_all(root, bp, found, "a figure of the blueprint's harden crossed", out);
}

fn add_all(root: &Path, bp: &Blueprint, found: Vec<Crossed>, why: &str, out: &mut Out) {
    let project = Project::open(root);
    if found.is_empty() {
        return;
    }
    let path = project.path("gdd/backlog.md");
    let Ok(mut text) = std::fs::read_to_string(&path) else {
        return;
    };
    let color = color::color_of(root, bp, &["gdd/blueprint.md".to_string()], None, None, "")
        .map(|c| c.color)
        .unwrap_or_else(|_| "red".into());
    let mut added = vec![];
    for mut c in found {
        // An item still to do holds it back; one done leaves room for the next, `<id>-2` on.
        let items = backlog::items(&text);
        let Some(id) = free_id(&c.id, &items) else {
            continue;
        };
        c.id = id;
        let item = backlog::Item {
            id: c.id.clone(),
            what: c.what.clone(),
            color: color.clone(),
            state: "todo".into(),
            ..backlog::Item::default()
        };
        if let Ok(t) = backlog::add(&text, &item) {
            text = t;
            out.say(format!(
                "{why}: added {} to the backlog. {} Propose it with gdd-harden when the person takes it up: what it becomes is theirs to approve",
                c.id, c.what
            ));
            added.push(c.id);
        }
    }
    if !added.is_empty() && std::fs::write(&path, text).is_ok() {
        out.fact("harden_opened", json!(added));
    }
}
