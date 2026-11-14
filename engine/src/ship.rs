//! Shipping: the description of the merge request, taken from the log and the brief, never from
//! memory; and what went into the main branch, from git, never from what an agent says merged.

use crate::git;
use crate::log::{step, str_of};
use crate::project::{section, title};
use crate::state::State;
use serde_json::Value;
use std::path::Path;

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
                    git::short(h),
                    git::short(str_of(g, "start"))
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

// ----- what went into the main branch -----

/// How a judged change is found in the main branch.
#[derive(Debug, Clone, PartialEq)]
pub enum WentIn {
    /// By the merge commit `merge`, whose other parent, `tip`, is the judged commit or stands for it.
    Merged { tip: String, merge: String },
    /// By a fast-forward: the main branch's own line holds the judged commit, and after it, up to
    /// `tip`, only the layer's bookkeeping.
    FastForward { tip: String },
    /// By squash or rebase: the commits of the main branch that brought the change's files, the
    /// last one `at`, hold exactly what was judged in each of its `files`, and nothing else.
    Content { files: usize, at: String },
}

/// Why what the main branch holds can't be said to be what was judged: a refusal's code and words.
#[derive(Debug, Clone, PartialEq)]
pub struct NotIn {
    pub code: &'static str,
    pub why: String,
}

fn not_in(code: &'static str, why: String) -> NotIn {
    NotIn { code, why }
}

/// What went into `main`, at `main_tip`, of a change judged at `judged` from `start`. `stands(c)`
/// says whether a commit is the judged one or stands for it by the layer's own rule (it differs
/// only in the layer's bookkeeping). `bound` is the tip that was merged, as the change's branch or
/// `--head` gives it, when there is one.
///
/// - The judged commit on the main branch's own line: a fast-forward. After it come the change's
///   bookkeeping commits, then whatever the main branch took next. When anything that is not
///   bookkeeping comes after them, only `bound` can say where the change ended: a plain commit, or
///   a merge (the builder's own merge of the main branch looks just like the next change's merge),
///   may be the change's as well as the next one's. Without a bound inside the bookkeeping run, it
///   can't be told.
/// - The judged commit in the main branch through a merge: the oldest commit of the main branch's
///   own line that holds it is the merge that brought it, and its other parent is what was merged.
///   A later merge whose other parent carries on from the judged commit without that merge brought
///   more of the change's own line, and must stand for it too.
/// - Not in it: a squash or a rebase. See [`by_content`].
#[allow(clippy::too_many_arguments)]
pub fn went_in(
    root: &Path,
    id: &str,
    main: &str,
    main_tip: &str,
    start: &str,
    judged: &str,
    bound: Option<&str>,
    stands: &dyn Fn(&str) -> bool,
) -> Result<WentIn, NotIn> {
    if !git::is_ancestor(root, judged, main_tip) {
        return by_content(root, id, main, main_tip, start, judged)
            .map(|(files, at)| WentIn::Content { files, at });
    }
    let base = (!start.is_empty())
        .then(|| git::merge_base(root, start, main_tip))
        .flatten()
        .ok_or_else(|| {
            not_in(
                "not-merged",
                format!("no <start> of the change that {main} holds: where it began can't be told"),
            )
        })?;
    let line = git::first_parents(root, &base, main_tip)
        .map_err(|e| not_in("not-merged", format!("{main}'s history can't be read: {e}")))?;
    if let Some(i) = line.iter().position(|c| c == judged) {
        let mut end = i;
        while end + 1 < line.len() && stands(&line[end + 1]) {
            end += 1;
        }
        let run = &line[i..=end];
        let bounded = bound.filter(|b| run.iter().any(|c| c == b));
        if let Some(next) = line.get(end + 1)
            && bounded.is_none()
        {
            return Err(not_in(
                "which-tip",
                format!(
                    "{} went into {main} by a fast-forward, and the commit after it, {}, isn't the layer's bookkeeping: whether it is the change's or the next one's can't be told",
                    git::short(&line[end]),
                    git::short(next)
                ),
            ));
        }
        return Ok(WentIn::FastForward {
            tip: bounded.unwrap_or(judged).to_string(),
        });
    }
    // The main branch's own line holds the judged commit from one commit on, and from then on
    // every commit of it does.
    let e = line.partition_point(|c| !git::is_ancestor(root, judged, c));
    let Some(merge) = line.get(e) else {
        return Err(not_in(
            "not-merged",
            format!(
                "{} is in {main}, but not on its own line",
                git::short(judged)
            ),
        ));
    };
    let Some(tip) = git::parents(root, merge)
        .into_iter()
        .skip(1)
        .find(|p| git::is_ancestor(root, judged, p))
    else {
        return Err(not_in(
            "unjudged",
            format!(
                "{} brought {} into {main}, and it is not a merge of it: what came with it can't be told",
                git::short(merge),
                git::short(judged)
            ),
        ));
    };
    if !stands(&tip) {
        return Err(not_in(
            "unjudged",
            format!(
                "{main} took {} by the merge {}, and it differs from {}, the commit that was judged, in more than the layer's bookkeeping",
                git::short(&tip),
                git::short(merge),
                git::short(judged)
            ),
        ));
    }
    for later in &line[e + 1..] {
        for q in git::parents(root, later).iter().skip(1) {
            if git::is_ancestor(root, judged, q) && !git::is_ancestor(root, merge, q) && !stands(q)
            {
                return Err(not_in(
                    "unjudged",
                    format!(
                        "{main} took {} by the merge {}, which carries on from {} with what nobody judged",
                        git::short(q),
                        git::short(later),
                        git::short(judged)
                    ),
                ));
            }
        }
    }
    Ok(WentIn::Merged {
        tip,
        merge: merge.clone(),
    })
}

