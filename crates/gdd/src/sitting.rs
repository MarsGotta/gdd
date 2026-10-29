//! The sitting of `gdd sitting <change>`: the screen where the human approves a change before it
//! is built, or asks for changes. It shows the scenarios as the branch of the change has them,
//! the red zones waiting for approval and the plan of a red change. The sitting is a state
//! machine, keys in and a screen out ([`Sitting`]), drawn with Ratatui. [`in_terminal`] shows it
//! in the terminal of the process. Nothing is written before the human confirms with `y`.

use std::cell::Cell;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use gdd_core::change;
use gdd_core::changelog;
use gdd_core::classify::SPIKE;
use gdd_core::flow::{self, Flow};
use gdd_core::gates::approve;
use gdd_core::git::Git;
use gdd_core::inbox;
use gdd_core::log;
use gdd_core::recipe;
use gdd_core::state;
use gdd_core::status::{self, Card};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};
use serde_json::Value;

use crate::tui::{self, Key, Screen};

/// The keys of the page of a change that is open.
const KEYS: &str = "↑↓ scroll · a approve · c ask for changes · q quit";

/// The keys of the G4 page.
const G4_KEYS: &str = "↑↓ scroll · o OK · k KO · p polish · q quit";

/// What the confirmation of a G4 answer asks.
const RUN: &str = "y to run · n to go back";

/// The state in which a change waits for the G4 of the human.
const IN_PRE: &str = "in-pre";

/// What the G4 page says when the review has no verdict.
const NO_REVIEW: &str = "Reviewer: no review yet";

/// What the G4 page says when the review does not say how the change works.
const UNEXPLAINED: &str = "How it works: the Reviewer did not explain the change";

/// What the G4 page says when the change has no demo files.
const NO_DEMO: &str = "Demo: none, the project has no demo command";

/// What the G4 page says when the record has no changelog line.
const NO_CHANGELOG: &str = "Changelog: none set";

/// What the G4 page of a spike says where another change has its review, its demo and its
/// changelog line.
const SPIKE_PAGE: &str = "Spike: no Reviewer, demo or changelog line. OK brings only its ADR \
                          files under docs/adr/ to main; its code stays on its branch.";

/// The keys of a page that only scrolls.
const SCROLL_KEYS: &str = "↑↓ scroll · q quit";

/// The keys of the text line.
const ASK_KEYS: &str = "Enter to continue · Backspace erases · Esc to go back";

/// What a confirmation asks.
const CONFIRM: &str = "y to record · n to go back";

/// The question of the text line.
const QUESTION: &str = "What should change?";

/// What the page says when it changed after the human read it.
const CHANGED: &str = "This changed after you read it: read it again before approving";

/// The note of every decision the sitting records, but the words of a KO.
const NOTE: &str = "sitting";

/// The state in which a change still has its sitting ahead.
const OPEN: &str = "open";

/// The color whose changes have a plan.
const RED: &str = "red";

/// A scenario file of the change, as its branch has it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scenario {
    path: String,
    /// Whether it is a hidden example: the Builders never see it.
    hidden: bool,
    text: String,
}

/// What approving records, as the confirmation shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Approval {
    /// The scenario files G1 seals, as they were shown; none, no G1.
    sealing: Vec<Scenario>,
    /// How many red zones wait for approval, when any do.
    zones: Option<usize>,
    /// Whether the plan of a red change waits for approval.
    plan: bool,
}

/// The page the human is on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    /// The scenarios, the red zones and the plan.
    Main,
    /// The confirmation of `a`.
    Approve,
    /// The text line of `c`, with what was typed.
    Ask(String),
    /// The confirmation of the words typed.
    Confirm(String),
    /// What was recorded, or why not.
    Done(Vec<String>),
    /// The text line of `k` and `p` at G4, with what was typed.
    Reply(Answer, String),
    /// The confirmation of a G4 answer, with its words (none for OK).
    Run(Answer, String),
}

/// What the human answers at G4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Ok,
    Ko,
    Polish,
}

impl Answer {
    /// The word `gdd resume --g4` takes.
    fn word(self) -> &'static str {
        match self {
            Answer::Ok => "ok",
            Answer::Ko => "ko",
            Answer::Polish => "polish",
        }
    }

    /// What follows the answer, as the confirmation says it; to a spike, which is never promoted
    /// and has no demo, when it is not the same.
    fn follows(self, spike: bool) -> &'static [&'static str] {
        match (self, spike) {
            (Answer::Ok, false) => &["The change is promoted."],
            (Answer::Ok, true) => &[
                "Only its ADR files reach main, in a commit of their own.",
                "Its code stays on its branch.",
            ],
            (Answer::Ko, _) => &["The change is rejected and stays out of production."],
            (Answer::Polish, false) => &[
                "One more Builder round with your words, then the gates and the demo again.",
                "The agents run now: it takes minutes.",
            ],
            (Answer::Polish, true) => &[
                "One more Builder round with your words, then its G2 again.",
                "The agents run now: it takes minutes.",
            ],
        }
    }
}

/// A thing the sitting records, with the arguments of the `gdd` command that records it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    args: Vec<String>,
    /// Whether it runs from a throwaway worktree of the branch of the change.
    on_branch: bool,
}

/// The sitting of a change: what it shows and the page the human is on.
#[derive(Debug)]
pub struct Sitting {
    project: String,
    id: String,
    root: PathBuf,
    /// The environment the decisions are recorded in: the one of the process.
    env: Vec<(String, String)>,
    /// The main page, framed by the title.
    lines: Vec<Line<'static>>,
    /// What `a` records; `None` once the sitting is over.
    approval: Option<Approval>,
    page: Page,
    /// Whether what was shown changed before `y`, so the human reads the page again.
    stale: bool,
    /// How many rows the main page or the result is scrolled up.
    scroll: u16,
    /// The room the body had when it was last painted; the pages scroll within it.
    body: Cell<Rect>,
    closed: bool,
    /// Whether the change waits for its G4: the page answers it instead of approving.
    g4: bool,
    /// Whether the change is a spike, whose G4 brings only its ADR files to main.
    spike: bool,
    /// The `gdd` arguments the human chose with `y` at G4.
    chosen: Option<Vec<String>>,
}

impl Sitting {
    /// A sitting of the change `id` of the project `project` at `root`, with `lines` on its main
    /// page and `approval` to record, in the environment of the process.
    fn new(
        project: &str,
        id: &str,
        root: &Path,
        lines: Vec<Line<'static>>,
        approval: Option<Approval>,
    ) -> Sitting {
        Sitting {
            project: project.to_string(),
            id: id.to_string(),
            root: root.to_path_buf(),
            env: crate::process_env(),
            lines,
            approval,
            page: Page::Main,
            stale: false,
            scroll: 0,
            body: Cell::new(Rect::new(0, 0, 80, 20)),
            closed: false,
            g4: false,
            spike: false,
            chosen: None,
        }
    }

    /// The sitting of the change `change` of the project that holds the folder `cwd`, as every
    /// command finds the project and its changes; an unknown change is an error naming it.
    pub fn load(cwd: &Path, change: &str) -> Result<Sitting, String> {
        let root = crate::project_root(cwd)?;
        let state = state::root(&root)?;
        let project = crate::read(&root, crate::PROJECT)?.unwrap_or_default();
        let basics =
            status::Basics::from_toml(&project).map_err(|e| format!("{}: {e}", crate::PROJECT))?;
        let card = card_of(&root, &state, change)?;
        if card.state == IN_PRE {
            let lines = g4_lines(&state, &card)?;
            let mut sitting = Sitting::new(&basics.name, &card.id, &root, lines, None);
            sitting.g4 = true;
            sitting.spike = card.kind == SPIKE;
            return Ok(sitting);
        }
        let title = format!("{} · {} · {}", card.id, card.title, card.color);
        let mut lines = vec![Line::from(title).bold(), Line::default()];
        if card.state != OPEN {
            let over = format!("{} is {}: its sitting is over", card.id, card.state);
            lines.push(Line::from(over));
            return Ok(Sitting::new(&basics.name, &card.id, &root, lines, None));
        }
        let scenarios = scenarios(&root, &card.id)?;
        let (zone_lines, zones) = red_zones(&root, &state)?;
        let red = card.color == RED;
        lines.extend(scenario_lines(&card.id, &scenarios));
        lines.extend(zone_lines);
        lines.push(Line::default());
        lines.extend(plan_lines(&state, &card, red)?);
        let approval = Approval {
            sealing: scenarios,
            zones,
            plan: red,
        };
        let sitting = Sitting::new(&basics.name, &card.id, &root, lines, Some(approval));
        Ok(sitting)
    }

    /// The human presses `key`. Ctrl-C closes the sitting on any page; once closed, keys do
    /// nothing. Nothing is written but by `y` on a confirmation.
    pub fn press(&mut self, key: Key) {
        if self.closed {
            return;
        }
        if key == Key::CtrlC {
            self.closed = true;
            return;
        }
        match self.page {
            Page::Main => self.on_main(key),
            Page::Approve => self.on_approve(key),
            Page::Ask(_) => self.on_ask(key),
            Page::Confirm(_) => self.on_confirm(key),
            Page::Done(_) => self.on_done(key),
            Page::Reply(..) => self.on_reply(key),
            Page::Run(..) => self.on_run(key),
        }
    }

    /// The screen as text, `width` x `height` cells, its lines joined by `\n`.
    pub fn render(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        self.paint(area, &mut buffer);
        tui::screen_text(&buffer)
    }

    /// Whether the human closed the sitting.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// The `gdd` arguments the human chose with `y` at G4; none until then.
    pub fn chosen(&self) -> Option<Vec<String>> {
        self.chosen.clone()
    }

    /// The main page: Up and Down scroll, `a` and `c` ask, while the sitting is not over, `o`, `k`
    /// and `p` answer at G4, and `q` closes.
    fn on_main(&mut self, key: Key) {
        let open = self.approval.is_some();
        match key {
            Key::Char('q') => self.closed = true,
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            Key::Char('a') if open => self.go_to(Page::Approve),
            Key::Char('c') if open => self.go_to(Page::Ask(String::new())),
            Key::Char('o') if self.g4 => self.go_to(Page::Run(Answer::Ok, String::new())),
            Key::Char('k') if self.g4 => self.go_to(Page::Reply(Answer::Ko, String::new())),
            Key::Char('p') if self.g4 => self.go_to(Page::Reply(Answer::Polish, String::new())),
            _ => {}
        }
    }

    /// The confirmation of `a`: Up and Down scroll it, `y` records, `n` and Esc go back, `q`
    /// closes.
    fn on_approve(&mut self, key: Key) {
        match key {
            Key::Char('y') => self.record_approval(),
            Key::Char('n') | Key::Esc => self.go_to(Page::Main),
            Key::Char('q') => self.closed = true,
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            _ => {}
        }
    }

    /// The text line: signs and Space type, Backspace erases, Enter with some text confirms and
    /// Esc goes back.
    fn on_ask(&mut self, key: Key) {
        let Page::Ask(text) = &mut self.page else {
            return;
        };
        match key {
            Key::Enter if !text.trim().is_empty() => {
                let words = text.trim().to_string();
                self.go_to(Page::Confirm(words));
            }
            Key::Esc => self.go_to(Page::Main),
            typed => type_into(text, typed),
        }
    }

    /// The text line of a G4 answer: as [`Sitting::on_ask`], and Enter goes on to the
    /// confirmation of the answer.
    fn on_reply(&mut self, key: Key) {
        let Page::Reply(answer, text) = &mut self.page else {
            return;
        };
        let answer = *answer;
        match key {
            Key::Enter if !text.trim().is_empty() => {
                let words = text.trim().to_string();
                self.go_to(Page::Run(answer, words));
            }
            Key::Esc => self.go_to(Page::Main),
            typed => type_into(text, typed),
        }
    }

    /// The confirmation of a G4 answer: Up and Down scroll it, `y` chooses what runs, `n` and Esc
    /// go back, `q` closes.
    fn on_run(&mut self, key: Key) {
        match key {
            Key::Char('y') => self.choose(),
            Key::Char('n') | Key::Esc => self.go_to(Page::Main),
            Key::Char('q') => self.closed = true,
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            _ => {}
        }
    }

