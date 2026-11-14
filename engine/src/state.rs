//! A change's state, derived only from its log and from what git says now. Every command takes it
//! from [`derive`], so no two commands can disagree about where a change stands.

use crate::blueprint::Blueprint;
use crate::limits::{self, Limits};
use crate::log::{self, Entry, kind, result, step, str_of, ts};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// What a log is about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogKind {
    /// A change, with a brief step first.
    Change,
    /// Setting up checks or a zone's gate: `standing-checks`, `standing-checks-<check>`, `gates-<zone>`.
    Setup,
    /// `blueprint`: preparing, hardening, the exam and the approvals of the blueprint.
    Blueprint,
}

pub fn log_kind(id: &str) -> LogKind {
    if id == "blueprint" {
        LogKind::Blueprint
    } else if id == "standing-checks"
        || id.starts_with("standing-checks-")
        || id.starts_with("gates-")
    {
        LogKind::Setup
    } else {
        LogKind::Change
    }
}

/// What git says about a change now. Without git, everything is `None`.
#[derive(Debug, Clone, Default)]
pub struct GitView {
    pub available: bool,
    pub branch: Option<String>,
    /// The tip of the change's branch: the commit its gates judge.
    pub head: Option<String>,
    pub seal_tip: Option<String>,
    pub main: String,
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
    crate::journal::read(&root.join(format!("gdd/log/{id}.engine.jsonl")))
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
        let mut p = Progress::new(id);
        for (line, v) in log::events(entries) {
            p.apply(line, v);
        }
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
            main,
            branch,
            head,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub line: usize,
    pub of: String,
    pub what: String,
    pub ts: Option<i64>,
    /// The kind of the event after it, which ended the wait, or `None` while it waits.
    pub ended_by: Option<String>,
    pub ended_at: Option<i64>,
    /// Whether what answered it was the answer it asked for: its approval, a waiver, an answer.
    pub answered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatesRun {
    pub line: usize,
    pub passed: bool,
    pub failed: Vec<String>,
    pub ran: Vec<String>,
    pub start: String,
    pub head: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub line: usize,
    pub verdict: String,
    pub head: Option<String>,
    pub start: Option<String>,
    pub color: Option<String>,
}

/// The next step of a change: what to do, the exact command, and when it can't go on, why.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Next {
    /// `brief`, `ask`, `wait`, `checks`, `build`, `gates`, `review`, `ship`, `done`, `stopped`.
    pub step: String,
    pub say: String,
    pub command: String,
    /// When it is stopped: `cap-spent`, `needs-person`, `seal-broken`, `review-stop`, `cannot`...
    pub blocked: Option<String>,
}

/// Where a change stands, as the log keeps it, step by step. [`Progress::apply`] takes one event;
/// [`Progress::allows`] says whether an event may come next, which is how the layer refuses a step
/// out of order and how `verify` finds one in a log.
#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub kind: Option<LogKind>,
    pub events: usize,
    pub color: Option<String>,
    pub planned: Option<String>,
    pub version: Option<i64>,
    pub base: Option<String>,
    pub start: Option<String>,
    pub brief_line: Option<usize>,
    pub brief_stopped: Option<String>,
    pub brief_files: BTreeMap<String, String>,
    pub sealed: BTreeMap<String, String>,
    pub uncovered: Vec<String>,
    pub zones: Vec<String>,
    pub paths: Vec<String>,
    pub why: String,
    pub approvals: Vec<(usize, String, BTreeMap<String, String>)>,
    pub checks_done: bool,
    pub checks_stopped: i64,
    pub checks_line: Option<usize>,
    pub builds: i64,
    pub last_build: Option<(usize, bool)>,
    pub build_stop: Option<String>,
    pub gates_runs: i64,
    pub last_gates: Option<GatesRun>,
    pub reviews: i64,
    pub last_review: Option<Review>,
    pub stop_told: bool,
    pub asks: Vec<Ask>,
    pub shipped: Option<(usize, String)>,
    pub last_ts: Option<i64>,
    pub first_ts: Option<i64>,
    pub checks_runs: i64,
    pub readings_n: i64,
    pub briefs: i64,
    /// Whether a build came after the last brief or checks step, which start the judging over.
    pub built_since_seal: bool,
    /// The person's request, word for word, when a brief step kept it.
    pub request: Option<String>,
    /// The line of the brief step from which the change is red, and whether a reading came after it.
    pub red_since: Option<usize>,
    pub read_since_red: bool,
    /// The seal's moves the log records with a `seal` event.
    pub seal_moves: i64,
    /// Whether the log knows a seal: a `seal` event, a brief step with its seal, or checks sealed.
    pub sealed_any: bool,
    /// Whether the person was told (an `ask --of stop`) that the last gates found the seal touched.
    pub seal_told: bool,
    /// Spans of the seal, from a commit to a later one, that held work no step had recorded
    /// when a `seal` event moved `<start>` past them: a role's report still counts them.
    pub unrecorded: Vec<(String, String)>,
}

