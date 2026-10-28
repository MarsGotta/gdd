//! What the screens of the terminal share: the keys ([`Key`]), the loop that draws a [`Screen`]
//! and gives it each key ([`run`]), the terminal it runs in ([`in_terminal`]) and the text of what
//! was drawn ([`screen_text`]).

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::Backend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::text::Span;
use ratatui::{Frame, Terminal};

/// A key the human presses on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A letter, a digit or a sign: typed in a text, `y` on the summary of the form.
    Char(char),
    Enter,
    Esc,
    Up,
    Down,
    /// Checks or unchecks in a checklist; in a text, a space.
    Space,
    Backspace,
    CtrlC,
}

/// A screen of the terminal: it draws itself, takes keys and says when the human is done.
pub(crate) trait Screen {
    /// What the screen is called in the message of a drawing that failed: `the form`, `the board`.
    const NAME: &'static str;

    /// Draws the screen on `frame`.
    fn draw(&self, frame: &mut Frame);

    /// The human presses `key`.
    fn press(&mut self, key: Key);

    /// Whether the human is done with the screen.
    fn done(&self) -> bool;
}

/// Shows `screen` on `terminal` and gives it each key `next_key` reads (`None` for one the
/// screen does not take) until it is done; else why it could not.
pub(crate) fn run<S: Screen, B: Backend>(
    screen: &mut S,
    terminal: &mut Terminal<B>,
    mut next_key: impl FnMut() -> io::Result<Option<Key>>,
) -> Result<(), String> {
    while !screen.done() {
        let drawn = terminal.draw(|frame| screen.draw(frame));
        drawn.map_err(|e| format!("cannot draw {}: {e}", S::NAME))?;
        let key = next_key().map_err(|e| format!("cannot read a key: {e}"))?;
        if let Some(key) = key {
            screen.press(key);
        }
    }
    Ok(())
}

/// Shows `screen` in the terminal of the process, in raw mode on the alternate screen, which are
/// put back as they were on every way out, a panic too, until it is done: the screen as the human
/// left it; else why it could not.
pub(crate) fn in_terminal<S: Screen>(mut screen: S) -> Result<S, String> {
    let _restore = Restore(ratatui::restore);
    let cannot = |e: io::Error| format!("cannot use the terminal: {e}");
    let mut terminal = ratatui::try_init().map_err(cannot)?;
    run(&mut screen, &mut terminal, || event::read().map(key_of))?;
    Ok(screen)
}

/// Puts the terminal back as it was when it goes, on every way out of a screen.
struct Restore(fn());

impl Drop for Restore {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// The key a terminal `event` is: a key pressed, if the screens take it.
fn key_of(event: Event) -> Option<Key> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => key_pressed(key),
        _ => None,
    }
}

/// The key `key` is; with Ctrl, only Ctrl-C.
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
pub(crate) fn screen_text(buffer: &Buffer) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::widgets::Paragraph;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A screen that counts the keys it is given and is done at `q`.
    struct Counter {
        pressed: Vec<Key>,
    }

    impl Screen for Counter {
        const NAME: &'static str = "the counter";

        fn draw(&self, frame: &mut Frame) {
            let text = format!("keys: {}", self.pressed.len());
            frame.render_widget(Paragraph::new(text), frame.area());
        }

        fn press(&mut self, key: Key) {
            self.pressed.push(key);
        }

        fn done(&self) -> bool {
            self.pressed.last() == Some(&Key::Char('q'))
        }
    }

    fn counter() -> Counter {
        Counter {
            pressed: Vec::new(),
        }
    }

    #[test]
    fn run_shows_the_screen_and_gives_it_each_key_until_it_is_done() {
        let mut screen = counter();
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        let keys = [None, Some(Key::Down), None, Some(Key::Char('q'))];
        let mut keys = keys.into_iter();
        // Reading fails once the keys run out, so a screen that never ends fails the test.
        let no_more = || io::Error::other("no more keys");
        let ended = run(&mut screen, &mut terminal, || {
            keys.next().ok_or_else(no_more)
        });
        assert_eq!(ended, Ok(()));
        assert_eq!(screen.pressed, [Key::Down, Key::Char('q')]);
        let shown = screen_text(terminal.backend().buffer());
        assert!(shown.starts_with("keys: 1 "), "{shown}");
    }

    #[test]
    fn run_reports_a_key_it_cannot_read() {
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        let gone = || Err(io::Error::other("gone"));
        let failed = run(&mut counter(), &mut terminal, gone);
        assert_eq!(failed, Err("cannot read a key: gone".to_string()));
    }

    #[test]
    fn run_reads_nothing_from_a_screen_that_is_done() {
        let mut screen = counter();
        screen.press(Key::Char('q'));
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
        let gone = || Err(io::Error::other("gone"));
        assert_eq!(run(&mut screen, &mut terminal, gone), Ok(()));
        assert_eq!(screen.pressed, [Key::Char('q')]);
    }

    #[test]
    fn a_key_pressed_in_the_terminal_is_a_key_of_the_screens() {
        let none = KeyModifiers::NONE;
        let pressed = |code, modifiers| key_of(Event::Key(KeyEvent::new(code, modifiers)));
        for (code, key) in [
            (KeyCode::Enter, Some(Key::Enter)),
            (KeyCode::Esc, Some(Key::Esc)),
            (KeyCode::Up, Some(Key::Up)),
            (KeyCode::Down, Some(Key::Down)),
            (KeyCode::Backspace, Some(Key::Backspace)),
            (KeyCode::Char(' '), Some(Key::Space)),
            (KeyCode::Char('c'), Some(Key::Char('c'))),
            (KeyCode::Char('ñ'), Some(Key::Char('ñ'))),
            (KeyCode::Tab, None),
        ] {
            assert_eq!(pressed(code, none), key, "{code:?}");
        }
        let control = KeyModifiers::CONTROL;
        assert_eq!(pressed(KeyCode::Char('c'), control), Some(Key::CtrlC));
        assert_eq!(pressed(KeyCode::Char('a'), control), None);
        assert_eq!(pressed(KeyCode::Enter, control), None);
        let shift = KeyModifiers::SHIFT;
        assert_eq!(pressed(KeyCode::Char('N'), shift), Some(Key::Char('N')));
        let kind = KeyEventKind::Release;
        let released = KeyEvent::new_with_kind(KeyCode::Enter, none, kind);
        assert_eq!(key_of(Event::Key(released)), None);
        assert_eq!(key_of(Event::FocusGained), None);
    }

    #[test]
    fn the_text_of_a_screen_hides_the_cells_a_wide_symbol_covers() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
        buffer.set_string(0, 0, "ab", ratatui::style::Style::default());
        buffer.set_string(0, 1, "界x", ratatui::style::Style::default());
        assert_eq!(screen_text(&buffer), "ab  \n界x ");
    }

    static RESTORED: AtomicBool = AtomicBool::new(false);

    fn restored() {
        RESTORED.store(true, Ordering::SeqCst);
    }

    #[test]
    fn the_terminal_is_put_back_when_the_guard_goes() {
        let guard = Restore(restored);
        assert!(!RESTORED.load(Ordering::SeqCst));
        drop(guard);
        assert!(RESTORED.load(Ordering::SeqCst));
    }
}
