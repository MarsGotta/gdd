//! The Board of `gdd board`: the changes of the project by phase, on a screen, read-only. The
//! Board is a state machine, keys in and a screen out ([`Board`]), drawn with Ratatui.
//! [`in_terminal`] shows it in the terminal of the process. Nothing on it writes anything.

use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use gdd_core::state;
use gdd_core::status::{self, Card};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::form::Key;

/// The state that reads `waiting for your G4` instead of its name.
const IN_PRE: &str = "in-pre";

/// What a change in pre reads instead of its state.
const WAITING: &str = "waiting for your G4";

/// What the Board says of a project without changes.
const NO_CHANGES: &str = "No changes yet · start one with gdd new-change <id> --title <what>";

/// The keys of the list.
const LIST_KEYS: &str = "↑↓ move · Enter details · d done · q quit";

/// The keys of the details.
const DETAIL_KEYS: &str = "Esc to go back · q quit";

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

/// The Board: the changes of a project, the one selected and what is on the screen.
#[derive(Debug)]
pub struct Board {
    project: String,
    cards: Vec<Card>,
    /// Whether the changes of Done show.
    done_shown: bool,
    /// The one selected, among the changes that show.
    selected: usize,
    /// Whether the screen is the details of the selected change.
    details: bool,
    closed: bool,
}

impl Board {
    /// A Board of the project called `project` with its changes `cards`, the first one selected,
    /// Done folded.
    pub fn new(project: &str, cards: Vec<Card>) -> Board {
        Board {
            project: project.to_string(),
            cards,
            done_shown: false,
            selected: 0,
            details: false,
            closed: false,
        }
    }

    /// The Board of the project that holds the folder `cwd`, with the changes under its state
    /// root, as every command finds them.
    pub fn load(cwd: &Path) -> Result<Board, String> {
        let root = crate::project_root(cwd)?;
        let state = state::root(&root)?;
        let project = crate::read(&root, crate::PROJECT)?.unwrap_or_default();
        let basics =
            status::Basics::from_toml(&project).map_err(|e| format!("{}: {e}", crate::PROJECT))?;
        Ok(Board::new(&basics.name, status::cards(&root, &state)))
    }

    /// The human presses `key`: q or Ctrl-C closes the Board; on the list, Up and Down move the
    /// selection within the changes that show, Enter opens the details of the selected one and
    /// `d` shows or folds Done; in the details, Esc goes back. Once closed, keys do nothing.
    pub fn press(&mut self, key: Key) {
        if self.closed {
            return;
        }
        let last = self.shown().len().saturating_sub(1);
        match (key, self.details) {
            (Key::CtrlC | Key::Char('q'), _) => self.closed = true,
            (Key::Esc, true) => self.details = false,
            (Key::Enter, false) => self.details = self.selected_card().is_some(),
            (Key::Up, false) => self.selected = self.selected.saturating_sub(1),
            (Key::Down, false) => self.selected = (self.selected + 1).min(last),
            (Key::Char('d'), false) => {
                self.done_shown = !self.done_shown;
                self.selected = self.selected.min(self.shown().len().saturating_sub(1));
            }
            _ => {}
        }
    }

    /// The screen as text, `width` x `height` cells, its lines joined by `\n`.
    pub fn render(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        self.paint(area, &mut buffer);
        screen_text(&buffer)
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
        let (lines, selected, keys_text) = if self.details {
            (self.detail_lines(), None, DETAIL_KEYS)
        } else {
            let (lines, selected) = self.list_lines();
            (lines, selected, LIST_KEYS)
        };
        let offset = scroll(&lines, selected, body);
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((offset, 0))
            .render(body, buffer);
        Paragraph::new(Line::from(keys_text).dim()).render(keys, buffer);
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
        lines
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
    // Wrapping at words can take up to twice the rows of the plain division.
    let room = (line.width() / usize::from(width)) * 2 + 2;
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
    let shown = Board::load(cwd).and_then(|mut board| {
        let _restore = Restore(ratatui::restore);
        let cannot = |e: io::Error| format!("cannot use the terminal: {e}");
        let mut terminal = ratatui::try_init().map_err(cannot)?;
        drive(&mut board, &mut terminal, || event::read().map(key_of))
    });
    match shown {
        Ok(()) => crate::Outcome::success(String::new()),
        Err(why) => crate::Outcome::error(&why),
    }
}

/// Puts the terminal back as it was when it goes, on every way out of the Board.
struct Restore(fn());

impl Drop for Restore {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// The key of the Board a terminal `event` is: a key pressed, if the Board takes it.
fn key_of(event: Event) -> Option<Key> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => key_pressed(key),
        _ => None,
    }
}

/// The key of the Board `key` is; with Ctrl, only Ctrl-C.
fn key_pressed(key: KeyEvent) -> Option<Key> {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if control => Some(Key::CtrlC),
        _ if control => None,
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Char(' ') => Some(Key::Space),
        KeyCode::Char(letter) => Some(Key::Char(letter)),
        _ => None,
    }
}