    /// `y` on the confirmation of a G4 answer: when the change still waits for its G4, chooses
    /// `gdd resume` and closes the screen; else says what state it is in and chooses nothing.
    fn choose(&mut self) {
        let Page::Run(answer, words) = &self.page else {
            return;
        };
        let (answer, words) = (*answer, words.clone());
        match self.current_state() {
            Ok(state) if state == IN_PRE => {
                self.chosen = Some(resume_args(&self.id, answer, &words));
                self.closed = true;
            }
            Ok(state) => {
                let now = format!("{} is {state} now: nothing was run", self.id);
                self.go_to(Page::Done(vec![now]));
            }
            Err(message) => self.go_to(Page::Done(refused_lines(Vec::new(), &message))),
        }
    }

    /// The state of the change as its record has it now.
    fn current_state(&self) -> Result<String, String> {
        let state = state::root(&self.root)?;
        card_of(&self.root, &state, &self.id).map(|card| card.state)
    }

    /// The confirmation of the words: Up and Down scroll it, `y` records, `n` and Esc go back,
    /// `q` closes.
    fn on_confirm(&mut self, key: Key) {
        match key {
            Key::Char('y') => self.record_changes(),
            Key::Char('n') | Key::Esc => self.go_to(Page::Main),
            Key::Char('q') => self.closed = true,
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            _ => {}
        }
    }

    /// What was recorded: Up and Down scroll it, `q` closes.
    fn on_done(&mut self, key: Key) {
        match key {
            Key::Char('q') => self.closed = true,
            Key::Up => self.scroll_up(),
            Key::Down => self.scroll_down(),
            _ => {}
        }
    }

    /// Shows `page` from its top.
    fn go_to(&mut self, page: Page) {
        self.page = page;
        self.scroll = 0;
        self.stale = false;
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        let next = self.scroll.saturating_add(1);
        self.scroll = next.min(self.scroll_limit());
    }

    /// How many rows the page can scroll up before its last row is at the bottom of the body as
    /// last painted.
    fn scroll_limit(&self) -> u16 {
        let (lines, _) = self.page_lines();
        limit(&lines, self.body.get())
    }

    /// Whether the page scrolls.
    fn scrolls(&self) -> bool {
        !matches!(self.page, Page::Ask(_) | Page::Reply(..))
    }

    /// The lines of the page the human is on, and its keys.
    fn page_lines(&self) -> (Vec<Line<'static>>, &'static str) {
        match &self.page {
            Page::Main if self.g4 => (self.main_lines(), G4_KEYS),
            Page::Main if self.approval.is_some() => (self.main_lines(), KEYS),
            Page::Reply(_, text) => {
                let typed = Line::from(format!("> {text}_"));
                (vec![Line::from(QUESTION), typed], ASK_KEYS)
            }
            Page::Run(answer, words) => {
                let text = run_text(&self.id, *answer, words, self.spike);
                (plain(&text), SCROLL_KEYS)
            }
            Page::Main => (self.main_lines(), SCROLL_KEYS),
            Page::Approve => (plain(&self.approval_text()), SCROLL_KEYS),
            Page::Ask(text) => {
                let typed = Line::from(format!("> {text}_"));
                (vec![Line::from(QUESTION), typed], ASK_KEYS)
            }
            Page::Confirm(text) => {
                let ko = format!("G1 ko on {}: {text}", self.id);
                (plain(&[ko, CONFIRM.to_string()]), SCROLL_KEYS)
            }
            Page::Done(done) => (plain(done), SCROLL_KEYS),
        }
    }

    /// The lines of the main page, with a warning above them when they changed under the human.
    fn main_lines(&self) -> Vec<Line<'static>> {
        if !self.stale {
            return self.lines.clone();
        }
        let warning = Line::from(CHANGED).bold();
        let mut lines = vec![warning, Line::default()];
        lines.extend(self.lines.iter().cloned());
        lines
    }

    /// What the confirmation of `a` says it records.
    fn approval_text(&self) -> Vec<String> {
        let Some(approval) = &self.approval else {
            return Vec::new();
        };
        let id = &self.id;
        let mut text = vec!["This records, as you:".to_string()];
        if !approval.sealing.is_empty() {
            let paths: Vec<&str> = approval.sealing.iter().map(|s| s.path.as_str()).collect();
            let sealing = paths.join(", ");
            text.push(format!("- G1 ok on {id}, sealing {sealing}"));
        }
        if let Some(zones) = approval.zones {
            text.push(format!("- red zones ok: {zones} red zone(s)"));
        }
        if approval.plan {
            text.push(format!("- plan ok on {id}"));
        }
        text.push(CONFIRM.to_string());
        text
    }

    /// Paints the screen on `buffer`, framed with the title, the keys at the bottom.
    fn paint(&self, area: Rect, buffer: &mut Buffer) {
        let title = format!(" gdd · Sitting · {} ", self.project);
        let block = Block::bordered().title(title);
        let inner = block.inner(area);
        block.render(area, buffer);
        let [body, keys] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
        self.body.set(body);
        let (lines, keys_text) = self.page_lines();
        let offset = if self.scrolls() {
            self.scroll.min(limit(&lines, body))
        } else {
            0
        };
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((offset, 0))
            .render(body, buffer);
        Paragraph::new(Line::from(keys_text).dim()).render(keys, buffer);
    }

    /// `y` on the confirmation of `a`: records G1 from the branch of the change, the red zones
    /// when they wait and the plan of a red change.
    fn record_approval(&mut self) {
        if !self.is_as_shown() {
            return;
        }
        let Some(approval) = &self.approval else {
            return;
        };
        let id = self.id.as_str();
        let mut steps = Vec::new();
        if !approval.sealing.is_empty() {
            steps.push(Step {
                args: decision(&["gate", "G1", "ok", id], NOTE),
                on_branch: true,
            });
        }
        if approval.zones.is_some() {
            steps.push(Step {
                args: decision(&["gate", "zones", "ok"], NOTE),
                on_branch: false,
            });
        }
        if approval.plan {
            steps.push(Step {
                args: decision(&["gate", "plan", "ok", id], NOTE),
                on_branch: false,
            });
        }
        let next = format!("Next: gdd launch {id}");
        self.record(&steps, Some(next));
    }

    /// The sitting as the project has it now; when it cannot be read, the result says why.
    fn reload(&mut self) -> Option<Sitting> {
        match Sitting::load(&self.root, &self.id) {
            Ok(now) => Some(now),
            Err(message) => {
                self.go_to(Page::Done(refused_lines(Vec::new(), &message)));
                None
            }
        }
    }

    /// Whether what the human read is what the project has now: the same page and the same
    /// decisions to record. When not, nothing is recorded: the sitting shows what it has now,
    /// with a warning, and waits for the human to read it again; when it cannot be read, the
    /// result says why.
    fn is_as_shown(&mut self) -> bool {
        let Some(now) = self.reload() else {
            return false;
        };
        if now.lines == self.lines && now.approval == self.approval {
            return true;
        }
        self.lines = now.lines;
        self.approval = now.approval;
        self.go_to(Page::Main);
        self.stale = true;
        false
    }

    /// Whether the change is still in its sitting, as the human left it. When it is not, nothing
    /// is recorded: the sitting shows that it is over, with a warning; when the project cannot
    /// be read, the result says why.
    fn is_still_open(&mut self) -> bool {
        let Some(now) = self.reload() else {
            return false;
        };
        if now.approval.is_some() {
            return true;
        }
        self.lines = now.lines;
        self.approval = None;
        self.go_to(Page::Main);
        self.stale = true;
        false
    }

    /// `y` on the confirmation of the words: records G1 KO with them.
    fn record_changes(&mut self) {
        if !self.is_still_open() {
            return;
        }
        let Page::Confirm(words) = &self.page else {
            return;
        };
        let step = Step {
            args: decision(&["gate", "G1", "ko", &self.id], words),
            on_branch: false,
        };
        self.record(&[step], None);
    }

    /// Records `steps` in order, stopping at the first that fails, and shows what happened;
    /// `next` is what to do after them.
    fn record(&mut self, steps: &[Step], next: Option<String>) {
        let mut recorded = Vec::new();
        let mut failed = None;
        for step in steps {
            let said = if step.on_branch {
                self.run_on_branch(&step.args)
            } else {
                gate(&step.args, &self.root, &self.env)
            };
            match said {
                Ok(lines) => recorded.extend(lines),
                Err(message) => {
                    failed = Some(message);
                    break;
                }
            }
        }
        let shown = match failed {
            None => recorded_lines(recorded, next),
            Some(message) => refused_lines(recorded, &message),
        };
        self.go_to(Page::Done(shown));
    }

    /// Runs the command `args` from a throwaway worktree of the branch of the change, so the main
    /// checkout never changes branch, and removes the worktree whatever the command said.
    fn run_on_branch(&self, args: &[String]) -> Result<Vec<String>, String> {
        let name = format!("gdd-sitting-{}-{}", self.id, std::process::id());
        let dir = std::env::temp_dir().join(name);
        if dir.exists() {
            return Err(format!("cannot use {}: it exists already", dir.display()));
        }
        let git = Git::new(&self.root);
        let branch = change::branch(&self.id);
        let added = git.add_worktree(&dir, &branch).map_err(|e| e.to_string());
        // The project may be a folder of the repository: the command runs in the same folder.
        let work = added.and_then(|()| recipe::worktree::project_root(&self.root, &dir));
        let work = work.and_then(|work| self.pinned(&work).map(|()| work));
        let said = work.and_then(|work| gate(args, &work, &self.env));
        with_cleanup(said, remove_worktree(&git, &dir))
    }

    /// Whether the scenario files in the project folder `work` of the worktree are the ones the
    /// human read, in case the branch moved after the page was checked.
    fn pinned(&self, work: &Path) -> Result<(), String> {
        let shown = self.approval.iter().flat_map(|a| &a.sealing);
        let now = scenarios_at(work, "HEAD")?;
        for scenario in shown.clone() {
            if !now.contains(scenario) {
                return Err(format!("{} changed after you read it", scenario.path));
            }
        }
        match now
            .iter()
            .find(|file| !shown.clone().any(|s| s.path == file.path))
        {
            Some(file) => Err(format!("{} was added after you read it", file.path)),
            None => Ok(()),
        }
    }
}

/// Types `key` into `text`: signs and Space type, Backspace erases, the other keys do nothing.
fn type_into(text: &mut String, key: Key) {
    match key {
        Key::Char(letter) => text.push(letter),
        Key::Space => text.push(' '),
        Key::Backspace => {
            text.pop();
        }
        _ => {}
    }
}

/// The arguments of `gdd resume` that give the answer `answer` at G4, with `words` for a KO or a
/// polish round.
fn resume_args(id: &str, answer: Answer, words: &str) -> Vec<String> {
    let mut args: Vec<String> = ["resume", id, "--g4", answer.word()]
        .map(str::to_string)
        .to_vec();
    if answer != Answer::Ok {
        args.extend(["--note".to_string(), words.to_string()]);
    }
    args
}

/// What the confirmation of a G4 answer says: the command that runs, what follows, to a `spike`
/// or not, and the keys.
fn run_text(id: &str, answer: Answer, words: &str, spike: bool) -> Vec<String> {
    let mut command = format!("gdd resume {id} --g4 {}", answer.word());
    if answer != Answer::Ok {
        command.push_str(&format!(" --note \"{words}\""));
    }
    let mut text = vec!["This runs, as you:".to_string(), command];
    text.extend(answer.follows(spike).iter().map(ToString::to_string));
    text.push(RUN.to_string());
    text
}

