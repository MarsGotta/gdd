//! What git says about a change: its branch and its head, the seal's tip, and whether a commit
//! still stands for the one its gates or its review judged, when only the layer's own bookkeeping
//! or a merge that touched nothing of the change came after it.

use crate::log::Entry;
use crate::state::{LogKind, Progress, hashes, log_kind};
use std::collections::BTreeMap;

/// What git says about a change now. Without git, everything is `None`.
#[derive(Debug, Clone, Default)]
pub struct GitView {
    pub available: bool,
    pub branch: Option<String>,
    /// The tip of the change's branch: the commit its gates judge.
    pub head: Option<String>,
    pub seal_tip: Option<String>,
    pub main_tip: Option<String>,
    /// Whether the repository has a remote, which says how it likely ships.
    pub has_remote: bool,
    /// Pairs of a commit that was judged and a commit that stands for it: the second differs from
    /// the first only in the orchestrator's bookkeeping, or by a merge of the main branch that
    /// touched none of the files the change touches.
    pub same: Vec<(String, String)>,
}

/// The orchestrator's bookkeeping, by name: what the layer itself commits as bookkeeping. That is
/// `gdd/backlog.md`, and the files directly in `gdd/log/` named `<id>.jsonl` or
/// `<id>.engine.jsonl` for a valid id; nothing in a subfolder, nothing with another name.
pub fn is_bookkeeping(path: &str) -> bool {
    if path == "gdd/backlog.md" {
        return true;
    }
    let Some(name) = path.strip_prefix("gdd/log/") else {
        return false;
    };
    name.strip_suffix(".engine.jsonl")
        .or_else(|| name.strip_suffix(".jsonl"))
        .is_some_and(crate::valid_id)
}

/// The bookkeeping commits the layer made for a change, as its journal in the working copy keeps
/// them: each commit with the blob of every file it changed.
fn bookkeeping_commits(
    root: &std::path::Path,
    id: &str,
) -> Vec<(String, BTreeMap<String, String>)> {
    if !crate::valid_id(id) {
        return vec![];
    }
    crate::journal::read(&root.join(crate::project::Project::journal_rel(id)))
        .iter()
        .filter(|j| j["command"] == "ship")
        .filter_map(|j| {
            let b = &j["bookkeeping"];
            Some((b["commit"].as_str()?.to_string(), hashes(&b["files"])))
        })
        .collect()
}

/// Whether a bookkeeping file holds, at `commit`, content the orchestrator's bookkeeping of `id` had:
/// the working copy's (both absent counts), or a blob a bookkeeping commit of the layer put there.
/// For commits of the main branch a squash or a rebase made, which descend from none of the layer's.
pub fn bookkeeping_known(root: &std::path::Path, id: &str, commit: &str, path: &str) -> bool {
    if !is_bookkeeping(path) {
        return false;
    }
    let at = crate::git::blob_at(root, commit, path);
    let copy = root
        .join(path)
        .is_file()
        .then(|| crate::git::hash_object(root, path).ok())
        .flatten();
    at == copy
        || at.is_some_and(|blob| {
            bookkeeping_commits(root, id)
                .iter()
                .any(|(_, files)| files.get(path) == Some(&blob))
        })
}

/// Whether a bookkeeping file holds, at `b`, what the orchestrator's bookkeeping holds: the blob
/// of the working copy's file (both absent counts), or the blob a bookkeeping commit of the layer
/// put there, that commit being between `a` and `b`.
fn bookkeeping_holds(
    root: &std::path::Path,
    a: &str,
    b: &str,
    path: &str,
    commits: &[(String, BTreeMap<String, String>)],
) -> bool {
    let at_b = crate::git::blob_at(root, b, path);
    let copy = root
        .join(path)
        .is_file()
        .then(|| crate::git::hash_object(root, path).ok())
        .flatten();
    if at_b == copy {
        return true;
    }
    let Some(blob) = at_b else { return false };
    commits.iter().any(|(c, files)| {
        files.get(path) == Some(&blob)
            && crate::git::is_ancestor(root, a, c)
            && crate::git::is_ancestor(root, c, b)
    })
}

impl GitView {
    /// Whether `now` stands for the commit `judged`: the same commit, or one that changed nothing
    /// a judge looks at since.
    pub fn stands_for(&self, judged: &str, now: &str) -> bool {
        judged == now || self.same.iter().any(|(a, b)| a == judged && b == now)
    }

    /// Look at git for a change and its log: as [`GitView::of`], and which commits stand for the
    /// ones its last gates and its last review judged.
    pub fn for_log(
        root: &std::path::Path,
        id: &str,
        branch: Option<&str>,
        head: Option<&str>,
        entries: &[Entry],
    ) -> GitView {
        let mut gv = GitView::of(root, id, branch, head);
        if !gv.available {
            return gv;
        }
        let p = Progress::of(id, entries);
        let g = p.last_gates.as_ref().and_then(|g| g.head.clone());
        let r = p.last_review.as_ref().and_then(|r| r.head.clone());
        let r_start = p.last_review.as_ref().and_then(|r| r.start.clone());
        let mut same = vec![];
        if let Some(h) = gv.head.clone() {
            for j in [&g, &r].into_iter().flatten() {
                if j != &h && only_bookkeeping(root, id, j, &h) {
                    same.push((j.clone(), h.clone()));
                }
            }
        }
        if let (Some(r), Some(g)) = (&r, &g)
            && r != g
            && (only_bookkeeping(root, id, r, g)
                || review_carries(root, r, g, r_start.as_deref(), &gv))
        {
            same.push((r.clone(), g.clone()));
            if let Some(h) = gv.head.clone()
                && h != *g
                && only_bookkeeping(root, id, g, &h)
            {
                same.push((r.clone(), h));
            }
        }
        gv.same = same;
        gv
    }
}