fn hashes(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

impl Progress {
    pub fn new(id: &str) -> Progress {
        Progress {
            kind: Some(log_kind(id)),
            ..Progress::default()
        }
    }

    /// How many times gdd-checks was launched for the change, stopped ones included.
    pub fn checks_launches(&self) -> i64 {
        self.checks_runs
    }

    pub fn readings(&self) -> i64 {
        self.readings_n
    }

    pub fn has_brief(&self) -> bool {
        !self.brief_files.is_empty()
    }

    /// Whether the person approved what has to pass, as the brief now is: every scenario file
    /// (or every file, when the brief has no scenario file) with the hash of its last approval.
    pub fn approved(&self) -> bool {
        let Some(brief_line) = self.brief_line else {
            return false;
        };
        let briefs: Vec<&(usize, String, BTreeMap<String, String>)> =
            self.approvals.iter().filter(|a| a.1 == "brief").collect();
        if self.brief_files.is_empty() {
            return briefs.iter().any(|a| a.0 > brief_line);
        }
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for a in &briefs {
            for (p, h) in &a.2 {
                seen.insert(p, h);
            }
        }
        let scenarios: Vec<(&String, &String)> = self
            .brief_files
            .iter()
            .filter(|(p, _)| p.starts_with("features/"))
            .collect();
        let need: Vec<(&String, &String)> = if scenarios.is_empty() {
            self.brief_files.iter().collect()
        } else {
            scenarios
        };
        !briefs.is_empty()
            && need
                .iter()
                .all(|(p, h)| seen.get(p.as_str()) == Some(&h.as_str()))
    }

    /// Whether the person approved the change itself after its last build.
    pub fn person_reviewed(&self) -> bool {
        let after = self.last_build.map(|b| b.0).unwrap_or(0);
        self.approvals
            .iter()
            .any(|a| a.1 == "change" && a.0 > after)
    }

    pub fn open_ask(&self) -> Option<&Ask> {
        self.asks.last().filter(|a| a.ended_by.is_none())
    }

    /// Take one event, in the order of the log.
    pub fn apply(&mut self, line: usize, v: &Value) {
        self.events += 1;
        let t = ts(v);
        if let Some(t) = t {
            if self.first_ts.is_none() {
                self.first_ts = Some(t);
            }
            self.last_ts = Some(t);
        }
        let k = kind(v);
        if k != "ask"
            && k != "correction"
            && let Some(a) = self.asks.last_mut().filter(|a| a.ended_by.is_none())
        {
            a.ended_by = Some(if k == "step" {
                step(v).to_string()
            } else {
                k.to_string()
            });
            a.ended_at = t;
            a.answered = match k {
                "answer" => true,
                "approval" => str_of(v, "of") == a.of,
                "waiver" => a.of == "waiver" || a.of == "stop" || a.of == "decision",
                _ => false,
            };
        }
        match k {
            "ask" => {
                if str_of(v, "of") == "stop"
                    && self
                        .last_review
                        .as_ref()
                        .is_some_and(|r| r.verdict == "stop")
                {
                    self.stop_told = true;
                }
                if str_of(v, "of") == "stop"
                    && self
                        .last_gates
                        .as_ref()
                        .is_some_and(|g| g.failed.iter().any(|c| c == "seal"))
                {
                    self.seal_told = true;
                }
                if let Some(a) = self.asks.last_mut().filter(|a| a.ended_by.is_none()) {
                    a.ended_by = Some("ask".into());
                    a.ended_at = t;
                }
                self.asks.push(Ask {
                    line,
                    of: str_of(v, "of").into(),
                    what: str_of(v, "what").into(),
                    ts: t,
                    ended_by: None,
                    ended_at: None,
                    answered: false,
                });
            }
            "approval" => self
                .approvals
                .push((line, str_of(v, "of").into(), hashes(&v["files"]))),
            "seal" => {
                self.seal_moves += 1;
                self.sealed_any = true;
                if let Some(to) = v["to"].as_str() {
                    if let (Some(from), Some(start)) = (v["from"].as_str(), self.start.as_deref())
                        && from != start
                    {
                        self.unrecorded.push((start.to_string(), from.to_string()));
                    }
                    self.start = Some(to.into());
                }
            }
            "step" => self.apply_step(line, v),
            _ => {}
        }
    }

    fn apply_step(&mut self, line: usize, v: &Value) {
        match step(v) {
            "brief" => {
                self.briefs += 1;
                let color = v["color"]["color"]
                    .as_str()
                    .filter(|c| crate::color_rank(c).is_some())
                    .or_else(|| Some(result(v)).filter(|c| crate::color_rank(c).is_some()));
                if result(v) == "stopped" {
                    self.brief_stopped = Some(str_of(&v["stop"], "reason").to_string());
                } else {
                    self.brief_stopped = None;
                }
                if let Some(c) = color {
                    if self.planned.is_none() {
                        self.planned = Some(c.into());
                    }
                    if c == "red" && self.color.as_deref() != Some("red") {
                        self.red_since = Some(line);
                        self.read_since_red = false;
                    }
                    self.color = Some(c.into());
                }
                if let Some(r) = v["request"].as_str().filter(|r| !r.trim().is_empty()) {
                    self.request = Some(r.to_string());
                }
                self.version = v["version"].as_i64().or(self.version);
                if self.base.is_none() {
                    self.base = v["color"]["base"].as_str().map(str::to_string);
                }
                self.paths = strings(&v["color"]["paths"]);
                self.zones = strings(&v["color"]["zones"]);
                self.uncovered = strings(&v["color"]["uncovered"]);
                self.why = str_of(&v["color"], "why").to_string();
                self.brief_files = hashes(&v["files"]);
                for (p, h) in &self.brief_files {
                    self.sealed.insert(p.clone(), h.clone());
                }
                if let Some(s) = v["seal"].as_str() {
                    self.start = Some(s.into());
                    self.sealed_any = true;
                } else if self.start.is_none() {
                    self.start = self.base.clone();
                }
                self.brief_line = Some(line);
                self.checks_done = false;
                self.checks_stopped = 0;
                self.built_since_seal = false;
                self.build_stop = None;
                self.last_gates = None;
                self.last_review = None;
                self.stop_told = false;
                self.seal_told = false;
            }
            "checks" => {
                self.checks_runs += 1;
                self.checks_line = Some(line);
                if result(v) == "stopped" || v.get("stop").is_some() {
                    self.checks_stopped += 1;
                } else {
                    self.checks_done = true;
                    self.checks_stopped = 0;
                    self.unrecorded.clear();
                    if let Some(c) = v["seal"]["commit"].as_str() {
                        self.start = Some(c.into());
                        self.sealed_any = true;
                    }
                    for (p, h) in hashes(&v["seal"]["checks"]) {
                        self.sealed.insert(p, h);
                    }
                    // A setup log has no brief: its color is the one its sealed paths give.
                    if let Some(c) = v["color"]["color"]
                        .as_str()
                        .filter(|c| crate::color_rank(c).is_some())
                    {
                        let c = match &self.color {
                            Some(old) => crate::higher(old, c).to_string(),
                            None => c.to_string(),
                        };
                        if self.planned.is_none() {
                            self.planned = Some(c.clone());
                        }
                        self.color = Some(c);
                        self.zones = strings(&v["color"]["zones"]);
                        self.uncovered = strings(&v["color"]["uncovered"]);
                        self.paths = strings(&v["color"]["paths"]);
                    }
                    if self.base.is_none() {
                        self.base = v["color"]["base"].as_str().map(str::to_string);
                    }
                    self.built_since_seal = false;
                    self.build_stop = None;
                    self.last_gates = None;
                    self.last_review = None;
                    self.seal_told = false;
                }
            }
            "build" => {
                self.builds += 1;
                let stopped = result(v) == "stopped" || v.get("stop").is_some();
                self.build_stop = stopped.then(|| str_of(&v["stop"], "reason").to_string());
                self.last_build = Some((line, stopped));
                self.built_since_seal = true;
                self.last_gates = None;
                self.last_review = None;
                self.stop_told = false;
                self.seal_told = false;
            }
            "gates" => {
                self.gates_runs += 1;
                self.seal_told = false;
                let mut failed = vec![];
                let mut ran = vec![];
                if let Some(Value::Array(items)) = v.get("gates") {
                    for g in items {
                        let c = str_of(g, "check").to_string();
                        if str_of(g, "result") == "fail" {
                            failed.push(c.clone());
                        }
                        ran.push(c);
                    }
                }
                let start = str_of(v, "start").to_string();
                if !start.is_empty() {
                    self.start = Some(start.clone());
                }
                let head = v["head"].as_str().map(str::to_string);
                self.last_gates = Some(GatesRun {
                    line,
                    passed: failed.is_empty(),
                    failed,
                    ran,
                    start,
                    head,
                });
            }
            "review" => {
                self.reviews += 1;
                self.last_review = Some(Review {
                    line,
                    verdict: str_of(v, "verdict").to_string(),
                    head: v["color"]["head"].as_str().map(str::to_string),
                    start: v["color"]["start"].as_str().map(str::to_string),
                    color: v["color"]["color"].as_str().map(str::to_string),
                });
                self.stop_told = false;
            }
            "reading" => {
                self.readings_n += 1;
                if self.red_since.is_some_and(|l| l < line) {
                    self.read_since_red = true;
                }
            }
            "ship" => self.shipped = Some((line, str_of(v, "where").to_string())),
            _ => {}
        }
    }

    /// Whether this event may come next, and if not, the code and the reason.
    pub fn allows(&self, bp: Option<&Blueprint>, v: &Value) -> Result<(), (&'static str, String)> {
        let kind = self.kind.unwrap_or(LogKind::Change);
        if kind == LogKind::Blueprint {
            return Ok(());
        }
        let st = step(v);
        let color = self.color.clone().unwrap_or_default();
        let asks =
            |check: &str| bp.is_some_and(|b| !color.is_empty() && b.color_asks(&color, check));
        if self.shipped.is_some() && log::kind(v) == "step" {
            return Err((
                "out-of-order",
                format!(
                    "the change already shipped: its log is closed, and a {st} step can't follow"
                ),
            ));
        }
        match st {
            "checks" | "reading" if kind == LogKind::Change => {
                if self.brief_line.is_none() {
                    return Err(("out-of-order", format!("a {st} step before the brief step")));
                }
                if st == "checks" && asks("brief-approval") && !self.approved() {
                    return Err((
                        "needs-approval",
                        format!(
                            "checks before the person approved the brief, which {color} asks for: nothing but its reading goes before the approval"
                        ),
                    ));
                }
            }
            "build" => {
                if kind == LogKind::Change {
                    if self.brief_line.is_none() {
                        return Err(("out-of-order", "a build before the brief step: the color goes to the log before anyone builds".into()));
                    }
                    if asks("brief-approval") && !self.approved() {
                        return Err((
                            "needs-approval",
                            format!(
                                "a build before the person approved the brief, which {color} asks for"
                            ),
                        ));
                    }
                    if self.has_brief() && !self.checks_done {
                        return Err((
                            "out-of-order",
                            "a build before its checks were sealed".into(),
                        ));
                    }
                }
            }
            "gates" => {
                if kind == LogKind::Change && self.last_build.is_none() {
                    return Err(("out-of-order", "gates with no build before them".into()));
                }
            }
            "review" => {
                if kind == LogKind::Setup && !self.checks_done {
                    return Err((
                        "out-of-order",
                        "a trial before the checks were set up and sealed".into(),
                    ));
                }
                if kind == LogKind::Change {
                    match &self.last_gates {
                        None => {
                            return Err((
                                "out-of-order",
                                "a review before the gates ran on the last build".into(),
                            ));
                        }
                        Some(g) if !g.passed => {
                            return Err((
                                "out-of-order",
                                format!(
                                    "a review after gates that failed ({})",
                                    g.failed.join(", ")
                                ),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            "ship" => {
                if kind == LogKind::Change {
                    let Some(g) = &self.last_gates else {
                        return Err(("out-of-order", "a ship before its gates passed".into()));
                    };
                    if !g.passed {
                        return Err(("out-of-order", "a ship after gates that failed".into()));
                    }
                    let Some(r) = &self.last_review else {
                        return Err(("out-of-order", "a ship before its review passed".into()));
                    };
                    if r.verdict != "pass" {
                        return Err((
                            "out-of-order",
                            format!("a ship after a review that said {}", r.verdict),
                        ));
                    }
                    // A review before the last gates may hold for a later head that only brought
                    // bookkeeping or the main branch: git says so, in `verify`.
                    if let (Some(gh), Some(rh)) = (&g.head, &r.head)
                        && gh != rh
                        && r.line > g.line
                    {
                        return Err((
                            "out-of-order",
                            "a ship whose gates and review judged different heads".into(),
                        ));
                    }
                } else if self
                    .last_review
                    .as_ref()
                    .is_none_or(|r| r.verdict != "pass")
                {
                    return Err(("out-of-order", "a ship before its review passed".into()));
                }
                if asks("person-review") && !self.person_reviewed() {
                    return Err((
                        "needs-approval",
                        format!(
                            "a ship before the person reviewed the change, which {color} asks for"
                        ),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// A change's state: where it stands, what its limits allow, and its next step.
#[derive(Debug, Clone)]
pub struct State {
    pub id: String,
    pub progress: Progress,
    pub limits: Limits,
    pub next: Next,
    pub git: GitView,
    /// The checks that passed on the current head, by the last gates run.
    pub passed_on_head: Vec<String>,
}

impl State {
    pub fn facts(&self) -> Value {
        let p = &self.progress;
        json!({
            "id": self.id,
            "color": p.color,
            "version": p.version,
            "base": p.base,
            "start": p.start,
            "head": self.git.head,
            "branch": self.git.branch,
            "approved": p.approved(),
            "passed_on_head": self.passed_on_head,
            "review": p.last_review.as_ref().map(|r| r.verdict.clone()),
            "rounds": self.limits.line("rounds_per_change", "rounds").map(|l| l.used as i64),
            "open_ask": p.open_ask().map(|a| json!({"line": a.line, "of": a.of, "what": a.what})),
            "shipped": p.shipped.is_some(),
            "stop": self.limits.stop,
            "judge": self.limits.judge,
            "next": {"step": self.next.step, "command": self.next.command, "blocked": self.next.blocked},
        })
    }
}

/// Derive a change's state from its log and from git, as of `now`.
pub fn derive(id: &str, bp: &Blueprint, entries: &[Entry], git: GitView, now: i64) -> State {
    let events = log::events(entries);
    let mut p = Progress::new(id);
    for (line, v) in &events {
        p.apply(*line, v);
    }
    let values: Vec<&Value> = events.iter().map(|(_, v)| *v).collect();
    // A closed log counts to its last event; an open one to now.
    let until = if p.shipped.is_some() {
        p.last_ts.unwrap_or(now)
    } else {
        now
    };
    let lim = limits::count(bp, &values, until);
    let passed_on_head = match (&p.last_gates, &git.head) {
        (Some(g), Some(h)) if g.passed && g.head.as_ref().is_none_or(|x| git.stands_for(x, h)) => {
            g.ran.clone()
        }
        (Some(g), None) if g.passed => g.ran.clone(),
        _ => vec![],
    };
    let next = next_step(id, bp, &p, &lim, &git, now);
    State {
        id: id.into(),
        progress: p,
        limits: lim,
        next,
        git,
        passed_on_head,
    }
}

fn n(step: &str, say: impl Into<String>, command: impl Into<String>) -> Next {
    Next {
        step: step.into(),
        say: say.into(),
        command: command.into(),
        blocked: None,
    }
}

fn blocked(step: &str, code: &str, say: impl Into<String>, command: impl Into<String>) -> Next {
    Next {
        step: step.into(),
        say: say.into(),
        command: command.into(),
        blocked: Some(code.into()),
    }
}

/// The checks role's turn in a setup log. A spent cap comes first. One that stopped is launched
/// once more. One that stopped twice goes to the person, and so does every stop after their
/// answer: the same launch is never named again and again.
fn setup_checks(id: &str, p: &Progress, lim: &Limits, say: &str) -> Next {
    if !lim.stop.is_empty() {
        return cap_spent(id, lim);
    }
    if p.checks_stopped >= 2 {
        let answered = p
            .checks_line
            .is_some_and(|c| p.asks.iter().any(|a| a.line > c && a.answered));
        if !answered {
            return blocked(
                "stopped",
                "cannot",
                "gdd-checks stopped twice: what it can't set up goes to the person, with what you'd do. If they leave it for later, it stays in the backlog and the next request doesn't wait for it",
                format!("gdd ask {id} --of decision \"<what it can't set up, and what you'd do>\""),
            );
        }
        return n(
            "checks",
            "the person answered: launch gdd-checks once more, with their answer, unless they left it for later",
            format!("gdd launch {id} checks"),
        );
    }
    n("checks", say, format!("gdd launch {id} checks"))
}

fn round_needs_room(id: &str, lim: &Limits, then: Next) -> Next {
    if lim.stop.is_empty() {
        return then;
    }
    cap_spent(id, lim)
}

/// A spent cap: no new round, and how more room is asked for. More room is always the person's
/// to give, by a waiver; for `standing-checks`, which every change waits for, there is no next
/// item to go on with, so the person is asked at once.
pub fn cap_spent(id: &str, lim: &Limits) -> Next {
    let caps = lim.stop.join(", ");
    let have = if lim.stop.len() == 1 { "has" } else { "have" };
    let spent = lim
        .lines
        .iter()
        .find(|l| lim.stop.iter().any(|s| s == l.name));
    let (name, key, used, cap) = spent
        .map(|l| {
            (
                l.name,
                l.key,
                l.used.trunc() as i64,
                l.cap.map(|c| c.to_string()).unwrap_or("<cap>".into()),
            )
        })
        .unwrap_or(("<cap>", "<key>", 0, "<cap>".into()));
    // The cap that gives one more round: a count of rounds takes one more; a count of rounds sent
    // back is spent past its cap, so it takes what was used.
    let to = match key {
        "rounds_per_change" | "runs_before_review" => (used + 1).to_string(),
        "fix_rounds_per_gate" => used.to_string(),
        _ => "<n>".into(),
    };
    let ask = format!(
        "gdd ask {id} --of waiver \"{name}: {used} of {cap} used. One more round would <what it buys>. Yes?\""
    );
    let backlog = if log_kind(id) == LogKind::Change {
        "<item>".to_string()
    } else {
        id.to_string()
    };
    let waive = format!(
        "gdd waiver {id} --who <person> --check {key} --why \"<their words>\" --backlog {backlog} --limit {key} --to {to}"
    );
    if id == "standing-checks" {
        return blocked(
            "stopped",
            "cap-spent",
            format!(
                "no new round: {caps} {have} no room left, and every change waits for the standing checks, so there is no next item to go on with. Ask the person at once for one more round: {ask}; after their yes: {waive}"
            ),
            ask,
        );
    }
    let ends = if log_kind(id) == LogKind::Change {
        format!(". Left in the backlog, it ends here: gdd brief {id} --stop \"<their words>\"")
    } else {
        String::new()
    };
    blocked(
        "stopped",
        "cap-spent",
        format!(
            "no new round: {caps} {have} no room left. Tell the person in one line and propose splitting it or leaving it in the backlog, never more room; ask once. More room is the person's to give: {ask}, and after their yes: {waive}{ends}"
        ),
        format!("gdd ask {id} --of decision \"<split it, or leave it in the backlog?>\""),
    )
}

fn waiting_or_ask(id: &str, p: &Progress, of: &str, what: &str, then: &str, now: i64) -> Next {
    if let Some(a) = p.open_ask().filter(|a| a.of == of) {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        return blocked(
            "wait",
            "needs-person",
            format!(
                "waiting for the person since line {} ({waited}): {}",
                a.line, a.what
            ),
            then.to_string(),
        );
    }
    blocked(
        "ask",
        "needs-person",
        format!("ask the person: {what}"),
        format!("gdd ask {id} --of {of} \"<one question a yes answers>\""),
    )
}

/// What records the person's answer to an ask of a kind: an approval for what a person approves,
/// a waiver for a waiver, and their words for the rest.
pub fn follow_up(id: &str, of: &str, version: i64) -> String {
    match of {
        "brief" | "change" | "blueprint" | "gates" | "debt" => format!(
            "gdd approve {id} --of {of} --version {version} --who <person> --where <conversation, commit or pull request> <files>"
        ),
        "waiver" => format!(
            "gdd waiver {id} --who <person> --check <check or limit> --why \"<reason>\" --backlog <item> [--limit <cap> --to <n>], or for a no: gdd answer {id} \"<their words>\""
        ),
        _ => format!("gdd answer {id} \"<their words>\""),
    }
}

/// How a change that passed everything ships: by a local merge, or by pull request.
fn ship_step(id: &str, git: &GitView) -> Next {
    let merge = format!("gdd ship {id} --merge");
    let request = format!(
        "gdd ship {id} --bookkeeping, then once the pull request is merged: gdd ship {id} --done <where>"
    );
    let (first, second) = if git.has_remote {
        (request, merge)
    } else {
        (merge, request)
    };
    n(
        "ship",
        format!(
            "describe it (gdd ship {id} --describe) and ship it as the constitution says: by a local merge, or by pull request"
        ),
        format!("{first}; or {second}"),
    )
}

fn next_step(
    id: &str,
    bp: &Blueprint,
    p: &Progress,
    lim: &Limits,
    git: &GitView,
    now: i64,
) -> Next {
    let kind = p.kind.unwrap_or(LogKind::Change);
    if kind == LogKind::Blueprint {
        return n(
            "done",
            "the blueprint's log has no next step of its own",
            "",
        );
    }
    if let Some((line, w)) = &p.shipped {
        return n(
            "done",
            format!("it shipped ({w}), line {line}: its log is closed"),
            "",
        );
    }
    // An open question waits first, whatever it is about.
    if let Some(a) = p.open_ask() {
        let waited =
            a.ts.map(|t| crate::time::waited(now - t))
                .unwrap_or_default();
        let then = follow_up(id, &a.of, p.version.unwrap_or(1));
        return blocked(
            "wait",
            "needs-person",
            format!(
                "waiting for the person since line {} ({waited}), {}: {}",
                a.line, a.of, a.what
            ),
            then,
        );
    }
    let color = p.color.clone().unwrap_or_default();
    let asks = |c: &str| !color.is_empty() && bp.color_asks(&color, c);
    let head_short = git
        .head
        .as_deref()
        .map(|h| &h[..h.len().min(12)])
        .unwrap_or("its head");

    if kind == LogKind::Setup {
        if !p.checks_done {
            return setup_checks(
                id,
                p,
                lim,
                "launch gdd-checks to set the checks up and seal them",
            );
        }
        return match &p.last_review {
            None => n(
                "review",
                "launch gdd-review to try each check",
                format!("gdd launch {id} review"),
            ),
            Some(r) if r.verdict == "pass" => {
                if asks("person-review") && !p.person_reviewed() {
                    return waiting_or_ask(
                        id,
                        p,
                        "change",
                        "the person reviews the checks before they ship",
                        "",
                        now,
                    );
                }
                ship_step(id, git)
            }
            // A stop is never waived: once the person was told, a fix round, then a new trial,
            // and only a trial that passes lifts it.
            Some(r) if r.verdict == "stop" && !p.stop_told => blocked(
                "stopped",
                "review-stop",
                "the trial said stop: the checks don't ship. Tell the person why; a stop is never waived, it gets a fix round and a new trial",
                format!("gdd ask {id} --of stop \"<why it stopped>\""),
            ),
            Some(r) if r.verdict == "stop" => setup_checks(
                id,
                p,
                lim,
                "a fix round after the trial's stop: launch gdd-checks, which gets the trial's findings and the person's answer, then a new trial",
            ),
            Some(_) => setup_checks(
                id,
                p,
                lim,
                "the trial sent the checks back: launch gdd-checks again",
            ),
        };
    }

    if p.brief_line.is_none() {
        return n(
            "brief",
            "load gdd-brief: its color goes to the log before anyone builds",
            format!(
                "gdd brief {id} --paths <path>... [--color <color> --why \"<reason>\"] [--request \"<their words>\" or --request-file <a file with them>, when there is no brief file] [<brief and scenario files>]"
            ),
        );
    }
    if let Some(reason) = &p.brief_stopped {
        return blocked(
            "stopped",
            "cannot",
            format!(
                "the brief stopped: {reason}. It stays dropped until a new brief takes it up again"
            ),
            format!("gdd brief {id} ..."),
        );
    }
    if color == "red"
        && p.has_brief()
        && !p.read_since_red
        && !p.approved()
        && p.last_build.is_none()
        && p.open_ask().is_none()
    {
        return n(
            "reading",
            "a red brief is read before the person approves it: launch gdd-review to read it, then take into a new version what you agree with",
            format!("gdd launch {id} reading"),
        );
    }
    if asks("brief-approval") && !p.approved() {
        let read = if color == "yellow" {
            format!(
                " (if it changes data another zone owns, have it read first: gdd launch {id} reading)"
            )
        } else {
            String::new()
        };
        return waiting_or_ask(
            id,
            p,
            "brief",
            &format!(
                "approve what has to pass, which {color} asks for, before anyone builds{read}"
            ),
            &format!(
                "gdd approve {id} --of brief --version {} --who <person> --where conversation <files>",
                p.version.unwrap_or(1)
            ),
            now,
        );
    }
    if p.has_brief() && !p.checks_done {
        // The person answered after the second stop: what they decided moves it.
        let decided = p.checks_stopped >= 2
            && p.checks_line
                .is_some_and(|c| p.asks.iter().any(|a| a.line > c && a.answered));
        if decided {
            return blocked(
                "stopped",
                "cannot",
                format!(
                    "gdd-checks stopped twice and the person answered: a new version of the brief, sealed (gdd seal) and logged (gdd brief), takes the checks up again; if they said to try once more as it is, gdd launch {id} checks --level deep; if they dropped it, gdd brief {id} --stop \"<their words>\""
                ),
                format!("gdd brief {id} ..."),
            );
        }
        if p.checks_stopped >= 2 {
            return blocked(
                "stopped",
                "cannot",
                "gdd-checks stopped twice: a check it can't write goes to the person",
                format!(
                    "gdd ask {id} --of decision \"<the check it can't write, and the new version of the brief you propose>\""
                ),
            );
        }
        let deep = if p.checks_stopped == 1 {
            " --level deep"
        } else {
            ""
        };
        return round_needs_room(
            id,
            lim,
            n(
                "checks",
                "launch gdd-checks: a test for each scenario, seen failing and sealed",
                format!("gdd launch {id} checks{deep}"),
            ),
        );
    }
    let Some((_, build_stopped)) = p.last_build.filter(|_| p.built_since_seal) else {
        return round_needs_room(
            id,
            lim,
            n(
                "build",
                "launch gdd-build on its own branch, from <start>",
                format!("gdd launch {id} build"),
            ),
        );
    };
    if build_stopped {
        return blocked(
            "stopped",
            "cannot",
            format!(
                "the builder stopped with CANNOT: {}. A check that looks wrong goes to gdd-checks or the person; anything else to a new version of the brief",
                p.build_stop.clone().unwrap_or_default()
            ),
            format!("gdd brief {id} ..."),
        );
    }
    let gates_on_head = p
        .last_gates
        .as_ref()
        .filter(|g| match (&g.head, &git.head) {
            (Some(a), Some(b)) => git.stands_for(a, b),
            _ => true,
        });
    let Some(g) = gates_on_head else {
        if !lim.stop.is_empty() && lim.judge != "gates" {
            return round_needs_room(id, lim, Next::default());
        }
        return n(
            "gates",
            format!("run the gates on {head_short}, out of the builder's reach"),
            format!("gdd gates {id}"),
        );
    };
    if !g.passed {
        if g.failed.iter().any(|c| c == "seal") {
            if !p.seal_told {
                return blocked(
                    "stopped",
                    "seal-broken",
                    "what judges the change was touched: nothing else counts, the change stops, and the person is told",
                    format!("gdd ask {id} --of stop \"<what was touched>\""),
                );
            }
            // Told: what judges it goes back as it was sealed, in a fix round, and the gates again.
            return round_needs_room(
                id,
                lim,
                n(
                    "build",
                    format!(
                        "a fix round that puts back what judges the change exactly as it was sealed (the seal check's output names the files), then the gates; if the person dropped it: gdd brief {id} --stop \"<their words>\""
                    ),
                    format!("gdd launch {id} build"),
                ),
            );
        }
        return round_needs_room(
            id,
            lim,
            n(
                "build",
                format!("a fix round: {} failed", g.failed.join(", ")),
                format!("gdd launch {id} build"),
            ),
        );
    }
    let review_on_head = p.last_review.as_ref().filter(|r| match (&r.head, &g.head) {
        (Some(a), Some(b)) => git.stands_for(a, b),
        _ => r.line > g.line,
    });
    let Some(r) = review_on_head else {
        return n(
            "review",
            "launch gdd-review with a clean context",
            format!("gdd launch {id} review"),
        );
    };
    match r.verdict.as_str() {
        "stop" => {
            if !p.stop_told {
                return blocked(
                    "stopped",
                    "review-stop",
                    "the reviewer said stop: the change doesn't go in. Tell the person why; a stop is never waived, it gets a fix and a new review",
                    format!("gdd ask {id} --of stop \"<why it stopped>\""),
                );
            }
            round_needs_room(
                id,
                lim,
                n(
                    "build",
                    "a fix round after the reviewer's stop",
                    format!("gdd launch {id} build"),
                ),
            )
        }
        "fix" => round_needs_room(
            id,
            lim,
            n(
                "build",
                "a fix round with the reviewer's notes",
                format!("gdd launch {id} build"),
            ),
        ),
        _ => {
            if let Some(rc) = &r.color
                && crate::color_rank(rc) > crate::color_rank(&color)
            {
                return n(
                    "brief",
                    format!(
                        "the reviewer found {rc}, higher than {color}: back to Brief, and gdd-checks adds its cases"
                    ),
                    format!("gdd brief {id} --color {rc} --why \"<what raised it>\" <files>"),
                );
            }
            if asks("person-review") && !p.person_reviewed() {
                return waiting_or_ask(
                    id,
                    p,
                    "change",
                    &format!("the person reviews the change, which {color} asks for"),
                    &format!(
                        "gdd approve {id} --of change --version 1 --who <person> --where <pull request or conversation>"
                    ),
                    now,
                );
            }
            ship_step(id, git)
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