/// The G4 page of the change `card`, whose state root is `state`: how the change works and what
/// else the Reviewer said, the demo, what was decided without the human, the changelog line and
/// the time of the change. A spike has no review, demo or changelog line: what OK brings
/// instead (see [`SPIKE_PAGE`]).
fn g4_lines(state: &Path, card: &Card) -> Result<Vec<Line<'static>>, String> {
    let id = card.id.as_str();
    let title = format!("{id} · {} · waiting for your G4", card.title);
    let record = record_of(state, id)?;
    let spike = card.kind == SPIKE;
    let mut lines = vec![Line::from(title).bold(), Line::default()];
    if !spike {
        lines.extend(review_lines(state, id)?);
        lines.push(Line::default());
        lines.push(Line::from(demo_line(state, id)));
        lines.push(Line::default());
    }
    let decided = decided_lines(&record);
    if !decided.is_empty() {
        lines.extend(decided);
        lines.push(Line::default());
    }
    let last = if spike {
        SPIKE_PAGE.to_string()
    } else {
        changelog_line(&record)
    };
    lines.push(Line::from(last));
    let time = time_lines(state, id);
    if !time.is_empty() {
        lines.push(Line::default());
        lines.extend(time);
    }
    Ok(lines)
}

/// The record of the change `id` as JSON.
fn record_of(state: &Path, id: &str) -> Result<Value, String> {
    let text = change::read_record(state, id)?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", change::record_path(id)))
}

/// The review of the change `id`: how the change works (see [`inbox::how_it_works`]), then its
/// verdict and its other lines; or that there is none.
fn review_lines(state: &Path, id: &str) -> Result<Vec<Line<'static>>, String> {
    let path = format!(".gdd/changes/{id}/review.md");
    let text = crate::read(state, &path)?.unwrap_or_default();
    let (explained, others) = inbox::how_it_works(&text);
    let mut lines = explained_lines(&explained);
    lines.push(Line::default());
    let Some(verdict) = verdict_of(&text) else {
        lines.push(Line::from(NO_REVIEW));
        return Ok(lines);
    };
    lines.push(Line::from(format!("Reviewer: {verdict}")));
    let others: Vec<String> = others
        .into_iter()
        .filter(|line| !line.trim().starts_with(VERDICT))
        .map(spaced)
        .collect();
    lines.extend(plain(&others));
    Ok(lines)
}

/// The `How it works` section of the G4 page: the lines of the review that say how the change
/// works; without them, that the Reviewer did not explain it.
fn explained_lines(explained: &[&str]) -> Vec<Line<'static>> {
    if explained.is_empty() {
        return vec![Line::from(UNEXPLAINED)];
    }
    let mut lines = vec![Line::from(inbox::HOW_IT_WORKS).bold()];
    let explained: Vec<String> = explained.iter().copied().map(spaced).collect();
    lines.extend(plain(&explained));
    lines
}

/// What a line of the verdict of a review starts with.
const VERDICT: &str = "VERDICT:";

/// The `OK` or `KO` of the last `VERDICT:` line of the review `text`; none when it has no such
/// line or it says something else.
fn verdict_of(text: &str) -> Option<&str> {
    let said = text
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(VERDICT))?;
    Some(said.trim()).filter(|said| matches!(*said, "OK" | "KO"))
}

/// The `Demo:` line of the change `id`: the line with the count of scenarios of the first demo
/// file that has one, else the first line of the first file; with its path.
fn demo_line(state: &Path, id: &str) -> String {
    let dir = format!(".gdd/changes/{id}/demo");
    let names = match demo_files(&state.join(&dir)) {
        Ok(names) => names,
        Err(why) => return format!("Demo: {why}"),
    };
    let mut first = None;
    for name in names {
        let path = format!("{dir}/{name}");
        let text = match fs::read_to_string(state.join(&path)) {
            Ok(text) => text,
            Err(e) => return format!("Demo: cannot read {path}: {e}"),
        };
        if let Some(count) = text
            .lines()
            .map(str::trim)
            .find(|line| counts_scenarios(line))
        {
            return format!("Demo: {count} · {path}");
        }
        let head = text.lines().next().unwrap_or_default().to_string();
        first.get_or_insert((path, head));
    }
    match first {
        Some((path, head)) => format!("Demo: {head} · {path}"),
        None => NO_DEMO.to_string(),
    }
}

/// The names of the files in the demo folder `dir`, sorted; none when the folder is not there.
fn demo_files(dir: &Path) -> Result<Vec<String>, String> {
    let cannot = |e: io::Error| format!("cannot read {}: {e}", dir.display());
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(cannot(e)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(cannot)?;
        if entry.file_type().map_err(cannot)?.is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Whether `line` is `<n> scenarios (...)`.
fn counts_scenarios(line: &str) -> bool {
    let Some((count, rest)) = line.split_once(' ') else {
        return false;
    };
    let number = !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit());
    number && rest.starts_with("scenarios (") && rest.ends_with(')')
}

/// The `Decided without you` section: the notes of kind `decided` of the record that are not
/// resolved; none, no lines.
fn decided_lines(record: &Value) -> Vec<Line<'static>> {
    let notes = record["inbox"].as_array().into_iter().flatten();
    let open = notes.filter(|note| note["kind"] == "decided" && !is_resolved(&note["resolved"]));
    let decided: Vec<String> = open
        .filter_map(|note| note["text"].as_str())
        .map(|text| format!("- {text}"))
        .collect();
    if decided.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![Line::from("Decided without you").bold()];
    lines.extend(plain(&decided));
    lines
}

/// Whether the `resolved` field of a note says it was resolved.
fn is_resolved(resolved: &Value) -> bool {
    match resolved {
        Value::Null | Value::Bool(false) => false,
        Value::String(when) => !when.is_empty(),
        _ => true,
    }
}

/// The `Changelog:` line of the record: the line it sets, with `**Breaking:**` first when it is;
/// else that none is set.
fn changelog_line(record: &Value) -> String {
    let set = !record["changelog"].is_null() && record["changelog"]["none"] != true;
    if !set {
        return NO_CHANGELOG.to_string();
    }
    match changelog::of_change(record) {
        Ok(Some(line)) => {
            let mark = if line.breaking { "**Breaking:** " } else { "" };
            format!("Changelog: {} · {mark}{}", line.section, line.text)
        }
        Ok(None) => NO_CHANGELOG.to_string(),
        Err(why) => format!("Changelog: {why}"),
    }
}

/// The time of the change `id` as `gdd report --time --change` writes it, without its first line
/// and its indent; nothing when none was measured.
fn time_lines(state: &Path, id: &str) -> Vec<Line<'static>> {
    let log = crate::read(state, log::PATH)
        .and_then(|text| flow::read(&text.unwrap_or_default()).map_err(|e| e.to_string()));
    let log = match log {
        Ok(log) => log,
        Err(why) => return vec![Line::from(format!("the log cannot be read: {why}"))],
    };
    let flow = Flow::new(&log, Some(id), None);
    let measured = serde_json::to_value(&flow).ok();
    if !matches!(measured, Some(Value::Array(changes)) if !changes.is_empty()) {
        return Vec::new();
    }
    let text = flow.to_string();
    let rows = text.lines().skip(1);
    rows.map(|row| Line::from(row.strip_prefix("  ").unwrap_or(row).to_string()))
        .collect()
}

/// The card of the change `id`; else why there is none.
fn card_of(root: &Path, state: &Path, id: &str) -> Result<Card, String> {
    let card = status::cards(root, state)
        .into_iter()
        .find(|card| card.id == id);
    let unknown = || format!("unknown change {id}: no {}", change::record_path(id));
    let card = card.ok_or_else(unknown)?;
    match &card.problem {
        Some(problem) => Err(format!("{id}: {problem}")),
        None => Ok(card),
    }
}

/// The scenario files of the change `id` on its branch, visible ones first, by path: the ones it
/// adds or changes against the main branch, read as its last commit has them. None while the
/// branch does not exist.
fn scenarios(root: &Path, id: &str) -> Result<Vec<Scenario>, String> {
    let branch = change::branch(id);
    let exists = Git::new(root).exists(&branch).map_err(|e| e.to_string())?;
    if !exists {
        return Ok(Vec::new());
    }
    scenarios_at(root, &branch)
}

/// The scenario files that the revision `rev` adds or changes against the main branch, visible
/// ones first, by path, read as `rev` has them.
fn scenarios_at(root: &Path, rev: &str) -> Result<Vec<Scenario>, String> {
    let acceptance = crate::load_table(root)?.acceptance;
    let main = crate::load_settings(root)?.main_branch;
    let git = Git::new(root);
    let cannot = |e: gdd_core::git::Error| e.to_string();
    let changed = git.committed_from(&main, rev).map_err(cannot)?;
    let hidden_dir = format!("{}/hidden/", acceptance.dir);
    let mut found = Vec::new();
    for path in changed {
        if !acceptance.needs_approval(&path) {
            continue;
        }
        // A file the branch deleted has nothing to read.
        let Some(bytes) = git.file_at(rev, &path).map_err(cannot)? else {
            continue;
        };
        found.push(Scenario {
            hidden: path.starts_with(&hidden_dir),
            text: String::from_utf8_lossy(&bytes).into_owned(),
            path,
        });
    }
    found.sort_by(|a, b| (a.hidden, &a.path).cmp(&(b.hidden, &b.path)));
    Ok(found)
}

/// The `Scenarios` section: each file with its text.
fn scenario_lines(id: &str, scenarios: &[Scenario]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("Scenarios").bold()];
    if scenarios.is_empty() {
        let none = format!("No scenario files on {}", change::branch(id));
        lines.push(Line::from(none));
    }
    for scenario in scenarios {
        let path = if scenario.hidden {
            format!("{} · hidden: the Builders never see it", scenario.path)
        } else {
            scenario.path.clone()
        };
        lines.push(Line::from(path).bold());
        lines.extend(plain(&text_lines(&scenario.text)));
        lines.push(Line::default());
    }
    lines
}

/// The lines of a file's `text`, with its tabs as spaces so that they take room on the screen.
fn text_lines(text: &str) -> Vec<String> {
    text.lines().map(spaced).collect()
}

/// `line` with its tabs as spaces, so that they take room on the screen.
fn spaced(line: &str) -> String {
    line.replace('\t', "    ")
}

/// The section of the red zones of the project at `root`, whose state root is `state`, and how
/// many red zones wait for approval, when any do: `Red zones: approved` when the zone map is
/// approved as it is.
fn red_zones(root: &Path, state: &Path) -> Result<(Vec<Line<'static>>, Option<usize>), String> {
    let text = crate::read(root, crate::ZONES)?.unwrap_or_default();
    let cannot = |e: approve::Error| e.to_string();
    let zones = approve::zones_state(state, &text).map_err(cannot)?;
    if zones.approved() {
        return Ok((vec![Line::from("Red zones: approved")], None));
    }
    let red = approve::red_zones(&text).map_err(cannot)?;
    let mut lines = vec![Line::from("Red zones to approve").bold()];
    for zone in &red {
        let paths = zone.get("paths").and_then(|paths| paths.as_array());
        let paths: Vec<&str> = paths
            .into_iter()
            .flatten()
            .filter_map(|path| path.as_str())
            .collect();
        let reason = zone.get("reason").and_then(|reason| reason.as_str());
        let line = match reason {
            Some(reason) if !reason.is_empty() => format!("{} · {reason}", paths.join(", ")),
            _ => paths.join(", "),
        };
        lines.push(Line::from(line));
    }
    Ok((lines, Some(red.len())))
}

/// The `Plan` section of `card`: the plan written for a red change, else why there is none.
fn plan_lines(state: &Path, card: &Card, red: bool) -> Result<Vec<Line<'static>>, String> {
    if !red {
        let not_needed = format!("Plan: not needed for a {} change", card.color);
        return Ok(vec![Line::from(not_needed)]);
    }
    let path = format!(".gdd/changes/{}/plan.md", card.id);
    let Some(plan) = crate::read(state, &path)? else {
        return Ok(vec![Line::from("Plan: none written yet")]);
    };
    let mut lines = vec![Line::from("Plan").bold()];
    lines.extend(plain(&text_lines(&plan)));
    Ok(lines)
}

/// `lines` as lines of a page.
fn plain(lines: &[String]) -> Vec<Line<'static>> {
    lines.iter().map(|line| Line::from(line.clone())).collect()
}