/// Whether `b` differs from `a` only in the orchestrator's bookkeeping of change `id`, `a` being in
/// `b`'s history: every path that differs is a bookkeeping file by name ([`is_bookkeeping`]), and
/// holds at `b` the working copy's content or what the layer's own bookkeeping commit put there.
/// A file a builder put under `gdd/log/` is neither, and the head that carries it is not judged.
pub fn only_bookkeeping(root: &std::path::Path, id: &str, a: &str, b: &str) -> bool {
    if !crate::git::is_ancestor(root, a, b) {
        return false;
    }
    let Ok(names) = crate::git::diff_names(root, a, b) else {
        return false;
    };
    if !names.iter().all(|p| is_bookkeeping(p)) {
        return false;
    }
    let commits = bookkeeping_commits(root, id);
    names
        .iter()
        .all(|p| bookkeeping_holds(root, a, b, p, &commits))
}

/// Whether `at` holds exactly what `judged` holds in every file the change touched from `start` to
/// `judged`, blob by blob: how a change merged by squash or by rebase, whose commits aren't in the
/// main branch, is still found there. Returns how many files it compared, or the first that differs.
pub fn holds_judged(
    root: &std::path::Path,
    start: &str,
    judged: &str,
    at: &str,
) -> Result<usize, String> {
    if start.is_empty() {
        return Err("no <start> to compare from".into());
    }
    let touched = crate::git::diff_names(root, start, judged)?;
    if touched.is_empty() {
        return Err("the change touched no file".into());
    }
    for p in &touched {
        if crate::git::blob_at(root, judged, p) != crate::git::blob_at(root, at, p) {
            return Err(format!("{p} differs"));
        }
    }
    Ok(touched.len())
}

/// Whether a review of `reviewed` still holds on `judged`: everything between them came from the
/// main branch or the seal, and none of it touched the files the change touched as reviewed.
pub fn review_carries(
    root: &std::path::Path,
    reviewed: &str,
    judged: &str,
    start: Option<&str>,
    gv: &GitView,
) -> bool {
    use crate::git;
    let Some(start) = start else { return false };
    if !git::is_ancestor(root, reviewed, judged) {
        return false;
    }
    let mut args = vec![
        "rev-list".to_string(),
        "--no-merges".into(),
        judged.into(),
        format!("^{reviewed}"),
    ];
    if let Some(m) = &gv.main_tip {
        args.push(format!("^{m}"));
    }
    if let Some(s) = &gv.seal_tip {
        args.push(format!("^{s}"));
    }
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let Ok(own) = git::out(root, &a) else {
        return false;
    };
    if !own.trim().is_empty() {
        return false;
    }
    let (Ok(touched), Ok(brought)) = (
        git::diff_names(root, start, reviewed),
        git::diff_names(root, reviewed, judged),
    ) else {
        return false;
    };
    brought
        .iter()
        .filter(|p| !is_bookkeeping(p))
        .all(|p| !touched.contains(p))
}

impl GitView {
    /// Look at git for a change. `branch` overrides the usual names of the change's branch.
    pub fn of(
        root: &std::path::Path,
        id: &str,
        branch: Option<&str>,
        head: Option<&str>,
    ) -> GitView {
        if !crate::git::is_repo(root) {
            return GitView::default();
        }
        let seal = format!("gdd-seal/{id}");
        // A setup log ships its seal: what it sets up lives there, and nobody builds on top.
        let branch = branch
            .map(str::to_string)
            .or_else(|| crate::git::change_branch(root, id))
            .or_else(|| {
                (log_kind(id) == LogKind::Setup
                    && crate::git::commit_of(root, &format!("refs/heads/{seal}")).is_some())
                .then_some(seal.clone())
            });
        let head = head
            .and_then(|h| crate::git::commit_of(root, h))
            .or_else(|| branch.as_ref().and_then(|b| crate::git::commit_of(root, b)));
        let main = crate::git::main_branch(root);
        GitView {
            available: true,
            has_remote: crate::git::out(root, &["remote"]).is_ok_and(|r| !r.trim().is_empty()),
            same: vec![],
            seal_tip: crate::git::commit_of(root, &format!("refs/heads/gdd-seal/{id}")),
            main_tip: crate::git::commit_of(root, &main),
            branch,
            head,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookkeeping_is_only_what_the_layer_commits() {
        for p in [
            "gdd/backlog.md",
            "gdd/log/empty-cart.jsonl",
            "gdd/log/empty-cart.engine.jsonl",
            "gdd/log/blueprint.engine.jsonl",
        ] {
            assert!(is_bookkeeping(p), "{p}");
        }
        for p in [
            "gdd/log/payload.sh",
            "gdd/log/sub/x.jsonl",
            "gdd/log/Empty-Cart.jsonl",
            "gdd/log/.jsonl",
            "gdd/log/x.jsonl.bak",
            "gdd/log/x y.jsonl",
            "gdd/logs/x.jsonl",
            "gdd/backlog.md.orig",
        ] {
            assert!(!is_bookkeeping(p), "{p}");
        }
    }
}
