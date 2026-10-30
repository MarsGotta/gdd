//! The Board of `gdd board`: the changes of the project by phase, on a screen, read-only. The
//! Board is a state machine, keys in and a screen out ([`Board`]), drawn with Ratatui.
//! [`in_terminal`] shows it in the terminal of the process. Nothing on it writes anything.

use std::cell;
use std::path::Path;

use gdd_core::flow::{self, Flow};
use gdd_core::log;
use gdd_core::state;
use gdd_core::status::{self, Card};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::tui::{self, Key, Screen};

/// The state that reads `waiting for your G4` instead of its name.
const IN_PRE: &str = "in-pre";

/// What a change in pre reads instead of its state.
const WAITING: &str = "waiting for your G4";

/// What the Board says of a project without changes.
const NO_CHANGES: &str =
    "No changes yet · ask your agent: /gdd-change <what you want> ($gdd-change in Codex)";

/// The keys of the list.
const LIST_KEYS: &str = "↑↓ move · Enter details · d done · t time · q quit";

/// The keys of the details.
const DETAIL_KEYS: &str = "Esc to go back · q quit";

/// The keys of the time view.
const TIME_KEYS: &str = "↑↓ scroll · Esc to go back · q quit";

/// The title of the time view.
const TIME_TITLE: &str = "Where the time goes";

/// What the time view says when the log has no change.
const NO_TIME: &str = "No time in the log yet";

/// What the details say of a change with no time measured.
const NOT_MEASURED: &str = "Time: nothing measured yet";

/// The line where the report puts what the project adds up to, after the changes.
const PROJECT_LINE: &str = "Project: ";

/// A section of the Board, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Records that cannot be read.
    Unreadable,
    Discover,
    Build,
    Release,
    Done,
}

impl Phase {
    /// Every section, in the order the Board shows them.
    const ALL: [Phase; 5] = [
        Phase::Unreadable,
        Phase::Discover,
        Phase::Build,
        Phase::Release,
        Phase::Done,
    ];

    /// The section of a change: a record that cannot be read has its own; a state the Board does
    /// not know is Build.
    fn of(card: &Card) -> Phase {
        if card.problem.is_some() {
            return Phase::Unreadable;
        }
        match card.state.as_str() {
            "open" => Phase::Discover,
            IN_PRE => Phase::Release,
            "closed" | "rejected" | "rolled-back" => Phase::Done,
            _ => Phase::Build,
        }
    }

    /// Its name.
    fn name(self) -> &'static str {
        match self {
            Phase::Unreadable => "Cannot read",
            Phase::Discover => "Discover",
            Phase::Build => "Build",
            Phase::Release => "Release",
            Phase::Done => "Done",
        }
    }
}

/// What is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    /// The details of the selected change.
    Details,
    /// Where the time of the project goes.
    Time,
}

/// The lines of the log that are gdd's and how many are not (see [`flow::read`]), or why the log
/// could not be read.
type TimeLog = Result<log::Read<flow::Line>, String>;

/// The Board: the changes of a project, the one selected and what is on the screen.
#[derive(Debug)]
pub struct Board {
    project: String,
    cards: Vec<Card>,
    /// The log, read once, when the Board loaded: the lines, or why they could not be read.
    log: TimeLog,
    /// Whether the changes of Done show.
    done_shown: bool,
    /// The one selected, among the changes that show.
    selected: usize,
    view: View,
    /// How many rows the details or the time view are scrolled up.
    scrolled: u16,
    /// The room the body had when it was last painted; the details and the time scroll within it.
    body: cell::Cell<Rect>,
    closed: bool,
}

impl Board {
    /// A Board of the project called `project` with its changes `cards` and no time in its log,
    /// the first change selected, Done folded.
    pub fn new(project: &str, cards: Vec<Card>) -> Board {
        Board {
            project: project.to_string(),
            cards,
            log: Ok(log::Read::default()),
            done_shown: false,
            selected: 0,
            view: View::List,
            scrolled: 0,
            body: cell::Cell::new(Rect::new(0, 0, 80, 20)),
            closed: false,
        }
    }

    /// This Board with `log` as the log its time comes from.
    fn with_log(self, log: TimeLog) -> Board {
        Board { log, ..self }
    }

    /// The Board of the project that holds the folder `cwd`, with the changes under its state
    /// root, as every command finds them.
    pub fn load(cwd: &Path) -> Result<Board, String> {
        let root = crate::project_root(cwd)?;
        let state = state::root(&root)?;
        let project = crate::read(&root, crate::PROJECT)?.unwrap_or_default();
        let basics =
            status::Basics::from_toml(&project).map_err(|e| format!("{}: {e}", crate::PROJECT))?;
        let board = Board::new(&basics.name, status::cards(&root, &state));
        Ok(board.with_log(read_log(&state)))
    }

