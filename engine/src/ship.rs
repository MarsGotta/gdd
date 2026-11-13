//! Shipping: the description of the merge request, taken from the log and the brief, never from
//! memory.

use crate::log::{step, str_of};
use crate::project::{section, title};
use crate::state::State;
use serde_json::Value;

fn secs(v: &Value) -> String {
    match v.as_f64() {
        Some(s) => format!("{s} s"),
        None => "not timed".into(),
    }
}

/// Whether a section only says there is nothing: "Nothing.", "None", "Nothing else.".
fn says_nothing(text: &str) -> bool {
    let w = text.trim().trim_end_matches('.').to_lowercase();
    ["nothing", "none", "nothing else", "nothing yet", "n/a"].contains(&w.as_str())
}

/// The description of a change's merge request.
pub fn describe(
    state: &State,
    events: &[(usize, &Value)],
    brief: Option<&(Value, String)>,
) -> String {
    let p = &state.progress;
    let id = &state.id;
    let body = brief.map(|b| b.1.as_str()).unwrap_or("");
    let mut out = vec![];
    out.push(format!("## {}", title(body).unwrap_or_else(|| id.clone())));
    out.push(String::new());
    out.push("### What changes".into());
    out.push(
        section(body, "What changes")
            .or_else(|| section(body, "Request"))
            .unwrap_or_else(|| {
                let paths = if p.paths.is_empty() {
                    "the paths of its log".to_string()
                } else {
                    p.paths.join(", ")
                };
                if crate::state::log_kind(id) == crate::state::LogKind::Setup {
                    format!(
                        "It sets up checks of the project ({id}), tried before they ship: {paths}."
                    )
                } else if let Some(r) = &p.request {
                    format!("{r}\n\nA change with no brief, to {paths}.")
                } else {
                    format!("A change with no brief, to {paths}.")
                }
            }),
    );
    out.push(String::new());
    let color = p.color.clone().unwrap_or_else(|| "unknown".into());
    out.push(format!("### Color: {color}"));
    let mut why = vec![];
    if !p.zones.is_empty() {
        why.push(format!("zones {}", p.zones.join(", ")));
    }
    if !p.uncovered.is_empty() {
        why.push(format!("{} no zone covers", p.uncovered.join(", ")));
    }
    if !p.why.is_empty() {
        why.push(p.why.clone());
    }
    if let Some(rc) = p.last_review.as_ref().and_then(|r| r.color.clone()) {
        why.push(format!("the reviewer found {rc} on what was touched"));
    }
    out.push(if why.is_empty() {
        "As its paths gave it.".into()
    } else {
        why.join("; ") + "."
    });
    out.push(String::new());
    out.push("### Gates".into());
    match p
        .last_gates
        .as_ref()
        .and_then(|g| events.iter().find(|(l, _)| *l == g.line))
    {
        Some((_, g)) => {
            if let Some(h) = g["head"].as_str() {
                out.push(format!(
                    "On {}, from {}:",
                    &h[..h.len().min(12)],
                    &str_of(g, "start")[..str_of(g, "start").len().min(12)]
                ));
            }
            for e in g["gates"].as_array().cloned().unwrap_or_default() {
                out.push(format!(
                    "- {}: {}, {} (`{}`, exit {})",
                    str_of(&e, "check"),
                    str_of(&e, "result"),
                    secs(&e["seconds"]),
                    str_of(&e, "command"),
                    e["exit"]
                ));
            }
        }
        None => out.push("- none ran on this head".into()),
    }
    out.push(String::new());
    out.push("### Review".into());
    let mut later: Vec<String> = vec![];
    match p
        .last_review
        .as_ref()
        .and_then(|r| events.iter().find(|(l, _)| *l == r.line))
    {
        Some((_, r)) => {
            out.push(format!(
                "{}, round {}{}.",
                str_of(r, "verdict"),
                r["report"]["round"],
                match r["report"]["model"].as_str() {
                    Some(m) => format!(", on {m}"),
                    None => String::new(),
                }
            ));
            for ran in r["ran"].as_array().cloned().unwrap_or_default() {
                out.push(format!(
                    "- ran `{}`: exit {}",
                    str_of(&ran, "command"),
                    ran["exit"]
                ));
            }
            for f in r["findings"].as_array().cloned().unwrap_or_default() {
                out.push(format!(
                    "- {} {}: {}",
                    str_of(&f, "id"),
                    str_of(&f, "where"),
                    str_of(&f, "what")
                ));
            }
            for c in r["closed"].as_array().cloned().unwrap_or_default() {
                out.push(format!(
                    "- {} closed: {}",
                    str_of(&c, "id"),
                    str_of(&c, "outcome")
                ));
                if str_of(&c, "outcome") == "minor" {
                    later.push(format!("finding {}", str_of(&c, "id")));
                }
            }
        }
        None => out.push("No review yet.".into()),
    }
    for (_, v) in events {
        if step(v) == "review" {
            for l in v["later"].as_array().cloned().unwrap_or_default() {
                if let Some(s) = l.as_str() {
                    later.push(s.to_string());
                }
            }
        }
    }
    out.push(String::new());
    out.push("### Left for later".into());
    // A brief that says it leaves nothing out says so only when nothing else was left for later.
    let wont =
        section(body, "What it won't do yet").filter(|w| !(says_nothing(w) && !later.is_empty()));
    if let Some(w) = &wont {
        out.push(w.clone());
    }
    for l in &later {
        out.push(format!("- {l}"));
    }
    if wont.is_none() && later.is_empty() {
        out.push("Nothing.".into());
    }
    out.push(String::new());
    out.push("### Undoing it".into());
    out.push(section(body, "Undoing it").unwrap_or_else(|| {
        if color == "red" {
            "No way to undo it was written: on red, the person decides knowing that.".into()
        } else {
            "Revert its merge commit (`git revert -m 1 <merge>`).".into()
        }
    }));
    out.join("\n")
}