/// The arguments of `gdd` that record the decision `words` as the human, with `note`.
fn decision(words: &[&str], note: &str) -> Vec<String> {
    let args = words.iter().copied();
    // `--note=<words>` in one piece: words that start with `-` are not read as an option.
    let note = format!("--note={note}");
    let args = args.chain(["--by", "human", note.as_str()]);
    args.map(str::to_string).collect()
}

/// Runs `gdd` with `args` from the folder `cwd` in the environment `env`, without printing: the
/// lines it says, else what it says to the standard error.
fn gate(args: &[String], cwd: &Path, env: &[(String, String)]) -> Result<Vec<String>, String> {
    let said = crate::run_with_env(args, cwd, env);
    if said.code == 0 {
        Ok(said.stdout.lines().map(str::to_string).collect())
    } else {
        Err(said.stderr.trim_end().to_string())
    }
}

/// Removes the working tree `dir` of the repository, with what it has that git does not, and
/// forgets the ones whose folders are gone.
fn remove_worktree(git: &Git, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        let cannot = |e: gdd_core::git::Error| format!("cannot remove {}: {e}", dir.display());
        git.remove_worktree(dir).map_err(cannot)?;
    }
    let cannot = |e: gdd_core::git::Error| format!("cannot prune the worktrees: {e}");
    git.prune_worktrees().map_err(cannot)
}

/// What a command said and what removing its worktree said, as one: a worktree left behind is
/// told after what the command said.
fn with_cleanup(
    said: Result<Vec<String>, String>,
    removed: Result<(), String>,
) -> Result<Vec<String>, String> {
    match (said, removed) {
        (said, Ok(())) => said,
        (Ok(mut lines), Err(left)) => {
            lines.push(format!("warning: {left}"));
            Ok(lines)
        }
        (Err(message), Err(left)) => Err(format!("{message}\n{left}")),
    }
}

/// The page after everything was recorded: what each decision said, then what comes next.
fn recorded_lines(recorded: Vec<String>, next: Option<String>) -> Vec<String> {
    let mut lines = vec!["Recorded".to_string()];
    lines.extend(recorded);
    if let Some(next) = next {
        lines.push(String::new());
        lines.push(next);
    }
    lines
}

/// The page after a decision failed with `message`: what was recorded before it, that nothing
/// more was, and why.
fn refused_lines(recorded: Vec<String>, message: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if recorded.is_empty() {
        lines.push("Nothing recorded".to_string());
    } else {
        lines.push("Recorded".to_string());
        lines.extend(recorded);
        lines.push("Nothing more recorded".to_string());
    }
    lines.push(String::new());
    lines.extend(message.lines().map(str::to_string));
    lines
}

/// How many rows `lines` scroll up before their last row is at the bottom of `body`.
fn limit(lines: &[Line], body: Rect) -> u16 {
    let total = rows(lines, body.width);
    let limit = total.saturating_sub(usize::from(body.height));
    u16::try_from(limit).unwrap_or(u16::MAX)
}

/// How many rows `lines` take in `width` cells once wrapped, without the empty ones at the end.
fn rows(lines: &[Line], width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    // A row holds at least one cell of a line, so a line never takes more rows than cells.
    let cells: usize = lines.iter().map(|line| line.width().max(1)).sum();
    let area = Rect::new(0, 0, width, u16::try_from(cells).unwrap_or(u16::MAX));
    let mut buffer = Buffer::empty(area);
    Paragraph::new(lines.to_vec())
        .wrap(Wrap { trim: false })
        .render(area, &mut buffer);
    let used = buffer
        .content
        .chunks(usize::from(width))
        .rposition(|row| row.iter().any(|cell| cell.symbol() != " "));
    used.map_or(0, |at| at + 1)
}

/// `gdd sitting <change>` in a terminal: shows the sitting of the change until the human closes
/// it, in raw mode on the alternate screen, which are put back as they were on every way out, a
/// panic too; else why it could not. Once the screen is done and the terminal is put back, what
/// the human chose at G4 runs in the terminal, as `gdd` with those arguments, and its outcome is
/// the outcome.
pub fn in_terminal(cwd: &Path, change: &str) -> crate::Outcome {
    let shown = Sitting::load(cwd, change).and_then(tui::in_terminal);
    match shown.map(|sitting| sitting.chosen()) {
        Ok(Some(chosen)) => crate::run(&chosen, cwd),
        Ok(None) => crate::Outcome::success(String::new()),
        Err(why) => crate::Outcome::error(&why),
    }
}

/// Why `gdd sitting` without a terminal did nothing, and what records the same decisions.
pub(crate) fn no_terminal(id: &str) -> String {
    format!(
        "gdd sitting needs a terminal; without one, record the sitting with gdd gate G1 ok {id} \
         --by human (and gdd gate zones ok --by human, gdd gate plan ok {id} --by human for a \
         red change)"
    )
}

