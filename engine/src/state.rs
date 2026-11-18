//! A change's state: where its log stands ([`Progress`]), the order its steps may come in, and the
//! whole of it as a command takes it ([`derive`]), so no two commands can disagree about where a
//! change stands. It reads nothing itself: the log's entries and what git says are given to it.

use crate::blueprint::Blueprint;
use crate::judged::GitView;
use crate::limits::{self, Limits};
use crate::log::{self, Entry, kind, result, step, str_of, ts};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

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
    pub spent_limit: bool,
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

impl GatesRun {
    /// Whether these gates found what judges the change touched.
    pub fn seal_failed(&self) -> bool {
        self.failed.iter().any(|c| c == "seal")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub line: usize,
    pub verdict: String,
    pub head: Option<String>,
    pub start: Option<String>,
    pub color: Option<String>,
}

/// Keep the files that were checks when the reviewer returned them. A reseal
/// can bring new files, including product changes from the main branch.
#[derive(Debug, Clone)]
pub struct ChecksFix {
    pub line: usize,
    pub head: Option<String>,
    pub paths: BTreeSet<String>,
    // Earlier versions could record the builder immediately after this verdict.
    recorded_route: bool,
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

/// Whether a step may be written: nothing, or the reason of the refusal and what is wrong.
pub type Allowed = Result<(), (&'static str, String)>;

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
    /// Paths that defined what had to pass when the brief was last approved.
    approved_paths: Vec<String>,
    pub checks_done: bool,
    pub checks_stopped: i64,
    pub checks_line: Option<usize>,
    pub builds: i64,
    pub last_build: Option<(usize, bool)>,
    pub build_stop: Option<String>,
    pub last_gates: Option<GatesRun>,
    pub reviews: i64,
    pub last_review: Option<Review>,
    /// The review that returned only sealed checks, kept through the merge and its gates.
    pub checks_fix: Option<ChecksFix>,
    pub stop_told: bool,
    pub asks: Vec<Ask>,
    /// The person answered a spent-limit question without granting more room.
    pub limit_deferred: bool,
    pub shipped: Option<(usize, String)>,
    pub last_ts: Option<i64>,
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
    /// Whether the log knows a seal: a `seal` event, a brief step with its seal, or checks sealed.
    pub sealed_any: bool,
    /// Whether the person was told (an `ask --of stop`) that the last gates found the seal touched.
    pub seal_told: bool,
    /// Whether the seal moved since the last gates: a merge of the main branch into it, which the
    /// change's branch then takes in, is a new head no build step has to explain.
    pub seal_since_gates: bool,
    /// Spans of the seal, from a commit to a later one, that held work no step had recorded
    /// when a `seal` event moved `<start>` past them: a role's report still counts them.
    pub unrecorded: Vec<(String, String)>,
    /// The last checks step that sealed: its log line and the change's color then.
    pub checked: Option<(usize, Option<String>)>,
    /// Every check a checks step sealed, with its hash.
    pub checks_sealed: BTreeMap<String, String>,
}

pub(crate) fn hashes(v: &Value) -> BTreeMap<String, String> {
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

    /// Where a log stands: every event that counts, taken in its order.
    pub fn of(id: &str, entries: &[Entry]) -> Progress {
        let mut p = Progress::new(id);
        for (line, v) in log::events(entries) {
            p.apply(line, v);
        }
        p
    }

    pub fn findings_on_checks(&self, findings: &Value) -> bool {
        let Some(items) = findings.as_array() else {
            return false;
        };
        !items.is_empty()
            && items.iter().all(|f| {
                let place = str_of(f, "where").trim_start_matches("./");
                self.checks_sealed.contains_key(place)
                    || place.rsplit_once(':').is_some_and(|(path, line)| {
                        line.parse::<usize>().is_ok() && self.checks_sealed.contains_key(path)
                    })
            })
    }

    pub fn needs_checks_fix(&self) -> bool {
        self.checks_fix.is_some() && self.last_review.is_some()
    }

    pub fn merging_checks_fix(&self) -> bool {
        self.checks_fix.is_some() && !self.built_since_seal
    }

    pub fn has_brief(&self) -> bool {
        !self.brief_files.is_empty()
    }

    /// Callers read the version they are judging: the seal for proofs, the working copy for metrics.
    pub(crate) fn scenario_count(
        &self,
        id: &str,
        read: impl Fn(&str) -> Option<String>,
    ) -> Option<usize> {
        if let Some(text) = read(&format!("gdd/briefs/{id}.md"))
            && let Some((front, _)) = crate::blueprint::split_front_matter(&text)
            && let Ok(brief) = crate::blueprint::read_yaml(front)
            && let Some(scenarios) = brief["scenarios"].as_array()
        {
            return Some(scenarios.len());
        }
        let tag = format!("@brief:{id}");
        let count = self
            .brief_files
            .keys()
            .filter(|file| file.starts_with("features/"))
            .filter_map(|file| read(file))
            .map(|text| text.split_whitespace().filter(|word| *word == tag).count())
            .sum();
        (count > 0).then_some(count)
    }

    fn requirements(&self) -> impl Iterator<Item = (&String, &String)> {
        let has_scenarios = self.brief_files.keys().any(|p| p.starts_with("features/"));
        self.brief_files
            .iter()
            .filter(move |(p, _)| !has_scenarios || p.starts_with("features/"))
    }

    /// A version or waiver does not change approval. Adding or removing a scenario file, or
    /// changing one of its approved hashes, does.
    pub fn approved(&self) -> bool {
        let Some(brief_line) = self.brief_line else {
            return false;
        };
        let mut briefs = self.approvals.iter().filter(|a| a.1 == "brief");
        if self.brief_files.is_empty() {
            return briefs.any(|a| a.0 > brief_line);
        }
        if !self.requirements().map(|(p, _)| p).eq(&self.approved_paths) {
            return false;
        }
        let seen: BTreeMap<_, _> = briefs.flat_map(|a| &a.2).collect();
        self.requirements()
            .all(|(path, hash)| seen.get(path) == Some(&hash))
    }

    /// Whether the person approved the change itself after its last build.
    pub fn person_reviewed(&self) -> bool {
        let after = self.last_build.map(|b| b.0).unwrap_or(0);
        self.approvals
            .iter()
            .any(|a| a.1 == "change" && a.0 > after)
    }

    /// Whether anyone built the brief as it now is: a build written after its last brief step.
    /// A build of an earlier version doesn't count, because a new version is a brief nobody built.
    pub fn built_since_brief(&self) -> bool {
        matches!(
            (self.last_build, self.brief_line),
            (Some((built, _)), Some(brief)) if built > brief
        )
    }

    pub fn open_ask(&self) -> Option<&Ask> {
        self.asks.last().filter(|a| a.ended_by.is_none())
    }

    /// Take one event, in the order of the log.
    pub fn apply(&mut self, line: usize, v: &Value) {
        self.events += 1;
        let t = ts(v);
        if let Some(t) = t {
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
            if a.spent_limit && k == "answer" {
                self.limit_deferred = true;
            }
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
                    && self.last_gates.as_ref().is_some_and(GatesRun::seal_failed)
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
                    spent_limit: v["limits"].is_array() && str_of(v, "of") == "waiver",
                });
            }
            "waiver" if v["limit"].is_string() => self.limit_deferred = false,
            "approval" => {
                let of = str_of(v, "of");
                if of == "brief" {
                    self.approved_paths = self.requirements().map(|(p, _)| p.clone()).collect();
                }
                self.approvals.push((line, of.into(), hashes(&v["files"])));
            }
            "seal" => {
                self.sealed_any = true;
                self.seal_since_gates = true;
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
                self.limit_deferred = false;
                self.briefs += 1;
                self.checks_fix = None;
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
                        self.checked = Some((line, self.color.clone()));
                    }
                    for (p, h) in hashes(&v["seal"]["checks"]) {
                        self.sealed.insert(p.clone(), h.clone());
                        self.checks_sealed.insert(p, h);
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
                self.seal_told = false;
                self.seal_since_gates = false;
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
                self.checks_fix = (self.kind == Some(LogKind::Change)
                    && v["verdict"] == "fix"
                    && self.findings_on_checks(&v["findings"]))
                .then(|| ChecksFix {
                    line,
                    head: v["color"]["head"].as_str().map(str::to_owned),
                    paths: self.checks_sealed.keys().cloned().collect(),
                    recorded_route: v["fix_role"] == "checks",
                });
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
    pub fn allows(&self, bp: Option<&Blueprint>, v: &Value) -> Allowed {
        if log::kind(v) != "step" {
            return Ok(());
        }
        self.allows_step(bp, step(v), v["head"].as_str())
    }

    /// Whether a step may be written next. `head` is the commit a gates step would judge.
    pub fn allows_step(&self, bp: Option<&Blueprint>, st: &str, head: Option<&str>) -> Allowed {
        let kind = self.kind.unwrap_or(LogKind::Change);
        if kind == LogKind::Blueprint {
            return Ok(());
        }
        if self
            .checks_fix
            .as_ref()
            .is_some_and(|fix| fix.recorded_route)
        {
            if st == "build" && self.needs_checks_fix() {
                return Err((
                    "out-of-order",
                    "the checks agent corrects its sealed tests first".into(),
                ));
            }
            if st == "gates" && self.merging_checks_fix() {
                return Err((
                    "out-of-order",
                    "the builder reports the merge of the corrected seal before its gates".into(),
                ));
            }
        }
        self.held_by_what_sent_it_back(kind, st, head)?;
        let color = self.color.clone().unwrap_or_default();
        let asks =
            |check: &str| bp.is_some_and(|b| !color.is_empty() && b.color_asks(&color, check));
        if self.shipped.is_some() {
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
            "ship" => self.may_ship(kind, asks("person-review"), &color)?,
            _ => {}
        }
        Ok(())
    }

    /// A review that sent the work back stands until a new round answers it, and so do gates that
    /// failed: until then nothing judges the same work again, and what stopped goes to the person
    /// before the round starts. These rules were only in what `next` names, and a step written
    /// without asking `next` went around them.
    fn held_by_what_sent_it_back(&self, kind: LogKind, st: &str, head: Option<&str>) -> Allowed {
        let sent_back = self.last_review.as_ref().filter(|r| r.verdict != "pass");
        let stop_untold = sent_back.is_some_and(|r| r.verdict == "stop") && !self.stop_told;
        let seal_untold =
            self.last_gates.as_ref().is_some_and(GatesRun::seal_failed) && !self.seal_told;
        let unanswered = self.open_ask().is_some_and(|a| a.of == "stop");
        // A new brief and new checks take a review and its gates out of the state, as a build
        // does: none of them is written before the person hears what stopped.
        if matches!(st, "brief" | "checks" | "build") {
            if stop_untold {
                return Err((
                    "out-of-order",
                    "a new round after a stop, before the person was told: a stop is never waived"
                        .into(),
                ));
            }
            if seal_untold {
                return Err((
                    "out-of-order",
                    "a new round after the gates found what judges the change touched, before the person was told".into(),
                ));
            }
            if unanswered {
                return Err((
                    "needs-person",
                    "a new round while the person hasn't answered what stopped the change".into(),
                ));
            }
        }
        // Gates that failed are answered by a round, and a round has a build step. On the same
        // commit they may run again; on another one, something moved the branch, and only the seal
        // taking the main branch in does that without a builder.
        if st == "gates"
            && kind == LogKind::Change
            && !self.seal_since_gates
            && let Some(g) = self.last_gates.as_ref().filter(|g| !g.passed)
            && let (Some(was), Some(now)) = (g.head.as_deref(), head)
            && was != now
        {
            return Err((
                "out-of-order",
                format!(
                    "gates on {}, after gates that failed on {}, with no build step since: the round that moved the branch is reported before it is judged",
                    crate::git::short(now),
                    crate::git::short(was)
                ),
            ));
        }
        if st == "gates" && kind == LogKind::Change && seal_untold {
            return Err((
                "out-of-order",
                "gates again after they found what judges the change touched, before the person was told".into(),
            ));
        }
        if let Some(r) = sent_back
            && (st == "review" || st == "gates" && kind == LogKind::Change)
        {
            let needs = if r.verdict == "stop" {
                "the person is told, then a fix round"
            } else {
                "a fix round comes first"
            };
            return Err((
                "out-of-order",
                format!(
                    "{} after a review that said {}, with nothing built or sealed since: {needs}",
                    if st == "review" { "a review" } else { "gates" },
                    r.verdict
                ),
            ));
        }
        Ok(())
    }

    /// A change ships when its gates and its review passed on the same work, and after the
    /// person's review where its color asks for one; a setup log, when its trial passed.
    fn may_ship(&self, kind: LogKind, person_reviews: bool, color: &str) -> Allowed {
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
        if person_reviews && !self.person_reviewed() {
            return Err((
                "needs-approval",
                format!("a ship before the person reviewed the change, which {color} asks for"),
            ));
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
    let p = Progress::of(id, entries);
    // A closed log counts to its last event; an open one to now.
    let until = if p.shipped.is_some() {
        p.last_ts.unwrap_or(now)
    } else {
        now
    };
    let lim = limits::count_entries(bp, id, entries, until);
    let passed_on_head = match (&p.last_gates, &git.head) {
        (Some(g), Some(h)) if g.passed && g.head.as_ref().is_none_or(|x| git.stands_for(x, h)) => {
            g.ran.clone()
        }
        (Some(g), None) if g.passed => g.ran.clone(),
        _ => vec![],
    };
    let next = crate::next::next_step(id, bp, &p, &lim, &git, now);
    State {
        id: id.into(),
        progress: p,
        limits: lim,
        next,
        git,
        passed_on_head,
    }
}