    /// The human presses `key`: q or Ctrl-C closes the Board; on the list, Up and Down move the
    /// selection within the changes that show, Enter opens the details of the selected one, `t`
    /// the time of the project and `d` shows or folds Done; in the details and the time, Up and
    /// Down scroll them and Esc goes back to the list. Once closed, keys do nothing.
    pub fn press(&mut self, key: Key) {
        if self.closed {
            return;
        }
        let last = self.shown().len().saturating_sub(1);
        match (key, self.view) {
            (Key::CtrlC | Key::Char('q'), _) => self.closed = true,
            (Key::Esc, View::Details | View::Time) => self.view = View::List,
            (Key::Up, View::Details | View::Time) => {
                self.scrolled = self.scrolled.saturating_sub(1);
            }
            (Key::Down, View::Details | View::Time) => {
                self.scrolled = (self.scrolled + 1).min(self.scroll_limit());
            }
            (Key::Enter, View::List) if self.selected_card().is_some() => self.open(View::Details),
            (Key::Char('t'), View::List) => self.open(View::Time),
            (Key::Up, View::List) => self.selected = self.selected.saturating_sub(1),
            (Key::Down, View::List) => self.selected = (self.selected + 1).min(last),
            (Key::Char('d'), View::List) => {
                self.done_shown = !self.done_shown;
                self.selected = self.selected.min(self.shown().len().saturating_sub(1));
            }
            _ => {}
        }
    }

    /// Shows `view`, scrolled to its top.
    fn open(&mut self, view: View) {
        self.view = view;
        self.scrolled = 0;
    }

    /// The screen as text, `width` x `height` cells, its lines joined by `\n`.
    pub fn render(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        self.paint(area, &mut buffer);
        tui::screen_text(&buffer)
    }

    /// Whether the human closed the Board.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Paints the screen on `buffer`, framed with the title, the keys at the bottom.
    fn paint(&self, area: Rect, buffer: &mut Buffer) {
        let title = format!(" gdd · Board · {} ", self.project);
        let block = Block::bordered().title(title);
        let inner = block.inner(area);
        block.render(area, buffer);
        let [body, keys] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
        self.body.set(body);
        let (lines, selected, keys_text) = match self.view {
            View::Details => (self.detail_lines(), None, DETAIL_KEYS),
            View::Time => (self.time_lines(), None, TIME_KEYS),
            View::List => {
                let (lines, selected) = self.list_lines();
                (lines, selected, LIST_KEYS)
            }
        };
        let offset = if self.view == View::List {
            scroll(&lines, selected, body)
        } else {
            self.scrolled.min(self.scroll_limit())
        };
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((offset, 0))
            .render(body, buffer);
        Paragraph::new(Line::from(keys_text).dim()).render(keys, buffer);
    }

    /// How many rows the details or the time can scroll up before their last row is at the
    /// bottom of the body as last painted.
    fn scroll_limit(&self) -> u16 {
        let body = self.body.get();
        let lines = if self.view == View::Time {
            self.time_lines()
        } else {
            self.detail_lines()
        };
        let total: usize = lines.iter().map(|line| rows(line, body.width)).sum();
        let limit = total.saturating_sub(usize::from(body.height));
        u16::try_from(limit).unwrap_or(u16::MAX)
    }

    /// The changes of a section, as positions among the cards.
    fn members(&self, phase: Phase) -> Vec<usize> {
        let cards = self.cards.iter().enumerate();
        let of_phase = cards.filter(|(_, card)| Phase::of(card) == phase);
        of_phase.map(|(at, _)| at).collect()
    }

    /// The changes that show, in the order of the sections, as positions among the cards.
    fn shown(&self) -> Vec<usize> {
        let phases = Phase::ALL.into_iter();
        let open = phases.filter(|phase| self.done_shown || *phase != Phase::Done);
        open.flat_map(|phase| self.members(phase)).collect()
    }

    /// The change selected, when any shows.
    fn selected_card(&self) -> Option<&Card> {
        let at = *self.shown().get(self.selected)?;
        self.cards.get(at)
    }

    /// The list: a heading per section and a line per change that shows, and the position of
    /// the line of the selected one.
    fn list_lines(&self) -> (Vec<Line<'static>>, Option<usize>) {
        if self.cards.is_empty() {
            return (vec![Line::from(NO_CHANGES)], None);
        }
        let mut at_selected = None;
        let selected = self.shown().get(self.selected).copied();
        let mut lines = Vec::new();
        for phase in Phase::ALL {
            let members = self.members(phase);
            if phase == Phase::Unreadable && members.is_empty() {
                continue;
            }
            lines.push(Line::from(self.heading(phase, members.len())).bold());
            let folded = phase == Phase::Done && !self.done_shown;
            for at in members.into_iter().filter(|_| !folded) {
                let current = selected == Some(at);
                let line = Line::from(item(&self.cards[at], current));
                if current {
                    at_selected = Some(lines.len());
                }
                lines.push(if current { line.bold() } else { line });
            }
            lines.push(Line::default());
        }
        (lines, at_selected)
    }

    /// The heading of a section of `count` changes; Done says what `d` does.
    fn heading(&self, phase: Phase, count: usize) -> String {
        let name = phase.name();
        match (phase, self.done_shown) {
            (Phase::Done, false) => format!("{name} ({count}) · d to show"),
            (Phase::Done, true) => format!("{name} ({count}) · d to hide"),
            _ => format!("{name} ({count})"),
        }
    }