/// The text of `buffer`, a line per row; a symbol wider than one cell hides the cells it covers.
fn screen_text(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width).max(1);
    let lines: Vec<String> = buffer.content.chunks(width).map(row_text).collect();
    lines.join("\n")
}

/// The text of a row of `cells` (see [`screen_text`]).
fn row_text(cells: &[Cell]) -> String {
    let mut text = String::new();
    let mut hidden = 0;
    for cell in cells {
        if hidden == 0 {
            text.push_str(cell.symbol());
        }
        let width = Span::raw(cell.symbol()).width();
        hidden = width.max(hidden).saturating_sub(1);
    }
    text
}

/// Shows `board` on `terminal` and gives it each key `next_key` reads (`None` for one the Board
/// does not take) until the human closes it.
fn drive<B: Backend>(
    board: &mut Board,
    terminal: &mut Terminal<B>,
    mut next_key: impl FnMut() -> io::Result<Option<Key>>,
) -> Result<(), String> {
    while !board.closed() {
        let drawn = terminal.draw(|frame| {
            let area = frame.area();
            board.paint(area, frame.buffer_mut());
        });
        drawn.map_err(|e| format!("cannot draw the board: {e}"))?;
        let key = next_key().map_err(|e| format!("cannot read a key: {e}"))?;
        if let Some(key) = key {
            board.press(key);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Project;
    use ratatui::backend::TestBackend;
    use std::fs;
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

    fn press_of(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Event {
        let mut key = KeyEvent::new(code, modifiers);
        key.kind = kind;
        Event::Key(key)
    }

    #[test]
    fn a_terminal_event_is_a_key_of_the_board_only_when_it_is_pressed_and_taken() {
        let (none, control) = (KeyModifiers::NONE, KeyModifiers::CONTROL);
        let pressed = |code, modifiers| key_of(press_of(code, modifiers, KeyEventKind::Press));
        assert_eq!(pressed(KeyCode::Char('c'), control), Some(Key::CtrlC));
        assert_eq!(pressed(KeyCode::Char('c'), none), Some(Key::Char('c')));
        assert_eq!(pressed(KeyCode::Char('d'), control), None);
        assert_eq!(pressed(KeyCode::Enter, control), None);
        assert_eq!(pressed(KeyCode::Enter, none), Some(Key::Enter));
        assert_eq!(pressed(KeyCode::Esc, none), Some(Key::Esc));
        assert_eq!(pressed(KeyCode::Up, none), Some(Key::Up));
        assert_eq!(pressed(KeyCode::Down, none), Some(Key::Down));
        assert_eq!(pressed(KeyCode::Backspace, none), Some(Key::Backspace));
        assert_eq!(pressed(KeyCode::Char(' '), none), Some(Key::Space));
        assert_eq!(pressed(KeyCode::Char('q'), none), Some(Key::Char('q')));
        assert_eq!(pressed(KeyCode::Tab, none), None);
        let released = press_of(KeyCode::Enter, none, KeyEventKind::Release);
        assert_eq!(key_of(released), None);
        assert_eq!(key_of(Event::FocusGained), None);
    }

    #[test]
    fn the_text_of_a_screen_hides_the_cells_a_wide_symbol_covers() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
        buffer.set_string(0, 0, "ab", ratatui::style::Style::default());
        buffer.set_string(0, 1, "界x", ratatui::style::Style::default());
        assert_eq!(screen_text(&buffer), "ab  \n界x ");
    }

    #[test]
    fn the_terminal_is_put_back_when_the_guard_goes() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static RESTORED: AtomicBool = AtomicBool::new(false);
        fn restore() {
            RESTORED.store(true, Ordering::SeqCst);
        }
        drop(Restore(restore));
        assert!(RESTORED.load(Ordering::SeqCst));
    }

    #[test]
    fn drive_shows_the_board_and_gives_it_each_key_until_it_closes() {
        let mut board = board(vec![card("a", "open"), card("b", "open")]);
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let keys = [None, Some(Key::Down), None, Some(Key::Char('q'))];
        let mut keys = keys.into_iter();
        // Reading fails once the keys run out, so a board that never closes fails the test.
        let no_more = || io::Error::other("no more keys");
        let next = || keys.next().ok_or_else(no_more);
        let ended = drive(&mut board, &mut terminal, next);
        assert_eq!(ended, Ok(()));
        let shown = screen_text(terminal.backend().buffer());
        assert!(shown.contains("› b ·"), "{shown}");
        let mut board = Board::new("demo", Vec::new());
        let gone = || Err(io::Error::other("gone"));
        let failed = drive(&mut board, &mut terminal, gone);
        assert_eq!(failed, Err("cannot read a key: gone".to_string()));
    }
}