/// A change merged by squash or by rebase, whose commits aren't in `main`: the commits of the
/// main branch's own line since it met the change's `<start>` that touched the change's files are
/// what brought it. They, and any commit between them, must change nothing but the change's files
/// and the layer's bookkeeping, and the last of them must hold the judged blob of every file the
/// change touched. Commits before the first of them and after the last, which touch none of its
/// files, are taken for other changes; one between them can't be told from the change, and is
/// refused. Returns how many files matched and the commit where it landed.
pub fn by_content(
    root: &Path,
    id: &str,
    main: &str,
    main_tip: &str,
    start: &str,
    judged: &str,
) -> Result<(usize, String), NotIn> {
    let missing = |why: String| {
        not_in(
            "not-merged",
            format!(
                "{} is not in {main} yet, and {main} doesn't hold what was judged ({why}): a ship is written once the change is in",
                git::short(judged)
            ),
        )
    };
    if start.is_empty() {
        return Err(missing("no <start> to compare from".into()));
    }
    let touched = git::diff_names(root, start, judged).map_err(&missing)?;
    if touched.is_empty() {
        return Err(missing("the change touched no file".into()));
    }
    let base = git::merge_base(root, start, main_tip)
        .ok_or_else(|| missing(format!("<start> shares no commit with {main}")))?;
    let line = git::first_parents(root, &base, main_tip).map_err(&missing)?;
    let mut changed = vec![];
    for c in &line {
        changed.push(git::changed_by(root, c).map_err(&missing)?);
    }
    let brought: Vec<usize> = (0..line.len())
        .filter(|&k| changed[k].iter().any(|p| touched.contains(p)))
        .collect();
    let (Some(&first), Some(&last)) = (brought.first(), brought.last()) else {
        return Err(missing(format!(
            "no commit of {main} since {} touched its files",
            git::short(&base)
        )));
    };
    let files = crate::judged::holds_judged(root, start, judged, &line[last]).map_err(&missing)?;
    for k in first..=last {
        let extra: Vec<&str> = changed[k]
            .iter()
            .filter(|p| {
                !touched.contains(p) && !crate::judged::bookkeeping_known(root, id, &line[k], p)
            })
            .map(String::as_str)
            .collect();
        if extra.is_empty() {
            continue;
        }
        return Err(not_in(
            "unjudged",
            if brought.contains(&k) {
                format!(
                    "{} brought {} into {main} along with the change's files, and nobody judged them",
                    git::short(&line[k]),
                    extra.join(", ")
                )
            } else {
                format!(
                    "{} changed {} between the commits that brought the change into {main}: it can't be told from the change, and nobody judged it",
                    git::short(&line[k]),
                    extra.join(", ")
                )
            },
        ));
    }
    Ok((files, line[last].clone()))
}