    /// The details of the selected change.
    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(card) = self.selected_card() else {
            return vec![Line::from(NO_CHANGES)];
        };
        let mut lines = vec![Line::from(format!("{} · {}", card.id, card.title)).bold()];
        if let Some(problem) = &card.problem {
            lines.push(Line::from(format!("Problem: {problem}")));
        } else {
            let paths = match card.paths.as_slice() {
                [] => "none planned".to_string(),
                paths => paths.join(", "),
            };
            lines.extend([
                Line::from(format!("Type: {}", card.kind)),
                Line::from(format!("Color: {}", card.color)),
                Line::from(format!("State: {}", card.state)),
                Line::from(format!("Gates: {}", card.gates)),
                Line::from(format!("Paths: {paths}")),
            ]);
        }
        let inbox = format!("Inbox: .gdd/changes/{}/inbox.md", card.id);
        lines.push(Line::from(inbox));
        lines.push(Line::default());
        lines.extend(self.change_time_lines(&card.id));
        lines
    }

    /// The time of the change `id` as `gdd report --time --change` writes it, without its first
    /// line and its indent; or why there is none.
    fn change_time_lines(&self, id: &str) -> Vec<Line<'static>> {
        let log = match &self.log {
            Ok(log) => log,
            Err(why) => return vec![cannot_read(why)],
        };
        let flow = Flow::new(&log.lines, Some(id), None);
        let entries = entries(&flow);
        let flow_s = entries.first().and_then(|change| change["flow_s"].as_u64());
        if flow_s.unwrap_or(0) == 0 {
            return vec![Line::from(NOT_MEASURED)];
        }
        let text = flow.to_string();
        let rows = text.lines().skip(1);
        rows.map(|row| Line::from(row.strip_prefix("  ").unwrap_or(row).to_string()))
            .collect()
    }

    /// The time of the project: how many lines of the log it left out, when it left out any (see
    /// [`log::left_out`]); then what the changes add up to, then each change, as the report
    /// writes them.
    fn time_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(TIME_TITLE).bold(), Line::default()];
        let log = match &self.log {
            Ok(log) => log,
            Err(why) => {
                lines.push(cannot_read(why));
                return lines;
            }
        };
        if let Some(said) = log::left_out(log.left_out) {
            lines.extend([Line::from(said), Line::default()]);
        }
        let flow = Flow::new(&log.lines, None, None);
        if entries(&flow).is_empty() {
            lines.push(Line::from(NO_TIME));
            return lines;
        }
        let text = flow.to_string();
        let rows: Vec<&str> = text.lines().collect();
        let project = rows
            .iter()
            .rposition(|row| row.starts_with(PROJECT_LINE))
            .unwrap_or(rows.len());
        let (changes, totals) = rows.split_at(project);
        lines.extend(totals.iter().map(|row| Line::from(row.to_string())));
        lines.push(Line::default());
        let blocks = changes.strip_suffix(&[""]).unwrap_or(changes);
        lines.extend(blocks.iter().map(|row| Line::from(row.to_string())));
        lines
    }
}

/// The line that says the log could not be read, and why.
fn cannot_read(why: &str) -> Line<'static> {
    Line::from(format!("the log cannot be read: {why}"))
}

/// The log under the state root `state`: its lines that are gdd's and how many are not, or why it
/// could not be read. A missing log has no lines.
fn read_log(state: &Path) -> TimeLog {
    let text = crate::read(state, log::PATH)?.unwrap_or_default();
    Ok(flow::read(&text))
}

/// The changes of `flow` as its JSON gives them.
fn entries(flow: &Flow) -> Vec<serde_json::Value> {
    match serde_json::to_value(flow) {
        Ok(serde_json::Value::Array(changes)) => changes,
        _ => Vec::new(),
    }
}

/// How many rows of the screen the list scrolls up so that the line `selected` shows whole in
/// `area`, counting the rows the lines take once wrapped; none when it fits or nothing is selected.
fn scroll(lines: &[Line], selected: Option<usize>, area: Rect) -> u16 {
    let Some(at) = selected else {
        return 0;
    };
    let height = |line: &Line| rows(line, area.width);
    let top: usize = lines.iter().take(at).map(height).sum();
    let bottom = top + lines.get(at).map_or(0, height);
    let offset = bottom.saturating_sub(usize::from(area.height)).min(top);
    u16::try_from(offset).unwrap_or(u16::MAX)
}

/// How many rows `line` takes in `width` cells once wrapped, at least one.
fn rows(line: &Line, width: u16) -> usize {
    if width == 0 {
        return 1;
    }
    // A row holds at least one cell of the line, so the line never takes more rows than cells.
    let room = line.width().max(1);
    let area = Rect::new(0, 0, width, u16::try_from(room).unwrap_or(u16::MAX));
    let mut buffer = Buffer::empty(area);
    Paragraph::new(line.clone())
        .wrap(Wrap { trim: false })
        .render(area, &mut buffer);
    let used = buffer
        .content
        .chunks(usize::from(width))
        .rposition(|row| row.iter().any(|cell| cell.symbol() != " "));
    used.map_or(1, |at| at + 1)
}

/// The line of a change in the list, `selected` or not.
fn item(card: &Card, selected: bool) -> String {
    let mark = if selected { '›' } else { ' ' };
    if let Some(problem) = &card.problem {
        return format!("{mark} {} · {problem}", card.id);
    }
    let state = if card.state == IN_PRE {
        WAITING
    } else {
        card.state.as_str()
    };
    let Card {
        id,
        color,
        title,
        gates,
        ..
    } = card;
    format!("{mark} {id} · {color} · {title} · {state} · {gates}")
}