impl Screen for Sitting {
    const NAME: &'static str = "the sitting";

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        self.paint(area, frame.buffer_mut());
    }

    fn press(&mut self, key: Key) {
        Sitting::press(self, key);
    }

    fn done(&self) -> bool {
        self.closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Project;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::fs;
    use std::process::Command;

    /// The scenario line of a hidden example.
    const HIDDEN: &str = "features/hidden/h.feature · hidden: the Builders never see it";

    /// A sitting without a project behind it, with `lines` on its main page.
    fn sitting(lines: Vec<Line<'static>>, approval: Option<Approval>) -> Sitting {
        Sitting::new("demo", "c1", Path::new("/nowhere"), lines, approval)
    }

    fn scenario(path: &str) -> Scenario {
        Scenario {
            path: path.to_string(),
            hidden: false,
            text: String::new(),
        }
    }

    fn approval() -> Approval {
        Approval {
            sealing: vec![scenario("features/a.feature")],
            zones: Some(2),
            plan: true,
        }
    }

    /// A sitting that has something to approve.
    fn approving() -> Sitting {
        sitting(Vec::new(), Some(approval()))
    }

    fn press(sitting: &mut Sitting, keys: &[Key]) {
        for key in keys {
            sitting.press(*key);
        }
    }

    fn type_in(sitting: &mut Sitting, text: &str) {
        for letter in text.chars() {
            sitting.press(Key::Char(letter));
        }
    }

    fn assert_shows(sitting: &Sitting, texts: &[&str]) {
        let screen = sitting.render(100, 40);
        for text in texts {
            assert!(screen.contains(text), "no {text:?} in\n{screen}");
        }
    }

    fn assert_hides(sitting: &Sitting, texts: &[&str]) {
        let screen = sitting.render(100, 40);
        for text in texts {
            assert!(!screen.contains(text), "{text:?} in\n{screen}");
        }
    }

    /// Asserts `first` comes before `then` on `screen`.
    fn assert_before(screen: &str, first: &str, then: &str) {
        let (at, to) = (screen.find(first), screen.find(then));
        assert!(at.is_some() && to.is_some(), "{screen}");
        assert!(at < to, "{first:?} is not before {then:?} in\n{screen}");
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=GDD Test", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(out.status.success(), "git {args:?}: {said}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A project in a git repository, with a red zone, everything committed on main.
    fn repo() -> Project {
        let project = Project::new();
        project.write(crate::RULES, "[protected]\nall = [\"gdd/**\"]\n");
        let zones = "default = \"yellow\"\n\n[[zone]]\ncolor = \"red\"\n\
                     paths = [\"src/storage/**\"]\nreason = \"Losing drafts\"\n";
        project.write(crate::ZONES, zones);
        git(&project.0, &["init", "-q", "-b", "main"]);
        git(&project.0, &["add", "-A"]);
        git(&project.0, &["commit", "-q", "-m", "project"]);
        project
    }

    /// Runs `gdd` with `command` as a person.
    fn gdd(project: &Project, command: &str) {
        let words: Vec<String> = command.split_whitespace().map(str::to_string).collect();
        let out = crate::run_as(&words, &project.0, None);
        assert_eq!(out.code, 0, "{command}: {out:?}");
    }

    /// Commits `text` as `path` on the branch of the change `id`, and goes back to main.
    fn on_branch(project: &Project, id: &str, path: &str, text: &str) {
        git(&project.0, &["checkout", "-q", &format!("change/{id}")]);
        project.write(path, text);
        git(&project.0, &["add", "--", path]);
        git(&project.0, &["commit", "-q", "-m", path]);
        git(&project.0, &["checkout", "-q", "main"]);
    }

    /// A change `id` that plans `path` and has a scenario on its branch. Each test that records
    /// from the branch has an id of its own: the folder of its worktree is named after it.
    fn change(project: &Project, id: &str, path: &str) {
        let plan = format!("{path} features/d.feature");
        let command = format!("new-change {id} --title Drafts --paths {plan}");
        gdd(project, &command);
        let text = "Feature: D\n  Scenario: One\n";
        on_branch(project, id, "features/d.feature", text);
    }

    /// The sitting of the change `id`, whose decisions are recorded as a person, whatever
    /// `GDD_ROLE` says.
    fn open(project: &Project, id: &str) -> Sitting {
        let mut sitting = Sitting::load(&project.0, id).unwrap();
        sitting.env.retain(|(name, _)| name != "GDD_ROLE");
        sitting
    }

    /// How many working trees git lists, the main checkout among them.
    fn worktrees(project: &Project) -> usize {
        let listed = git(&project.0, &["worktree", "list", "--porcelain"]);
        listed.matches("worktree /").count()
    }

    fn read(project: &Project, path: &str) -> String {
        fs::read_to_string(project.0.join(path)).unwrap_or_default()
    }

    fn log(project: &Project) -> String {
        read(project, ".gdd/log.jsonl")
    }

    fn record(project: &Project, id: &str) -> String {
        read(project, &format!(".gdd/changes/{id}/change.json"))
    }

    /// Presses `a` and `y` on the sitting.
    fn approve(sitting: &mut Sitting) {
        press(sitting, &[Key::Char('a'), Key::Char('y')]);
    }

    #[test]
    fn the_title_names_the_project_and_the_keys_are_at_the_bottom() {
        let sitting = sitting(vec![Line::from("hello")], Some(approval()));
        let screen = sitting.render(100, 10);
        assert!(screen.starts_with("┌ gdd · Sitting · demo "), "{screen}");
        let rows: Vec<&str> = screen.lines().collect();
        assert!(rows[8].contains(KEYS), "{screen}");
        assert!(rows[9].starts_with('└'), "{screen}");
        assert_eq!(sitting.render(0, 0), "");
    }

    #[test]
    fn a_page_longer_than_the_screen_scrolls_by_a_row() {
        let rows: Vec<Line> = (0..60).map(|n| Line::from(format!("row {n}"))).collect();
        let mut sitting = sitting(rows, Some(approval()));
        let top = sitting.render(80, 24);
        assert!(top.contains("row 0 ") && top.contains("row 20 "), "{top}");
        assert!(!top.contains("row 21 "), "{top}");
        sitting.press(Key::Down);
        let down = sitting.render(80, 24);
        assert!(!down.contains("row 0 "), "{down}");
        assert!(down.contains("row 21 "), "{down}");
        for _ in 0..100 {
            sitting.press(Key::Down);
        }
        let end = sitting.render(80, 24);
        assert!(end.contains("row 59 ") && end.contains("row 39 "), "{end}");
        assert!(!end.contains("row 38 "), "{end}");
        sitting.press(Key::Up);
        let up = sitting.render(80, 24);
        assert!(up.contains("row 38 ") && !up.contains("row 59 "), "{up}");
        for _ in 0..100 {
            sitting.press(Key::Up);
        }
        assert!(sitting.render(80, 24).contains("row 0 "));
    }

    #[test]
    fn a_wrapped_line_counts_as_the_rows_it_takes() {
        let long = "word ".repeat(60);
        let mut sitting = sitting(vec![Line::from("first"), Line::from(long)], None);
        sitting.render(40, 6);
        for _ in 0..50 {
            sitting.press(Key::Down);
        }
        let end = sitting.render(40, 6);
        assert!(!end.contains("first"), "{end}");
        assert!(end.contains("word word"), "{end}");
    }

    #[test]
    fn a_short_page_does_not_scroll() {
        let mut sitting = sitting(vec![Line::from("only")], Some(approval()));
        press(&mut sitting, &[Key::Down, Key::Down]);
        assert_shows(&sitting, &["only"]);
        assert_eq!(sitting.scroll, 0);
    }

    #[test]
    fn a_sitting_that_is_over_only_scrolls_and_closes() {
        let over = Line::from("c1 is building: its sitting is over");
        let mut sitting = sitting(vec![over], None);
        assert_shows(&sitting, &["c1 is building", SCROLL_KEYS]);
        assert_hides(&sitting, &[KEYS]);
        press(&mut sitting, &[Key::Char('a'), Key::Char('c')]);
        assert_eq!(sitting.page, Page::Main);
        assert_hides(&sitting, &["This records", QUESTION]);
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
    }

    #[test]
    fn the_confirmation_says_exactly_what_will_be_recorded() {
        let mut sitting = approving();
        sitting.press(Key::Char('a'));
        assert_shows(&sitting, &["This records, as you:", CONFIRM]);
        assert_shows(&sitting, &["- G1 ok on c1, sealing features/a.feature"]);
        assert_shows(&sitting, &["- red zones ok: 2 red zone(s)"]);
        assert_shows(&sitting, &["- plan ok on c1", SCROLL_KEYS]);
        let nothing = Approval {
            sealing: Vec::new(),
            zones: None,
            plan: false,
        };
        let mut sitting = sitting_of(nothing);
        sitting.press(Key::Char('a'));
        assert_hides(&sitting, &["G1 ok", "red zones", "plan ok"]);
    }

    fn sitting_of(approval: Approval) -> Sitting {
        sitting(Vec::new(), Some(approval))
    }

    #[test]
    fn n_and_esc_go_back_from_a_confirmation() {
        for back in [Key::Char('n'), Key::Esc] {
            let mut sitting = approving();
            press(&mut sitting, &[Key::Char('a'), back]);
            assert_eq!(sitting.page, Page::Main);
            press(&mut sitting, &[Key::Char('c'), Key::Char('x')]);
            press(&mut sitting, &[Key::Enter, back]);
            assert_eq!(sitting.page, Page::Main);
            assert!(!sitting.closed());
        }
    }

    #[test]
    fn q_closes_from_a_confirmation_and_other_keys_do_nothing() {
        let mut sitting = approving();
        press(&mut sitting, &[Key::Char('a'), Key::Char('q')]);
        assert!(sitting.closed());
        let mut sitting = approving();
        press(&mut sitting, &[Key::Char('c'), Key::Char('x')]);
        assert!(!sitting.closed());
        let mut sitting = approving();
        press(&mut sitting, &[Key::Char('c'), Key::Char('x'), Key::Enter]);
        press(&mut sitting, &[Key::Char('x'), Key::Up, Key::Enter]);
        assert_eq!(sitting.page, Page::Confirm("x".to_string()));
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
        let mut sitting = approving();
        press(&mut sitting, &[Key::Char('a'), Key::Char('x'), Key::Up]);
        assert_eq!(sitting.page, Page::Approve);
    }

    #[test]
    fn the_text_line_types_erases_and_needs_some_text_to_go_on() {
        let mut sitting = approving();
        sitting.press(Key::Char('c'));
        assert_shows(&sitting, &[QUESTION, ASK_KEYS]);
        sitting.press(Key::Enter);
        assert_eq!(sitting.page, Page::Ask(String::new()));
        press(&mut sitting, &[Key::Space, Key::Enter]);
        assert_eq!(sitting.page, Page::Ask(" ".to_string()));
        press(&mut sitting, &[Key::Backspace, Key::Backspace]);
        assert_eq!(sitting.page, Page::Ask(String::new()));
        type_in(&mut sitting, "Two");
        sitting.press(Key::Space);
        type_in(&mut sitting, "q y n!");
        assert_shows(&sitting, &["> Two q y n!_"]);
        sitting.press(Key::Backspace);
        assert_shows(&sitting, &["> Two q y n_"]);
        press(&mut sitting, &[Key::Up, Key::Down]);
        assert!(matches!(sitting.page, Page::Ask(_)));
        assert!(!sitting.closed());
        sitting.press(Key::Enter);
        assert_shows(&sitting, &["G1 ko on c1: Two q y n", CONFIRM]);
    }

    #[test]
    fn esc_on_the_text_line_goes_back_and_forgets_it() {
        let mut sitting = approving();
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, "abc");
        sitting.press(Key::Esc);
        assert_eq!(sitting.page, Page::Main);
        sitting.press(Key::Char('c'));
        assert_eq!(sitting.page, Page::Ask(String::new()));
    }

    #[test]
    fn ctrl_c_closes_on_every_page_and_a_closed_sitting_takes_no_keys() {
        let pages = [vec![], vec![Key::Char('a')], vec![Key::Char('c')]];
        let later = [Key::Char('a'), Key::Char('c'), Key::Esc, Key::Char('n')];
        for keys in pages {
            let mut sitting = approving();
            press(&mut sitting, &keys);
            sitting.press(Key::CtrlC);
            assert!(sitting.closed(), "{keys:?}");
            let page = sitting.page.clone();
            press(&mut sitting, &later);
            assert_eq!(sitting.page, page, "{keys:?}");
        }
    }

    #[test]
    fn the_result_scrolls_and_q_closes_it() {
        let mut sitting = approving();
        sitting.page = Page::Done((0..50).map(|n| format!("line {n}")).collect());
        sitting.render(80, 24);
        press(&mut sitting, &[Key::Down, Key::Char('a'), Key::Char('y')]);
        assert_eq!(sitting.scroll, 1);
        sitting.press(Key::Up);
        assert_eq!(sitting.scroll, 0);
        assert!(!sitting.closed());
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
    }

    #[test]
    fn a_confirmation_does_not_scroll_with_the_page_it_came_from() {
        let rows: Vec<Line> = (0..60).map(|n| Line::from(format!("row {n}"))).collect();
        let mut sitting = sitting(rows, Some(approval()));
        sitting.render(80, 24);
        for _ in 0..5 {
            sitting.press(Key::Down);
        }
        sitting.press(Key::Char('a'));
        // A body of three rows holds less than the confirmation, so a scroll would show.
        let small = sitting.render(80, 6);
        assert!(small.contains("This records, as you:"), "{small}");
    }

    #[test]
    fn a_red_zone_without_a_reason_shows_only_its_paths() {
        let project = repo();
        let zones = "default = \"yellow\"\n\n[[zone]]\ncolor = \"red\"\npaths = [\"a/**\", \"b/**\"]\n\
                     reason = \"\"\n\n[[zone]]\ncolor = \"red\"\npaths = [\"c/**\"]\n";
        project.write(crate::ZONES, zones);
        let (lines, waiting) = red_zones(&project.0, &project.0).unwrap();
        let shown: Vec<String> = lines.iter().map(ToString::to_string).collect();
        assert_eq!(shown, ["Red zones to approve", "a/**, b/**", "c/**"]);
        assert_eq!(waiting, Some(2));
    }

    #[test]
    fn the_sitting_is_a_screen_that_is_done_when_it_closes() {
        let rows: Vec<Line> = (0..3).map(|n| Line::from(format!("row {n}"))).collect();
        let mut sitting = sitting(rows, Some(approval()));
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let keys = [None, Some(Key::Down), None, Some(Key::Char('q'))];
        let mut keys = keys.into_iter();
        // Reading fails once the keys run out, so a sitting that never closes fails the test.
        let no_more = || std::io::Error::other("no more keys");
        let next = || keys.next().ok_or_else(no_more);
        let ended = tui::run(&mut sitting, &mut terminal, next);
        assert_eq!(ended, Ok(()));
        assert!(sitting.closed());
        let shown = tui::screen_text(terminal.backend().buffer());
        assert!(shown.contains("row 2"), "{shown}");
    }

    #[test]
    fn q_on_the_main_page_closes_and_writes_nothing() {
        let project = repo();
        change(&project, "c1", "src/a.rs");
        let mut sitting = open(&project, "c1");
        sitting.press(Key::Esc);
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn an_unknown_change_is_an_error_naming_it() {
        let project = repo();
        let failed = Sitting::load(&project.0, "nope").unwrap_err();
        assert!(failed.contains("unknown change nope"), "{failed}");
        let failed = Sitting::load(Path::new("/"), "nope").unwrap_err();
        assert!(failed.contains("not a GDD project"), "{failed}");
    }

    #[test]
    fn a_record_that_cannot_be_read_is_an_error() {
        let project = repo();
        project.write(".gdd/changes/z/change.json", "{not json");
        let failed = Sitting::load(&project.0, "z").unwrap_err();
        assert!(failed.starts_with("z: "), "{failed}");
    }

    #[test]
    fn the_scenarios_of_the_branch_show_visible_ones_first_and_hidden_ones_marked() {
        let project = repo();
        change(&project, "c1", "src/storage/db.rs");
        let hidden = "Feature: H\n\tScenario: Hid\n";
        on_branch(&project, "c1", "features/hidden/h.feature", hidden);
        on_branch(&project, "c1", "features/a.feature", "Feature: A\n");
        on_branch(&project, "c1", "docs/x.md", "not a scenario\n");
        let sitting = open(&project, "c1");
        let screen = sitting.render(100, 40);
        assert_before(&screen, "c1 · Drafts · red", "Scenarios");
        assert_before(&screen, "features/a.feature", "features/d.feature");
        assert_before(&screen, "features/d.feature", HIDDEN);
        assert_before(&screen, "Scenario: One", "Red zones to approve");
        assert_before(&screen, "src/storage/** · Losing drafts", "Plan: none");
        assert_hides(&sitting, &["not a scenario"]);
        assert_shows(&sitting, &["    Scenario: Hid"]);
    }

    #[test]
    fn a_change_without_scenario_files_says_so_and_seals_nothing() {
        let project = repo();
        gdd(&project, "new-change c1 --title Drafts --paths src/a.rs");
        let mut sitting = open(&project, "c1");
        assert_shows(&sitting, &["No scenario files on change/c1"]);
        sitting.press(Key::Char('a'));
        assert_shows(&sitting, &["This records, as you:"]);
        assert_shows(&sitting, &["red zones ok: 1 red zone(s)"]);
        assert_hides(&sitting, &["G1 ok", "plan ok"]);
    }

    #[test]
    fn a_change_whose_branch_is_not_there_has_no_scenario_files() {
        let project = repo();
        gdd(&project, "new-change c1 --title Drafts --paths src/a.rs");
        git(&project.0, &["branch", "-D", "change/c1"]);
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["No scenario files on change/c1"]);
    }

    #[test]
    fn a_file_the_branch_deleted_has_nothing_to_show() {
        let project = repo();
        project.write("features/gone.feature", "Feature: Gone\n");
        git(&project.0, &["add", "--", "features/gone.feature"]);
        git(&project.0, &["commit", "-q", "-m", "gone"]);
        change(&project, "c1", "src/a.rs");
        git(&project.0, &["checkout", "-q", "change/c1"]);
        git(&project.0, &["rm", "-q", "features/gone.feature"]);
        git(&project.0, &["commit", "-q", "-m", "delete it"]);
        git(&project.0, &["checkout", "-q", "main"]);
        let sitting = open(&project, "c1");
        assert_hides(&sitting, &["features/gone.feature"]);
        assert_shows(&sitting, &["features/d.feature"]);
    }

    #[test]
    fn a_project_without_its_main_branch_cannot_be_read() {
        let project = repo();
        change(&project, "c1", "src/a.rs");
        project.write(crate::RULES, "main_branch = \"trunk\"\n");
        let failed = Sitting::load(&project.0, "c1").unwrap_err();
        assert!(failed.contains("trunk"), "{failed}");
    }

    #[test]
    fn red_zones_approved_say_so_and_a_yellow_change_needs_no_plan() {
        let project = repo();
        gdd(&project, "gate zones ok --by human");
        change(&project, "c2", "src/export.rs");
        let mut sitting = open(&project, "c2");
        assert_shows(&sitting, &["c2 · Drafts · yellow", "Red zones: approved"]);
        assert_shows(&sitting, &["Plan: not needed for a yellow change"]);
        assert_hides(&sitting, &["Red zones to approve"]);
        sitting.press(Key::Char('a'));
        assert_shows(&sitting, &["G1 ok on c2"]);
        assert_hides(&sitting, &["red zones ok", "plan ok"]);
    }

    #[test]
    fn a_zone_without_a_reason_shows_only_its_paths() {
        let project = repo();
        let zones = "[[zone]]\ncolor = \"red\"\npaths = [\"a/**\", \"b/**\"]\n";
        project.write(crate::ZONES, zones);
        gdd(&project, "new-change c1 --title Drafts --paths src/a.rs");
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["a/**, b/**"]);
        assert_hides(&sitting, &["a/**, b/** ·"]);
    }

    #[test]
    fn a_red_change_shows_its_plan() {
        let project = repo();
        change(&project, "c1", "src/storage/db.rs");
        let plan = "# Plan\n\nKeep every draft in one file.\n";
        project.write(".gdd/changes/c1/plan.md", plan);
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["Plan", "Keep every draft in one file."]);
        assert_hides(&sitting, &["none written yet"]);
    }

    #[test]
    fn a_change_past_its_sitting_shows_only_that() {
        let project = repo();
        change(&project, "c1", "src/a.rs");
        let text = record(&project, "c1").replace("\"open\"", "\"building\"");
        project.write(".gdd/changes/c1/change.json", &text);
        let mut sitting = open(&project, "c1");
        assert_shows(&sitting, &["c1 is building: its sitting is over"]);
        assert_hides(&sitting, &["Scenarios", "Red zones", "Plan"]);
        sitting.press(Key::Char('a'));
        assert_hides(&sitting, &["This records"]);
    }

    #[test]
    fn y_records_it_all_from_the_branch_and_leaves_no_worktree() {
        let project = repo();
        change(&project, "rec", "src/storage/db.rs");
        let mut sitting = open(&project, "rec");
        approve(&mut sitting);
        assert_shows(&sitting, &["Recorded", "G1 ok on rec"]);
        assert_shows(&sitting, &["Next: gdd launch rec", SCROLL_KEYS]);
        let log = log(&project);
        for gate in ["\"G1\"", "\"zones\"", "\"plan\""] {
            assert!(log.contains(gate), "{gate} in {log}");
        }
        assert_eq!(git(&project.0, &["branch", "--show-current"]), "main");
        assert_eq!(worktrees(&project), 1);
        let record = record(&project, "rec");
        assert!(record.contains("features/d.feature"), "{record}");
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
    }

    #[test]
    fn a_yellow_change_with_the_zones_approved_records_only_g1() {
        let project = repo();
        gdd(&project, "gate zones ok --by human");
        change(&project, "yel", "src/export.rs");
        let mut sitting = open(&project, "yel");
        approve(&mut sitting);
        let log = log(&project);
        assert!(log.contains("\"G1\""), "{log}");
        assert_eq!(log.matches("\"zones\"").count(), 1, "{log}");
        assert!(!log.contains("\"plan\""), "{log}");
    }

    #[test]
    fn a_g1_the_gate_refuses_records_nothing_and_leaves_no_worktree() {
        let project = repo();
        change(&project, "ref", "src/storage/db.rs");
        let hidden = "Feature: H\n  Scenario: S\n    Given a brand new sentence\n";
        on_branch(&project, "ref", "features/hidden/h.feature", hidden);
        let mut sitting = open(&project, "ref");
        approve(&mut sitting);
        assert_shows(&sitting, &["Nothing recorded", "a brand new sentence"]);
        assert_hides(&sitting, &["Recorded\n", "Next:"]);
        assert!(!log(&project).contains("\"zones\""));
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn a_branch_that_does_not_exist_records_nothing_and_leaves_no_worktree() {
        let project = repo();
        gdd(&project, "new-change gone --title Drafts --paths src/a.rs");
        let mut sitting = open(&project, "gone");
        sitting.approval = Some(approval());
        git(&project.0, &["branch", "-D", "change/gone"]);
        // The branch goes after the page was checked: the worktree cannot be made.
        let args = decision(&["gate", "G1", "ok", "gone"], NOTE);
        let said = sitting.run_on_branch(&args);
        assert!(said.is_err(), "{said:?}");
        assert!(!log(&project).contains("\"G1\""));
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn a_branch_that_moved_after_the_page_was_checked_records_nothing() {
        let project = repo();
        change(&project, "mov", "src/a.rs");
        let sitting = open(&project, "mov");
        let moved = "Feature: D\n  Scenario: Two\n";
        on_branch(&project, "mov", "features/d.feature", moved);
        let args = decision(&["gate", "G1", "ok", "mov"], NOTE);
        let said = sitting.run_on_branch(&args);
        let changed = "features/d.feature changed after you read it";
        assert_eq!(said, Err(changed.to_string()));
        assert!(!log(&project).contains("\"G1\""));
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn a_scenario_file_added_after_the_page_was_checked_records_nothing() {
        let project = repo();
        change(&project, "add", "src/a.rs");
        let sitting = open(&project, "add");
        on_branch(&project, "add", "features/f.feature", "Feature: F\n");
        let args = decision(&["gate", "G1", "ok", "add"], NOTE);
        let said = sitting.run_on_branch(&args);
        let added = "features/f.feature was added after you read it";
        assert_eq!(said, Err(added.to_string()));
        assert!(!log(&project).contains("\"G1\""));
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn a_page_that_changed_before_y_is_shown_again_and_nothing_is_recorded() {
        let project = repo();
        change(&project, "chg", "src/storage/db.rs");
        let mut sitting = open(&project, "chg");
        sitting.press(Key::Char('a'));
        assert_shows(&sitting, &["red zones ok: 1 red zone(s)"]);
        // A second red zone arrives while the confirmation is on the screen.
        let zones = "default = \"yellow\"\n\n[[zone]]\ncolor = \"red\"\n\
                     paths = [\"src/storage/**\"]\nreason = \"Losing drafts\"\n\n\
                     [[zone]]\ncolor = \"red\"\npaths = [\"src/auth/**\"]\nreason = \"Logins\"\n";
        project.write(crate::ZONES, zones);
        sitting.press(Key::Char('y'));
        assert_eq!(sitting.page, Page::Main);
        assert_shows(&sitting, &[CHANGED, "src/auth/** · Logins", KEYS]);
        assert!(!log(&project).contains("\"G1\""));
        assert!(!log(&project).contains("\"zones\""));
        assert_eq!(worktrees(&project), 1);
        // Read again, the confirmation shows both zones and `y` records them.
        sitting.press(Key::Char('a'));
        assert_shows(&sitting, &["red zones ok: 2 red zone(s)"]);
        assert_hides(&sitting, &[CHANGED]);
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["Recorded", "Next: gdd launch chg"]);
    }

    #[test]
    fn a_scenario_that_changed_before_y_is_shown_again() {
        let project = repo();
        change(&project, "scn", "src/a.rs");
        let mut sitting = open(&project, "scn");
        sitting.press(Key::Char('a'));
        on_branch(&project, "scn", "features/e.feature", "Feature: E\n");
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &[CHANGED, "features/e.feature"]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_plan_that_changed_before_y_is_shown_again_though_the_decisions_are_the_same() {
        let project = repo();
        change(&project, "pln", "src/storage/db.rs");
        let mut sitting = open(&project, "pln");
        sitting.press(Key::Char('a'));
        let plan = "# Plan\n\nWritten while the confirmation was open.\n";
        project.write(".gdd/changes/pln/plan.md", plan);
        sitting.press(Key::Char('y'));
        assert_eq!(sitting.page, Page::Main);
        assert_shows(
            &sitting,
            &[CHANGED, "Written while the confirmation was open."],
        );
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_change_that_left_its_sitting_before_y_loses_the_keys_to_approve() {
        let project = repo();
        change(&project, "lft", "src/a.rs");
        let mut sitting = open(&project, "lft");
        sitting.press(Key::Char('a'));
        let text = record(&project, "lft").replace("\"open\"", "\"building\"");
        project.write(".gdd/changes/lft/change.json", &text);
        sitting.press(Key::Char('y'));
        let over = "lft is building: its sitting is over";
        assert_shows(&sitting, &[CHANGED, over, SCROLL_KEYS]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_change_that_left_its_sitting_before_the_ko_records_nothing() {
        let project = repo();
        change(&project, "kol", "src/a.rs");
        let mut sitting = open(&project, "kol");
        press(&mut sitting, &[Key::Char('c')]);
        type_in(&mut sitting, "Too vague");
        press(&mut sitting, &[Key::Enter]);
        let text = record(&project, "kol").replace("\"open\"", "\"building\"");
        project.write(".gdd/changes/kol/change.json", &text);
        sitting.press(Key::Char('y'));
        let over = "kol is building: its sitting is over";
        assert_shows(&sitting, &[CHANGED, over, SCROLL_KEYS]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_project_that_cannot_be_read_before_the_ko_says_why_and_records_nothing() {
        let project = repo();
        change(&project, "kob", "src/a.rs");
        let mut sitting = open(&project, "kob");
        press(&mut sitting, &[Key::Char('c')]);
        type_in(&mut sitting, "Too vague");
        press(&mut sitting, &[Key::Enter]);
        project.write(crate::ZONES, "default = \n");
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["Nothing recorded"]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_project_that_cannot_be_read_before_y_says_why_and_records_nothing() {
        let project = repo();
        change(&project, "bad", "src/a.rs");
        let mut sitting = open(&project, "bad");
        sitting.press(Key::Char('a'));
        project.write(crate::ZONES, "default = \n");
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["Nothing recorded"]);
        assert_hides(&sitting, &["Recorded\n", "Next:"]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_failure_after_the_first_decision_says_what_was_recorded() {
        let project = repo();
        change(&project, "part", "src/storage/db.rs");
        let mut sitting = open(&project, "part");
        // The zones cannot be approved: the map cannot be read, after the page was checked.
        project.write(crate::ZONES, "default = \n");
        let g1 = decision(&["gate", "G1", "ok", "part"], NOTE);
        let zones = decision(&["gate", "zones", "ok"], NOTE);
        let on_branch = |args, on_branch| Step { args, on_branch };
        let steps = [on_branch(g1, true), on_branch(zones, false)];
        sitting.record(&steps, None);
        assert_shows(&sitting, &["Recorded", "G1 ok on part"]);
        assert_shows(&sitting, &["Nothing more recorded"]);
        assert_hides(&sitting, &["Next:"]);
    }

    #[test]
    fn a_worktree_folder_that_exists_is_refused() {
        let project = repo();
        change(&project, "dup", "src/a.rs");
        let name = format!("gdd-sitting-dup-{}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        fs::create_dir_all(&dir).unwrap();
        let mut sitting = open(&project, "dup");
        approve(&mut sitting);
        fs::remove_dir_all(&dir).unwrap();
        assert_shows(&sitting, &["Nothing recorded", "cannot use "]);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn a_builder_is_refused_like_on_the_command_line() {
        let project = repo();
        change(&project, "bld", "src/a.rs");
        let mut sitting = open(&project, "bld");
        let builder = ("GDD_ROLE".to_string(), "builder-build".to_string());
        sitting.env.push(builder);
        approve(&mut sitting);
        assert_shows(&sitting, &["Nothing recorded"]);
        assert!(!log(&project).contains("\"G1\""));
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn asking_for_changes_records_g1_ko_with_the_words_in_one_argument() {
        let project = repo();
        change(&project, "c1", "src/storage/db.rs");
        let mut sitting = open(&project, "c1");
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, "Two");
        press(&mut sitting, &[Key::Space, Key::Space]);
        type_in(&mut sitting, "drafts at once");
        sitting.press(Key::Enter);
        assert_shows(&sitting, &["G1 ko on c1: Two  drafts at once", CONFIRM]);
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["Recorded", "G1 ko on c1"]);
        assert_hides(&sitting, &["Next:"]);
        assert!(record(&project, "c1").contains("Two  drafts at once"));
        let log = log(&project);
        assert!(log.contains("\"ko\""), "{log}");
        assert!(!log.contains("\"zones\""), "{log}");
    }

    #[test]
    fn words_that_start_with_a_dash_are_recorded_as_they_are() {
        let project = repo();
        change(&project, "dash", "src/a.rs");
        let mut sitting = open(&project, "dash");
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, "- check concurrency");
        press(&mut sitting, &[Key::Enter, Key::Char('y')]);
        assert_shows(&sitting, &["Recorded", "G1 ko on dash"]);
        assert!(record(&project, "dash").contains("- check concurrency"));
    }

    #[test]
    fn a_project_in_a_folder_of_the_repository_records_from_the_same_folder() {
        let project = repo();
        project.write("app/gdd/project.toml", "name = \"app\"\n");
        project.write("app/gdd/rules.toml", "[protected]\nall = [\"gdd/**\"]\n");
        git(&project.0, &["add", "-A"]);
        git(&project.0, &["commit", "-q", "-m", "app"]);
        let app = project.0.join("app");
        let words: Vec<String> = "new-change sub --title Drafts --paths src/a.rs"
            .split_whitespace()
            .map(str::to_string)
            .collect();
        assert_eq!(crate::run_as(&words, &app, None).code, 0);
        on_branch(&project, "sub", "app/features/d.feature", "Feature: D\n");
        let mut sitting = Sitting::load(&app, "sub").unwrap();
        sitting.env.retain(|(name, _)| name != "GDD_ROLE");
        approve(&mut sitting);
        assert_shows(&sitting, &["Recorded", "G1 ok on sub"]);
        assert_eq!(worktrees(&project), 1);
    }

    #[test]
    fn a_confirmation_longer_than_the_screen_scrolls() {
        let sealing = (0..60)
            .map(|n| scenario(&format!("features/f{n}.feature")))
            .collect();
        let long = Approval {
            sealing,
            zones: Some(1),
            plan: true,
        };
        for page in [Key::Char('a'), Key::Char('c')] {
            let mut sitting = sitting_of(long.clone());
            if page == Key::Char('c') {
                sitting.page = Page::Confirm("word ".repeat(80));
            } else {
                sitting.press(page);
            }
            let top = sitting.render(40, 8);
            assert!(!top.contains("y to record"), "{top}");
            for _ in 0..200 {
                sitting.press(Key::Down);
            }
            let end = sitting.render(40, 8);
            assert!(end.contains("y to record"), "{end}");
            assert!(end.contains(SCROLL_KEYS), "{end}");
            let bottom = sitting.scroll;
            sitting.press(Key::Up);
            assert_eq!(sitting.scroll, bottom - 1, "{page:?}");
            sitting.press(Key::Char('n'));
            assert_eq!(sitting.scroll, 0);
        }
    }

    #[test]
    fn the_text_line_never_scrolls() {
        let mut sitting = approving();
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, &"word ".repeat(40));
        sitting.scroll = 3;
        let screen = sitting.render(20, 6);
        assert!(screen.contains("What should"), "{screen}");
    }

    #[test]
    fn a_ko_the_gate_refuses_says_why() {
        let project = repo();
        change(&project, "c1", "src/a.rs");
        let mut sitting = open(&project, "c1");
        let reviewer = ("GDD_ROLE".to_string(), "reviewer".to_string());
        sitting.env.push(reviewer);
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, "no");
        press(&mut sitting, &[Key::Enter, Key::Char('y')]);
        assert_shows(&sitting, &["Nothing recorded"]);
        assert!(!log(&project).contains("\"ko\""));
    }

    #[test]
    fn nothing_but_y_writes_a_file() {
        let project = repo();
        change(&project, "c1", "src/storage/db.rs");
        let before = log(&project);
        let mut sitting = open(&project, "c1");
        press(&mut sitting, &[Key::Char('a'), Key::Char('n')]);
        sitting.press(Key::Char('c'));
        type_in(&mut sitting, "words");
        press(&mut sitting, &[Key::Enter, Key::Char('n')]);
        assert_eq!(log(&project), before);
        assert!(!log(&project).contains("\"G1\""));
    }

    #[test]
    fn the_no_terminal_message_names_the_change() {
        let said = no_terminal("c1");
        let start = "gdd sitting needs a terminal; without one, record";
        assert!(said.starts_with(start), "{said}");
        assert!(said.contains("gdd gate G1 ok c1 --by human"), "{said}");
        assert!(said.contains("gdd gate zones ok --by human"), "{said}");
        let plan = "gdd gate plan ok c1 --by human for a red change";
        assert!(said.contains(plan), "{said}");
    }

    #[test]
    fn a_decision_is_recorded_as_the_human_with_a_note() {
        let args = decision(&["gate", "G1", "ok", "c1"], "sitting");
        let words = "gate G1 ok c1 --by human --note=sitting";
        assert_eq!(args.join(" "), words);
        let args = decision(&["gate", "G1", "ko", "c1"], "two words");
        assert_eq!(args.last().map(String::as_str), Some("--note=two words"));
        let args = decision(&["gate", "G1", "ko", "c1"], "- one more");
        assert_eq!(args.last().map(String::as_str), Some("--note=- one more"));
    }

    #[test]
    fn a_worktree_left_behind_is_told_after_what_the_command_said() {
        let left = || Err("cannot remove x".to_string());
        let said = || Ok(vec!["a".to_string()]);
        assert_eq!(with_cleanup(said(), Ok(())), said());
        let kept = vec!["a".to_string(), "warning: cannot remove x".to_string()];
        assert_eq!(with_cleanup(said(), left()), Ok(kept));
        let failed = Err("no".to_string());
        assert_eq!(with_cleanup(failed.clone(), Ok(())), failed);
        let both = Err("no\ncannot remove x".to_string());
        assert_eq!(with_cleanup(failed, left()), both);
    }

    /// A change `c1` that waits for its G4, with the notes `inbox` in its record.
    fn waiting(inbox: Value) -> (Project, Sitting) {
        let project = repo();
        gdd(&project, "new-change c1 --title App --paths src/a.rs");
        set_record(&project, |record| {
            record["state"] = Value::from(IN_PRE);
            record["inbox"] = inbox;
        });
        let sitting = open(&project, "c1");
        (project, sitting)
    }

    fn set_record(project: &Project, change: impl FnOnce(&mut Value)) {
        let mut record: Value = serde_json::from_str(&record(project, "c1")).unwrap();
        change(&mut record);
        project.write(".gdd/changes/c1/change.json", &record.to_string());
    }

    /// Writes the review of `c1`.
    fn review(project: &Project, text: &str) {
        project.write(".gdd/changes/c1/review.md", text);
    }

    /// Writes the demo file `name` of `c1`.
    fn demo(project: &Project, name: &str, text: &str) {
        let path = format!(".gdd/changes/c1/demo/{name}");
        project.write(&path, text);
    }

    #[test]
    fn a_change_at_g4_shows_its_page_instead_of_being_over() {
        let (project, _) = waiting(Value::Null);
        review(&project, "Fine work\nVERDICT: OK\n");
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["c1 · App · waiting for your G4", G4_KEYS]);
        assert_shows(&sitting, &["Reviewer: OK", "Fine work"]);
        assert_shows(&sitting, &[NO_DEMO, NO_CHANGELOG]);
        let gone = ["its sitting is over", "Decided without you", "VERDICT"];
        assert_hides(&sitting, &gone);
        assert_shows(&sitting, &["Flow time"]);
        assert_before(&sitting.render(100, 40), "Reviewer: OK", "Demo: none");
    }

    #[test]
    fn the_last_verdict_of_the_review_is_the_one_shown() {
        let (project, _) = waiting(Value::Null);
        let said = "Blocking: the name\nVERDICT: KO\nVERDICT: OK\n  VERDICT: KO  \n";
        review(&project, said);
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["Reviewer: KO", "Blocking: the name"]);
        assert_hides(&sitting, &["Reviewer: OK", "VERDICT"]);
    }

    #[test]
    fn a_review_without_a_verdict_is_no_review() {
        let (project, _) = waiting(Value::Null);
        assert_shows(&open(&project, "c1"), &[NO_REVIEW]);
        review(&project, "Just words\nVERDICT: maybe\n");
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &[NO_REVIEW]);
        assert_hides(&sitting, &["Just words"]);
    }

    #[test]
    fn the_g4_page_shows_how_the_change_works_before_the_review() {
        let (project, _) = waiting(Value::Null);
        let said = "Fine work\n## How it works\nOne pure function.\n\tThen the edge.\n\n\
                    ## Improvements\nShorter names\nVERDICT: OK\n";
        review(&project, said);
        let sitting = open(&project, "c1");
        let expected = vec![
            Line::from("c1 · App · waiting for your G4").bold(),
            Line::default(),
            Line::from("How it works").bold(),
            Line::from("One pure function."),
            Line::from("    Then the edge."),
            Line::default(),
            Line::from("Reviewer: OK"),
            Line::from("Fine work"),
            Line::from("## Improvements"),
            Line::from("Shorter names"),
            Line::default(),
        ];
        assert_eq!(sitting.lines[..expected.len()], expected);
        assert_hides(&sitting, &["## How it works", UNEXPLAINED]);
    }

    #[test]
    fn without_a_section_the_g4_page_says_the_reviewer_did_not_explain_the_change() {
        let (project, sitting) = waiting(Value::Null);
        let unexplained = |sitting: &Sitting, reviewer: &str| {
            let said = Line::from(reviewer.to_string());
            let start = [Line::from(UNEXPLAINED), Line::default(), said];
            assert_eq!(sitting.lines[2..5], start);
        };
        unexplained(&sitting, NO_REVIEW);
        review(&project, "Fine work\nVERDICT: OK\n");
        unexplained(&open(&project, "c1"), "Reviewer: OK");
        review(&project, "Fine work\n## How it works\n\n \nVERDICT: OK\n");
        let sitting = open(&project, "c1");
        unexplained(&sitting, "Reviewer: OK");
        assert_hides(&sitting, &["## How it works"]);
    }

    #[test]
    fn the_demo_shows_the_first_count_of_scenarios_and_its_path() {
        let (project, _) = waiting(Value::Null);
        demo(&project, "a.txt", "Recording done\nnot a count\n");
        demo(&project, "b.txt", "Run\n 12 scenarios (12 passed)\n");
        demo(&project, "c.txt", "3 scenarios (3 passed)\n");
        let shown = "Demo: 12 scenarios (12 passed) · .gdd/changes/c1/demo/b.txt";
        assert_shows(&open(&project, "c1"), &[shown]);
    }

    #[test]
    fn a_demo_file_without_a_count_shows_its_first_line() {
        let (project, _) = waiting(Value::Null);
        demo(&project, "a.txt", "Recording done\nsecond\n");
        demo(&project, "b.txt", "Other\n");
        let sitting = open(&project, "c1");
        let shown = "Demo: Recording done · .gdd/changes/c1/demo/a.txt";
        assert_shows(&sitting, &[shown]);
        assert_hides(&sitting, &[NO_DEMO, "second"]);
        let not_counts = [
            "scenarios (1)",
            "1 scenarios",
            "x scenarios (1)",
            "1 scenarios (1",
        ];
        for line in not_counts {
            assert!(!counts_scenarios(line), "{line}");
        }
        assert!(counts_scenarios("1 scenarios (1 passed)"));
    }

    #[test]
    fn a_demo_that_cannot_be_read_is_told() {
        let (project, _) = waiting(Value::Null);
        demo(&project, "a.txt", "x");
        let binary = project.0.join(".gdd/changes/c1/demo/b.txt");
        fs::write(binary, [0xff, 0xfe]).unwrap();
        let told = "Demo: cannot read .gdd/changes/c1/demo/b.txt";
        assert_shows(&open(&project, "c1"), &[told]);
        let (project, _) = waiting(Value::Null);
        project.write(".gdd/changes/c1/demo", "a file, not a folder");
        assert_shows(&open(&project, "c1"), &["Demo: cannot read"]);
    }

    #[test]
    fn only_the_open_decided_notes_show() {
        let notes = serde_json::json!([
            {"kind": "decided", "text": "Took the short path"},
            {"kind": "decided", "text": "Old choice", "resolved": "2026-10-01"},
            {"kind": "decided", "text": "Null resolved", "resolved": null},
            {"kind": "needs", "text": "A question"},
            {"kind": "decided", "text": "Flag resolved", "resolved": true},
            {"kind": "decided", "text": "Unresolved flag", "resolved": false},
            {"kind": "decided", "text": "Empty resolved", "resolved": ""},
        ]);
        let (_, sitting) = waiting(notes);
        assert_shows(&sitting, &["Decided without you", "- Took the short path"]);
        let kept = ["- Null resolved", "- Unresolved flag", "- Empty resolved"];
        assert_shows(&sitting, &kept);
        assert_hides(&sitting, &["Old choice", "A question", "Flag resolved"]);
        let old = serde_json::json!({"kind": "decided", "text": "Old", "resolved": "x"});
        let (_, sitting) = waiting(Value::Array(vec![old]));
        assert_hides(&sitting, &["Decided without you", "Old"]);
    }

    #[test]
    fn the_changelog_line_shows_with_its_breaking_mark() {
        let (project, _) = waiting(Value::Null);
        gdd(&project, "changelog-line c1 Added Export notes as Markdown");
        let added = "Changelog: Added · Export notes as Markdown";
        assert_shows(&open(&project, "c1"), &[added]);
        gdd(&project, "changelog-line c1 Changed One file --breaking");
        let sitting = open(&project, "c1");
        let breaking = "Changelog: Changed · **Breaking:** One file";
        assert_shows(&sitting, &[breaking]);
        gdd(&project, "changelog-line c1 --none");
        assert_shows(&open(&project, "c1"), &[NO_CHANGELOG]);
        set_record(&project, |record| record["changelog"] = Value::from("junk"));
        let junk = "Changelog: the field \"changelog\"";
        assert_shows(&open(&project, "c1"), &[junk]);
    }

    #[test]
    fn the_time_of_the_change_shows_when_it_was_measured() {
        let (project, _) = waiting(Value::Null);
        let entry = |ts: &str, change: &str| {
            format!(r#"{{"ts":"{ts}","event":"gate","change":"{change}","gate":"G4"}}"#)
        };
        let other = entry("2026-10-28T10:00:00+00:00", "c2");
        project.write(".gdd/log.jsonl", &format!("{other}\n"));
        let sitting = open(&project, "c1");
        assert_hides(&sitting, &["Flow time", "c2"]);
        let start = entry("2026-10-28T10:00:00+00:00", "c1");
        let end = entry("2026-10-28T11:00:00+00:00", "c1");
        project.write(".gdd/log.jsonl", &format!("{other}\n{start}\n{end}\n"));
        let sitting = open(&project, "c1");
        assert_shows(&sitting, &["Flow time 1h 00m"]);
        assert_before(&sitting.render(100, 40), "Changelog: none set", "Flow time");
        project.write(".gdd/log.jsonl", "{not json\n");
        assert_shows(&open(&project, "c1"), &["the log cannot be read"]);
    }

    #[test]
    fn o_asks_for_confirmation_and_then_chooses_resume_ok() {
        let (_project, mut sitting) = waiting(Value::Null);
        sitting.press(Key::Char('o'));
        let command = "gdd resume c1 --g4 ok";
        assert_shows(&sitting, &["This runs, as you:", command]);
        assert_shows(&sitting, &["The change is promoted."]);
        assert_shows(&sitting, &[RUN, SCROLL_KEYS]);
        assert_hides(&sitting, &["--note"]);
        assert_eq!(sitting.chosen(), None);
        sitting.press(Key::Char('y'));
        assert!(sitting.closed(), "{:?}", sitting.page);
        let ok = args(&["resume", "c1", "--g4", "ok"]);
        assert_eq!(sitting.chosen(), Some(ok));
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn k_and_p_ask_for_words_and_then_confirm_them() {
        let rejected = "The change is rejected and stays out of production.";
        let polish = "One more Builder round with your words, then the gates and the demo again.";
        let cases = [('k', "ko", rejected), ('p', "polish", polish)];
        for (key, answer, follows) in cases {
            let (_project, mut sitting) = waiting(Value::Null);
            sitting.press(Key::Char(key));
            assert_shows(&sitting, &[QUESTION, ASK_KEYS]);
            sitting.press(Key::Enter);
            assert_shows(&sitting, &[QUESTION]);
            assert_hides(&sitting, &["This runs"]);
            type_in(&mut sitting, "Not what I asked");
            press(&mut sitting, &[Key::Backspace, Key::Char('d'), Key::Space]);
            assert_shows(&sitting, &["> Not what I asked _"]);
            sitting.press(Key::Enter);
            let command = format!("gdd resume c1 --g4 {answer} --note \"Not what I asked\"");
            assert_shows(&sitting, &["This runs, as you:", &command, follows, RUN]);
            sitting.press(Key::Char('y'));
            assert!(sitting.closed());
            let words = ["resume", "c1", "--g4", answer, "--note", "Not what I asked"];
            assert_eq!(sitting.chosen(), Some(args(&words)));
        }
    }

    #[test]
    fn the_g4_of_a_spike_says_what_ok_brings_and_promises_no_promotion_or_demo() {
        let (project, _) = waiting(Value::Null);
        set_record(&project, |record| record["type"] = Value::from(SPIKE));
        let mut sitting = open(&project, "c1");
        assert!(sitting.lines.contains(&Line::from(SPIKE_PAGE)));
        assert_hides(&sitting, &[NO_DEMO, NO_REVIEW, NO_CHANGELOG, UNEXPLAINED]);
        assert_shows(&sitting, &["c1 · App · waiting for your G4", G4_KEYS]);
        sitting.press(Key::Char('o'));
        let ok = ["Only its ADR files reach main, in a commit of their own."];
        assert_shows(&sitting, &ok);
        assert_shows(&sitting, &["Its code stays on its branch."]);
        assert_shows(&sitting, &["gdd resume c1 --g4 ok"]);
        assert_hides(&sitting, &["promoted"]);
        sitting.press(Key::Esc);
        press(&mut sitting, &[Key::Char('p'), Key::Char('x'), Key::Enter]);
        let polish = ["One more Builder round with your words, then its G2 again."];
        assert_shows(&sitting, &polish);
        assert_hides(&sitting, &["the demo"]);
        // Other kinds keep their page and their answers.
        let (_, mut feature) = waiting(Value::Null);
        assert_shows(&feature, &[NO_DEMO, NO_CHANGELOG]);
        assert_hides(&feature, &["Spike:"]);
        feature.press(Key::Char('o'));
        assert_hides(&feature, &ok);
    }

    #[test]
    fn polish_says_the_agents_run_now() {
        let (_, mut sitting) = waiting(Value::Null);
        sitting.press(Key::Char('p'));
        type_in(&mut sitting, "Shorter");
        sitting.press(Key::Enter);
        assert_shows(&sitting, &["The agents run now: it takes minutes."]);
        let (_, mut sitting) = waiting(Value::Null);
        press(&mut sitting, &[Key::Char('k'), Key::Char('x'), Key::Enter]);
        assert_hides(&sitting, &["The agents run now"]);
    }

    #[test]
    fn n_and_esc_go_back_to_the_g4_page_and_nothing_is_chosen() {
        for back in [Key::Char('n'), Key::Esc] {
            let asking = vec![Key::Char('k'), Key::Char('x'), Key::Enter];
            for open in [vec![Key::Char('o')], asking] {
                let (_, mut sitting) = waiting(Value::Null);
                press(&mut sitting, &open);
                sitting.press(back);
                assert_shows(&sitting, &["waiting for your G4", G4_KEYS]);
                assert!(!sitting.closed());
                assert_eq!(sitting.chosen(), None);
            }
        }
        let (_, mut sitting) = waiting(Value::Null);
        press(&mut sitting, &[Key::Char('p'), Key::Char('x'), Key::Esc]);
        assert_eq!(sitting.page, Page::Main);
        assert_eq!(sitting.chosen(), None);
    }

    #[test]
    fn the_g4_page_ignores_the_keys_of_the_sitting_and_q_closes() {
        let (_, mut sitting) = waiting(Value::Null);
        let keys = [Key::Char('a'), Key::Char('c'), Key::Char('y')];
        press(&mut sitting, &keys);
        assert_eq!(sitting.page, Page::Main);
        assert!(!sitting.closed());
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
        assert_eq!(sitting.chosen(), None);
        let mut sitting = approving();
        let keys = [Key::Char('o'), Key::Char('k'), Key::Char('p')];
        press(&mut sitting, &keys);
        assert_eq!(sitting.page, Page::Main);
    }

    #[test]
    fn q_and_ctrl_c_close_a_g4_confirmation_without_choosing() {
        for close in [Key::Char('q'), Key::CtrlC] {
            let (_, mut sitting) = waiting(Value::Null);
            let keys = [Key::Char('o'), Key::Char('x'), Key::Up, Key::Down, close];
            press(&mut sitting, &keys);
            assert!(sitting.closed());
            assert_eq!(sitting.chosen(), None);
        }
        let (_, mut sitting) = waiting(Value::Null);
        let keys = [Key::Char('k'), Key::Char('x'), Key::Char('q')];
        press(&mut sitting, &keys);
        assert!(!sitting.closed());
    }

    #[test]
    fn a_change_decided_meanwhile_is_not_chosen() {
        let (project, mut sitting) = waiting(Value::Null);
        sitting.press(Key::Char('o'));
        set_record(&project, |record| record["state"] = Value::from("closed"));
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["c1 is closed now: nothing was run"]);
        assert!(!sitting.closed());
        assert_eq!(sitting.chosen(), None);
        sitting.press(Key::Char('q'));
        assert!(sitting.closed());
    }

    #[test]
    fn a_record_that_cannot_be_read_at_y_chooses_nothing() {
        let (project, mut sitting) = waiting(Value::Null);
        sitting.press(Key::Char('o'));
        project.write(".gdd/changes/c1/change.json", "{not json");
        sitting.press(Key::Char('y'));
        assert_shows(&sitting, &["Nothing recorded"]);
        assert_eq!(sitting.chosen(), None);
    }

    #[test]
    fn up_and_down_scroll_the_g4_confirmation() {
        let (_project, mut sitting) = waiting(Value::Null);
        sitting.press(Key::Char('o'));
        sitting.render(80, 6);
        sitting.press(Key::Down);
        assert_eq!(sitting.scroll, 1);
        assert!(!sitting.render(80, 6).contains("This runs, as you:"));
        sitting.press(Key::Up);
        assert_eq!(sitting.scroll, 0);
        assert!(sitting.render(80, 6).contains("This runs, as you:"));
        assert_eq!(sitting.chosen(), None);
    }

    #[test]
    fn a_long_review_scrolls_at_80_by_24() {
        let (project, _) = waiting(Value::Null);
        let mut review: String = (0..60).map(|n| format!("note {n}\n")).collect();
        review.push_str("VERDICT: OK\n");
        project.write(".gdd/changes/c1/review.md", &review);
        let mut sitting = open(&project, "c1");
        let top = sitting.render(80, 24);
        assert!(top.contains("Reviewer: OK"), "{top}");
        assert!(top.contains("note 0 "), "{top}");
        assert!(!top.contains("note 59 "), "{top}");
        assert!(top.contains(G4_KEYS), "{top}");
        for _ in 0..200 {
            sitting.press(Key::Down);
        }
        let end = sitting.render(80, 24);
        assert!(end.contains("note 59 "), "{end}");
        assert!(end.contains("Changelog: none set"), "{end}");
        assert!(!end.contains("Reviewer: OK"), "{end}");
        sitting.press(Key::Up);
        assert!(sitting.render(80, 24).contains("Demo: none"));
    }
}