/// `gdd board` in a terminal: shows the Board of the project that holds the folder `cwd` until
/// the human closes it, in raw mode on the alternate screen, which are put back as they were on
/// every way out, a panic too; else why it could not.
pub fn in_terminal(cwd: &Path) -> crate::Outcome {
    let shown = Board::load(cwd).and_then(tui::in_terminal);
    match shown {
        Ok(_) => crate::Outcome::success(String::new()),
        Err(why) => crate::Outcome::error(&why),
    }
}

impl Screen for Board {
    const NAME: &'static str = "the board";

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        self.paint(area, frame.buffer_mut());
    }

    fn press(&mut self, key: Key) {
        Board::press(self, key);
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
    use std::io;
    use std::path::PathBuf;

    fn card(id: &str, state: &str) -> Card {
        Card {
            id: id.to_string(),
            title: format!("Title of {id}"),
            kind: "Feature".to_string(),
            state: state.to_string(),
            color: "yellow".to_string(),
            gates: "no gates yet".to_string(),
            paths: Vec::new(),
            problem: None,
        }
    }

    fn unreadable(id: &str) -> Card {
        let problem = "change.json is not valid: x".to_string();
        Card {
            problem: Some(problem),
            ..card(id, "")
        }
    }

    fn board(cards: Vec<Card>) -> Board {
        Board::new("demo", cards)
    }

    fn press(board: &mut Board, keys: &[Key]) {
        for key in keys {
            board.press(*key);
        }
    }

    fn assert_shows(board: &Board, texts: &[&str]) {
        let screen = board.render(100, 40);
        for text in texts {
            assert!(screen.contains(text), "no {text:?} in\n{screen}");
        }
    }

    fn assert_hides(board: &Board, texts: &[&str]) {
        let screen = board.render(100, 40);
        for text in texts {
            assert!(!screen.contains(text), "{text:?} in\n{screen}");
        }
    }

    /// The lines of the screen of `board` inside its frame, without the empty ones.
    fn lines(board: &Board) -> Vec<String> {
        let screen = board.render(100, 40);
        let inside = |row: &str| {
            let row = row.trim_start_matches('│');
            row.trim_end_matches(['│', ' ']).to_string()
        };
        let rows = screen.lines().skip(1).filter(|row| !row.starts_with('└'));
        rows.map(inside).filter(|row| !row.is_empty()).collect()
    }

    #[test]
    fn the_title_names_the_project_and_the_keys_are_at_the_bottom() {
        let board = board(vec![card("a", "open")]);
        let screen = board.render(100, 10);
        assert!(screen.starts_with("┌ gdd · Board · demo "), "{screen}");
        let rows: Vec<&str> = screen.lines().collect();
        assert!(rows[8].contains(LIST_KEYS), "{screen}");
        assert!(rows[9].starts_with('└'), "{screen}");
        assert_eq!(rows.len(), 10);
        assert_eq!(board.render(0, 0), "");
    }

    #[test]
    fn the_changes_are_in_the_section_of_their_state() {
        let states = [
            ("a", "open"),
            ("b", "prepared"),
            ("c", "building"),
            ("d", "polishing"),
            ("e", "parked"),
            ("f", "in-pre"),
            ("g", "closed"),
            ("h", "rejected"),
            ("i", "rolled-back"),
            ("j", "something-new"),
        ];
        let mut board = board(states.map(|(id, state)| card(id, state)).to_vec());
        let shown = [
            "Discover (1)",
            "Build (5)",
            "Release (1)",
            "Done (3) · d to show",
        ];
        assert_eq!(&lines(&board)[..1], ["Discover (1)"]);
        assert_shows(&board, &shown);
        let order = lines(&board);
        let at = |text: &str| order.iter().position(|row| row.contains(text)).unwrap();
        assert!(at("Discover (1)") < at("› a ·"));
        assert!(at("› a ·") < at("Build (5)"));
        assert!(at("Build (5)") < at(" b ·") && at(" j ·") < at("Release (1)"));
        assert!(at("Release (1)") < at(" f ·") && at(" f ·") < at("Done (3)"));
        assert_hides(&board, &[" g ·", " h ·", " i ·", "Cannot read"]);
        board.press(Key::Char('d'));
        assert_shows(&board, &["Done (3) · d to hide", " g ·", " h ·", " i ·"]);
        board.press(Key::Char('d'));
        assert_shows(&board, &["Done (3) · d to show"]);
        assert_hides(&board, &[" g ·", "d to hide"]);
    }

    #[test]
    fn a_record_that_cannot_be_read_comes_first_under_its_own_section() {
        let board = board(vec![card("a", "open"), unreadable("z")]);
        let order = lines(&board);
        assert_eq!(order[0], "Cannot read (1)");
        assert_eq!(order[1], "› z · change.json is not valid: x");
        assert_eq!(order[2], "Discover (1)");
    }

    #[test]
    fn a_change_shows_its_color_title_state_and_gates() {
        let drafts = Card {
            color: "red".to_string(),
            gates: "G1:ok, plan:ok".to_string(),
            ..card("c1", "building")
        };
        let board = board(vec![drafts, card("c2", "in-pre")]);
        let first = "› c1 · red · Title of c1 · building · G1:ok, plan:ok";
        assert_shows(&board, &[first]);
        let second = "  c2 · yellow · Title of c2 · waiting for your G4 · no gates yet";
        assert_shows(&board, &[second]);
        assert_hides(&board, &["in-pre"]);
    }

    #[test]
    fn the_first_change_is_selected_and_up_and_down_stay_within_the_ends() {
        let cards = vec![card("a", "open"), card("b", "open"), card("c", "closed")];
        let mut board = board(cards);
        assert_shows(&board, &["› a ·", "  b ·"]);
        press(&mut board, &[Key::Up, Key::Down]);
        assert_shows(&board, &["  a ·", "› b ·"]);
        press(&mut board, &[Key::Down, Key::Down]);
        assert_shows(&board, &["› b ·"]);
        press(&mut board, &[Key::Up, Key::Up]);
        assert_shows(&board, &["› a ·"]);
        assert_hides(&board, &["› b ·"]);
        let keys = [Key::Char('d'), Key::Down, Key::Down, Key::Down];
        press(&mut board, &keys);
        assert_shows(&board, &["› c ·"]);
        assert_hides(&board, &["› a ·", "› b ·"]);
    }

    #[test]
    fn folding_done_moves_a_selection_in_it_to_the_last_that_shows() {
        let mut board = board(vec![card("a", "open"), card("c", "closed")]);
        press(&mut board, &[Key::Char('d'), Key::Down]);
        assert_shows(&board, &["› c ·"]);
        board.press(Key::Char('d'));
        assert_shows(&board, &["› a ·"]);
        board.press(Key::Enter);
        assert_shows(&board, &["a · Title of a"]);
    }

    #[test]
    fn the_list_follows_the_selection_on_a_small_screen() {
        let cards: Vec<Card> = (0..22).map(|n| card(&format!("c{n:02}"), "open")).collect();
        let mut board = board(cards);
        let top = board.render(80, 24);
        assert!(
            top.contains("Discover (22)") && top.contains("› c00 ·"),
            "{top}"
        );
        assert!(!top.contains("c21 ·"), "{top}");
        for _ in 0..21 {
            board.press(Key::Down);
        }
        let bottom = board.render(80, 24);
        assert!(bottom.contains("› c21 ·"), "{bottom}");
        assert!(
            !bottom.contains("c00 ·") && bottom.contains(LIST_KEYS),
            "{bottom}"
        );
        for _ in 0..21 {
            board.press(Key::Up);
        }
        assert!(board.render(80, 24).contains("› c00 ·"));
    }

    #[test]
    fn the_list_follows_the_selection_into_done_and_through_wrapped_lines() {
        let long = "word ".repeat(30);
        let mut cards: Vec<Card> = (0..6)
            .map(|n| Card {
                title: long.clone(),
                ..card(&format!("o{n}"), "open")
            })
            .collect();
        cards.push(card("done1", "closed"));
        let mut board = board(cards);
        board.press(Key::Char('d'));
        for _ in 0..6 {
            board.press(Key::Down);
        }
        let screen = board.render(40, 14);
        assert!(screen.contains("› done1 ·"), "{screen}");
        assert!(screen.contains("Done (1) · d to hide"), "{screen}");
        press(&mut board, &[Key::Up, Key::Up]);
        let screen = board.render(40, 14);
        assert!(screen.contains("› o4 ·"), "{screen}");
        assert_eq!(rows(&Line::from("x".repeat(100)), 40), 3);
        assert_eq!(rows(&Line::default(), 40), 1);
        assert_eq!(rows(&Line::from("x"), 0), 1);
    }

    #[test]
    fn enter_shows_the_details_and_esc_goes_back() {
        let paths = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let first = Card {
            paths,
            kind: "Spike".to_string(),
            gates: "plan:ok".to_string(),
            ..card("c1", "open")
        };
        let mut board = board(vec![first, card("c2", "open")]);
        board.press(Key::Enter);
        let details = [
            "c1 · Title of c1",
            "Type: Spike",
            "Color: yellow",
            "State: open",
            "Gates: plan:ok",
            "Paths: src/a.rs, src/b.rs",
            "Inbox: .gdd/changes/c1/inbox.md",
            DETAIL_KEYS,
        ];
        assert_shows(&board, &details);
        assert_hides(&board, &["Discover", LIST_KEYS, "c2"]);
        press(&mut board, &[Key::Down, Key::Char('d'), Key::Enter]);
        assert_shows(&board, &["c1 · Title of c1"]);
        assert_hides(&board, &["Done (0) · d to hide"]);
        board.press(Key::Esc);
        assert_shows(&board, &["Discover (2)", "› c1 ·", LIST_KEYS]);
        press(&mut board, &[Key::Esc, Key::Down, Key::Enter]);
        assert_shows(&board, &["c2 · Title of c2", "Paths: none planned"]);
        assert_shows(&board, &["State: open", "Inbox: .gdd/changes/c2/inbox.md"]);
    }

    #[test]
    fn long_details_scroll_with_up_and_down_within_their_ends() {
        let paths: Vec<String> = (0..120)
            .map(|n| format!("crates/gdd/src/path{n:03}.rs"))
            .collect();
        let long = Card {
            paths,
            ..card("c1", "open")
        };
        let mut board = board(vec![long]);
        board.press(Key::Enter);
        let top = board.render(80, 24);
        assert!(
            top.contains("Type: Feature") && !top.contains("Inbox:"),
            "{top}"
        );
        for _ in 0..100 {
            board.press(Key::Down);
        }
        let end = board.render(80, 24);
        assert!(end.contains("Inbox: .gdd/changes/c1/inbox.md"), "{end}");
        assert!(end.contains(NOT_MEASURED), "{end}");
        assert!(!end.contains("Type: Feature"), "{end}");
        press(&mut board, &[Key::Up, Key::Up, Key::Up]);
        let up = board.render(80, 24);
        assert!(up.contains("path119.rs") && !up.contains("Inbox:"), "{up}");
        for _ in 0..100 {
            board.press(Key::Up);
        }
        let back = board.render(80, 24);
        assert!(
            back.contains("Type: Feature") && !back.contains("Inbox:"),
            "{back}"
        );
        board.press(Key::Down);
        board.press(Key::Esc);
        board.press(Key::Enter);
        assert!(board.render(80, 24).contains("Type: Feature"));
    }

    #[test]
    fn short_details_do_not_scroll() {
        let mut board = board(vec![card("c1", "open")]);
        board.press(Key::Enter);
        board.render(80, 24);
        press(&mut board, &[Key::Down, Key::Down]);
        assert_shows(&board, &["c1 · Title of c1", "Type: Feature"]);
    }

    #[test]
    fn the_details_of_a_record_that_cannot_be_read_say_why() {
        let mut board = board(vec![unreadable("z")]);
        board.press(Key::Enter);
        let details = [
            "z · ",
            "Problem: change.json is not valid: x",
            "Inbox: .gdd/changes/z/inbox.md",
        ];
        assert_shows(&board, &details);
        assert_hides(&board, &["Type:", "Paths:"]);
    }

    #[test]
    fn enter_without_a_change_stays_on_the_list() {
        let mut board = board(vec![card("c", "closed")]);
        board.press(Key::Enter);
        assert_shows(&board, &["Done (1) · d to show", LIST_KEYS]);
        assert_hides(&board, &["Inbox"]);
    }

    #[test]
    fn a_project_without_changes_says_how_to_start_one() {
        let mut board = board(Vec::new());
        assert_shows(&board, &[NO_CHANGES, LIST_KEYS]);
        assert_hides(&board, &["Discover", "Done"]);
        let keys = [Key::Enter, Key::Down, Key::Up, Key::Char('d')];
        press(&mut board, &keys);
        assert_shows(&board, &["No changes yet"]);
        assert!(!board.closed());
    }

    #[test]
    fn q_and_ctrl_c_close_the_board_from_the_list_and_the_details() {
        for (keys, closes) in [
            (vec![Key::Char('q')], true),
            (vec![Key::CtrlC], true),
            (vec![Key::Enter, Key::Char('q')], true),
            (vec![Key::Enter, Key::CtrlC], true),
            (vec![Key::Char('x'), Key::Space, Key::Backspace], false),
        ] {
            let mut board = board(vec![card("a", "open")]);
            press(&mut board, &keys);
            assert_eq!(board.closed(), closes, "{keys:?}");
        }
        let mut board = board(vec![card("a", "open"), card("b", "open")]);
        board.press(Key::Char('q'));
        press(&mut board, &[Key::Down, Key::Enter, Key::Char('d')]);
        assert!(board.closed());
        assert_shows(&board, &["› a ·"]);
        assert_hides(&board, &["Inbox", "Done (0) · d to hide"]);
    }

    /// Every file under `folder` with its bytes.
    fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut found = Vec::new();
        for entry in fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(files(&path));
            } else {
                found.push((path.clone(), fs::read(&path).unwrap()));
            }
        }
        found.sort();
        found
    }

    #[test]
    fn nothing_on_the_board_writes_a_file() {
        let project = Project::new();
        let a = r#"{"title": "A", "paths": ["x.rs"]}"#;
        project.write(".gdd/changes/a/change.json", a);
        project.write(".gdd/changes/b/change.json", r#"{"state": "closed"}"#);
        project.write(".gdd/changes/c/change.json", "{not json");
        let before = files(&project.0);
        let mut board = Board::load(&project.0).unwrap();
        let keys = [
            Key::Char('d'),
            Key::Down,
            Key::Enter,
            Key::Char('d'),
            Key::Up,
            Key::Esc,
            Key::Space,
            Key::Backspace,
            Key::Char('q'),
            Key::CtrlC,
        ];
        for key in keys {
            board.press(key);
            board.render(100, 40);
        }
        assert!(board.closed());
        assert_eq!(files(&project.0), before);
    }

    #[test]
    fn the_board_of_a_project_has_its_name_and_its_changes() {
        let project = Project::new();
        let record = r#"{"title": "Drafts", "state": "parked"}"#;
        project.write(".gdd/changes/a/change.json", record);
        let board = Board::load(&project.0).unwrap();
        let drafts = "› a · yellow · Drafts · parked";
        assert_shows(&board, &["gdd · Board · demo", "Build (1)", drafts]);
        let nested = project.0.join("deep/er");
        fs::create_dir_all(&nested).unwrap();
        assert!(Board::load(&nested).is_ok());
        let outside = Board::load(Path::new("/")).unwrap_err();
        assert!(outside.starts_with("not a GDD project"), "{outside}");
        project.write("gdd/project.toml", "name = ");
        let invalid = Board::load(&project.0).unwrap_err();
        assert!(invalid.starts_with("gdd/project.toml: "), "{invalid}");
    }

    /// A log line of `change` at `ts` (`2026-10-28T` and the rest), a run of the builder of
    /// `secs` seconds when `secs` is not 0, else the creation of the change.
    fn log_line(change: &str, ts: &str, secs: u32) -> String {
        let what = if secs == 0 {
            r#""role": "orchestrator", "result": "change-created", "note": "T""#.to_string()
        } else {
            format!(
                r#""gate": "G2", "role": "builder-build", "agent": "claude", "model": "sonnet", "duration_s": {secs}, "result": "ok""#
            )
        };
        format!(r#"{{"ts": "2026-10-28T{ts}+00:00", "change": "{change}", {what}}}"#)
    }

    /// A board of `cards` whose log is `lines`.
    fn timed(cards: Vec<Card>, lines: &[String]) -> Board {
        let log = flow::read(&lines.join("\n"));
        board(cards).with_log(Ok(log))
    }

    /// `count` changes `c00`... each created and then run by the builder an hour later.
    fn busy_log(count: usize) -> Vec<String> {
        let hour = |n: usize| format!("{:02}", 6 + n);
        (0..count)
            .flat_map(|n| {
                let id = format!("c{n:02}");
                let created = log_line(&id, &format!("{}:00:00", hour(n)), 0);
                [created, log_line(&id, &format!("{}:30:00", hour(n)), 600)]
            })
            .collect()
    }

    #[test]
    fn the_details_end_with_the_time_of_the_change_as_the_report_writes_it() {
        let log = [log_line("b", "08:00:00", 0), log_line("b", "08:30:00", 600)];
        let mut board = timed(vec![card("a", "open"), card("b", "open")], &log);
        press(&mut board, &[Key::Down, Key::Enter]);
        let rows = lines(&board);
        let at = rows
            .iter()
            .position(|row| row.starts_with("Inbox: "))
            .unwrap();
        assert!(rows[at + 1].starts_with("Flow time 30m"), "{rows:?}");
        assert!(
            rows[at + 2].starts_with("Agents: builder-build · sonnet"),
            "{rows:?}"
        );
        assert_hides(&board, &["b · T · ", NOT_MEASURED]);
    }

    #[test]
    fn a_change_without_time_in_the_log_says_nothing_is_measured_yet() {
        let log = [log_line("only", "08:00:00", 0)];
        let mut board = timed(vec![card("only", "open"), card("none", "open")], &log);
        board.press(Key::Enter);
        assert_shows(&board, &[NOT_MEASURED]);
        assert_hides(&board, &["Flow time"]);
        press(&mut board, &[Key::Esc, Key::Down, Key::Enter]);
        assert_shows(&board, &[NOT_MEASURED]);
    }

    #[test]
    fn the_time_view_scrolls_to_its_end_and_back() {
        let mut board = timed(vec![card("c00", "open")], &busy_log(12));
        board.press(Key::Char('t'));
        let top = board.render(80, 24);
        assert!(top.contains(TIME_TITLE) && top.contains(TIME_KEYS), "{top}");
        assert!(top.contains("Project: 12 change(s) · flow time "), "{top}");
        assert!(top.contains("c00 · T · "), "{top}");
        assert!(!top.contains("c11 · "), "{top}");
        press(&mut board, &[Key::Up]);
        assert_eq!(board.render(80, 24), top);
        for _ in 0..100 {
            board.press(Key::Down);
        }
        let end = board.render(80, 24);
        assert!(
            end.contains("c11 · T · ") && !end.contains(TIME_TITLE),
            "{end}"
        );
        assert!(end.contains(TIME_KEYS), "{end}");
        board.press(Key::Up);
        assert_ne!(board.render(80, 24), end);
        for _ in 0..100 {
            board.press(Key::Up);
        }
        assert_eq!(board.render(80, 24), top);
    }

    #[test]
    fn the_time_view_puts_the_project_before_the_blocks_of_the_changes() {
        let mut board = timed(vec![card("c00", "open")], &busy_log(2));
        board.press(Key::Char('t'));
        let rows = lines(&board);
        let at = |start: &str| rows.iter().position(|row| row.starts_with(start)).unwrap();
        assert_eq!(rows[0], TIME_TITLE);
        assert_eq!(at("Project: 2 change(s)"), 1);
        assert!(at("Project: ") < at("c00 · T · "), "{rows:?}");
        assert!(at("c00 · T · ") < at("  Flow time "), "{rows:?}");
        assert!(at("  Flow time ") < at("c01 · T · "), "{rows:?}");
        let screen = board.render(100, 40);
        let blank = screen.lines().nth(2).unwrap();
        assert!(blank.trim_matches(['│', ' ']).is_empty(), "{screen}");
    }

    #[test]
    fn a_log_without_changes_says_there_is_no_time_yet() {
        let mut board = board(vec![card("a", "open")]);
        board.press(Key::Char('t'));
        assert_eq!(lines(&board)[..2], [TIME_TITLE, NO_TIME]);
        let nameless = [r#"{"ts": "2026-10-28T08:00:00+00:00", "change": "-"}"#.to_string()];
        let mut board = timed(vec![card("a", "open")], &nameless);
        board.press(Key::Char('t'));
        assert_shows(&board, &[NO_TIME]);
    }

    #[test]
    fn a_missing_log_is_an_empty_one() {
        let project = Project::new();
        project.write(".gdd/changes/a/change.json", r#"{"title": "A"}"#);
        let mut board = Board::load(&project.0).unwrap();
        board.press(Key::Enter);
        assert_shows(&board, &[NOT_MEASURED]);
        press(&mut board, &[Key::Esc, Key::Char('t')]);
        assert_shows(&board, &[TIME_TITLE, NO_TIME]);
    }

    #[test]
    fn a_board_loads_the_log_of_the_state_root_once() {
        let project = Project::new();
        project.write(".gdd/changes/b/change.json", r#"{"title": "B"}"#);
        let log = [log_line("b", "08:00:00", 0), log_line("b", "08:30:00", 600)];
        project.write(".gdd/log.jsonl", &format!("{}\n", log.join("\n")));
        let mut board = Board::load(&project.0).unwrap();
        project.write(".gdd/log.jsonl", "{not json\n");
        board.press(Key::Enter);
        assert_shows(&board, &["Flow time 30m"]);
        press(&mut board, &[Key::Esc, Key::Char('t')]);
        assert_shows(&board, &["Project: 1 change(s)"]);
    }

    #[test]
    fn a_line_of_the_log_that_is_not_gdds_is_left_out_and_the_time_says_so() {
        let project = Project::new();
        project.write(".gdd/changes/a/change.json", r#"{"title": "A"}"#);
        let log = [log_line("a", "08:00:00", 0), log_line("a", "08:30:00", 600)];
        let text = format!("{{not json\n{}\n", log.join("\n"));
        project.write(".gdd/log.jsonl", &text);
        let mut board = Board::load(&project.0).unwrap();
        assert_shows(&board, &["Discover (1)", "› a ·"]);
        board.press(Key::Enter);
        assert_shows(&board, &["a · A", "Flow time 30m"]);
        assert_hides(&board, &["cannot be read"]);
        press(&mut board, &[Key::Esc, Key::Char('t')]);
        let said = "1 line of the log is not one of gdd's: left out";
        assert_shows(&board, &[TIME_TITLE, said, "Project: 1 change(s)"]);
        let rows = lines(&board);
        let at = |start: &str| rows.iter().position(|row| row.starts_with(start)).unwrap();
        assert!(at(said) < at("Project: "), "{rows:?}");
        // A log whose lines are all gdd's says nothing of it.
        project.write(".gdd/log.jsonl", &format!("{}\n", log.join("\n")));
        let mut board = Board::load(&project.0).unwrap();
        board.press(Key::Char('t'));
        assert_hides(&board, &["left out"]);
    }

    #[test]
    fn a_log_that_cannot_be_read_shows_where_the_time_would_and_the_list_works() {
        let project = Project::new();
        project.write(".gdd/changes/a/change.json", r#"{"title": "A"}"#);
        fs::create_dir_all(project.0.join(".gdd/log.jsonl")).unwrap();
        let mut board = Board::load(&project.0).unwrap();
        assert_shows(&board, &["Discover (1)", "› a ·"]);
        let why = "the log cannot be read: cannot read .gdd/log.jsonl: ";
        board.press(Key::Enter);
        assert_shows(&board, &["a · A", why]);
        assert_hides(&board, &[NOT_MEASURED]);
        press(&mut board, &[Key::Esc, Key::Char('t')]);
        assert_shows(&board, &[TIME_TITLE, why]);
        assert_hides(&board, &[NO_TIME, "Project:"]);
    }

    #[test]
    fn t_does_nothing_in_the_details() {
        let mut board = timed(vec![card("a", "open")], &busy_log(1));
        board.press(Key::Enter);
        let details = board.render(100, 40);
        board.press(Key::Char('t'));
        assert_eq!(board.render(100, 40), details);
        assert_shows(&board, &[DETAIL_KEYS]);
        assert_hides(&board, &[TIME_TITLE]);
    }

    #[test]
    fn esc_from_the_time_keeps_the_selection_and_done_as_they_were() {
        let cards = vec![card("a", "open"), card("b", "open"), card("c", "closed")];
        let mut board = timed(cards, &busy_log(1));
        press(&mut board, &[Key::Char('d'), Key::Down, Key::Down]);
        let before = board.render(100, 40);
        press(&mut board, &[Key::Char('t'), Key::Esc]);
        assert_eq!(board.render(100, 40), before);
        assert_shows(&board, &["› c ·", "Done (1) · d to hide"]);
        press(&mut board, &[Key::Char('d'), Key::Char('t'), Key::Esc]);
        assert_shows(&board, &["Done (1) · d to show", "› b ·"]);
        assert_hides(&board, &[TIME_TITLE]);
    }

    #[test]
    fn q_and_ctrl_c_close_the_board_from_the_time() {
        for key in [Key::Char('q'), Key::CtrlC] {
            let mut board = board(vec![card("a", "open")]);
            press(&mut board, &[Key::Char('t'), key]);
            assert!(board.closed());
        }
    }

    #[test]
    fn the_board_is_a_screen_that_is_done_when_it_closes() {
        let mut board = board(vec![card("a", "open"), card("b", "open")]);
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let keys = [None, Some(Key::Down), None, Some(Key::Char('q'))];
        let mut keys = keys.into_iter();
        // Reading fails once the keys run out, so a board that never closes fails the test.
        let no_more = || io::Error::other("no more keys");
        let next = || keys.next().ok_or_else(no_more);
        let ended = tui::run(&mut board, &mut terminal, next);
        assert_eq!(ended, Ok(()));
        assert!(board.closed());
        let shown = tui::screen_text(terminal.backend().buffer());
        assert!(shown.contains("› b ·"), "{shown}");
    }
}
