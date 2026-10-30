//! The Setup form of `gdd init`: the deterministic part of the Setup, one step at a time. The form
//! is a state machine, keys in and a screen out ([`Form`]), drawn with Ratatui. [`in_terminal`]
//! asks it in the terminal of the process; [`finish`] does what the human answered, exactly as
//! the flags would.

use std::path::Path;

use gdd_core::agents::sheets;
use gdd_core::init;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::tui::{self, Screen};

pub use crate::tui::Key;

/// The stacks of the catalog, in its order; `Not sure` comes after them.
const STACKS: [&str; 3] = ["rust", "typescript", "python"];

/// The last of the stacks: the human does not know the technology yet.
const NOT_SURE: &str = "Not sure";

/// The language the agents use with the human unless they type another.
const LANGUAGE: &str = "en";

/// Why Enter on the first step stays there.
const NO_NAME: &str = "Type a name: the project needs one";

/// Why Enter on the stacks stays there.
const NO_STACK: &str = "Pick at least one stack, or Not sure";

/// Why Enter on the coding agents stays there.
const NO_AGENT: &str = "Pick at least one coding agent";

/// What the summary asks when every coding agent checked is of one model family.
const SAME_FAMILY: &str =
    "The Reviewer would be of the same model family as the Builder: press y to accept it";

/// What the human does with the flags command when they are not sure of the technology.
const ASK_AGENT: &str = "Your coding agent can help you choose it.";

/// The last line once the project is set up and its stacks installed.
const NEXT_AGENT: &str = "Next: start your coding agent here and run the gdd-setup command \
                          (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).";

/// What the human does when the install of the stacks failed after the setup.
const INSTALL_FAILED: &str =
    "The harness is set up and committed. Fix the install, then run gdd bootstrap.";

/// Why the form ended without writing anything.
const QUIT: &str = "gdd init stopped: nothing was written";

/// The keys of the steps that take text.
const TEXT_KEYS: &str = "Enter: next · Esc: back · Ctrl-C: quit";

/// The keys of the steps with a checklist.
const CHECK_KEYS: &str = "Up/Down: move · Space: check · Enter: next · Esc: back · Ctrl-C: quit";

/// How the human finished the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// They set the project up with these answers.
    SetUp(init::Options),
    /// They are not sure of the technology: the flags command with every other answer.
    NotSure(String),
    /// They quit with Ctrl-C.
    Quit,
}

/// A step of the form, in order, then the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Name,
    Description,
    Platform,
    Stacks,
    Agents,
    Language,
    Sensitive,
    Summary,
}

impl Step {
    /// The step Enter goes to.
    fn next(self) -> Step {
        match self {
            Step::Name => Step::Description,
            Step::Description => Step::Platform,
            Step::Platform => Step::Stacks,
            Step::Stacks => Step::Agents,
            Step::Agents => Step::Language,
            Step::Language => Step::Sensitive,
            Step::Sensitive | Step::Summary => Step::Summary,
        }
    }

    /// The step Esc goes back to; the first one stays.
    fn back(self) -> Step {
        match self {
            Step::Name | Step::Description => Step::Name,
            Step::Platform => Step::Description,
            Step::Stacks => Step::Platform,
            Step::Agents => Step::Stacks,
            Step::Language => Step::Agents,
            Step::Sensitive => Step::Language,
            Step::Summary => Step::Sensitive,
        }
    }

    /// Its title, with its number.
    fn title(self) -> &'static str {
        match self {
            Step::Name => "Step 1 of 7 · Project name",
            Step::Description => "Step 2 of 7 · Description",
            Step::Platform => "Step 3 of 7 · Platform",
            Step::Stacks => "Step 4 of 7 · Stacks",
            Step::Agents => "Step 5 of 7 · Coding agents",
            Step::Language => "Step 6 of 7 · Language",
            Step::Sensitive => "Step 7 of 7 · Sensitive areas",
            Step::Summary => "Summary",
        }
    }

    /// What it asks.
    fn asks(self) -> &'static str {
        match self {
            Step::Name => "What is the project called?",
            Step::Description => "What is it, in a few words? It may stay empty.",
            Step::Platform => "Where does it run?",
            Step::Stacks => "Which technology does it use? Pick one or more, or Not sure.",
            Step::Agents => {
                "Which coding agents? The first checked builds; those found on this machine come \
                 checked."
            }
            Step::Language => "Which language do the agents use with you? (en, es...)",
            Step::Sensitive => "What would be serious if it failed? One area per line.",
            Step::Summary => "Every answer, before anything is written.",
        }
    }

    /// The keys it takes.
    fn keys(self) -> &'static str {
        match self {
            Step::Name => "Enter: next · Ctrl-C: quit",
            Step::Description | Step::Language => TEXT_KEYS,
            Step::Platform => "Up/Down: move · Enter: pick · Esc: back · Ctrl-C: quit",
            Step::Stacks | Step::Agents => CHECK_KEYS,
            Step::Sensitive => {
                "Enter: add, or go on when empty · Backspace: edit the last · Esc: back · \
                 Ctrl-C: quit"
            }
            Step::Summary => "Ctrl-C: quit",
        }
    }
}

/// A list of the form: its items, which of them are checked and the one at the cursor.
#[derive(Debug)]
struct List {
    items: Vec<String>,
    checked: Vec<bool>,
    cursor: usize,
}

impl List {
    /// The `items`, checked when `checked` says so, with the cursor on the first.
    fn of<'a>(items: impl IntoIterator<Item = &'a str>, checked: impl Fn(&str) -> bool) -> List {
        let items: Vec<String> = items.into_iter().map(str::to_string).collect();
        let checked = items.iter().map(|item| checked(item)).collect();
        List {
            items,
            checked,
            cursor: 0,
        }
    }

    /// Up and Down move the cursor within the list; Space checks or unchecks its item.
    fn press(&mut self, key: Key) {
        match key {
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down => self.cursor = (self.cursor + 1).min(self.items.len() - 1),
            Key::Space => {
                if let Some(checked) = self.checked.get_mut(self.cursor) {
                    *checked = !*checked;
                }
            }
            _ => {}
        }
    }

    /// The item at the cursor.
    fn current(&self) -> String {
        self.items.get(self.cursor).cloned().unwrap_or_default()
    }

    /// The items checked, in order.
    fn picked(&self) -> Vec<String> {
        let items = self.items.iter().zip(&self.checked);
        let checked = items.filter(|(_, checked)| **checked);
        checked.map(|(item, _)| item.clone()).collect()
    }

    /// A line per item, `>` at the cursor, with a box checked or not when `boxes`.
    fn lines(&self, boxes: bool) -> Vec<String> {
        let mut lines = Vec::new();
        for (at, (item, &checked)) in self.items.iter().zip(&self.checked).enumerate() {
            let cursor = if at == self.cursor { ">" } else { " " };
            let mark = match (boxes, checked) {
                (false, _) => "",
                (true, true) => "[x] ",
                (true, false) => "[ ] ",
            };
            lines.push(format!("{cursor} {mark}{item}"));
        }
        lines
    }
}

/// The Setup form: the answers so far, the step it is at and, once the human finished, how.
#[derive(Debug)]
pub struct Form {
    step: Step,
    name: String,
    description: String,
    platform: List,
    stacks: List,
    agents: List,
    language: String,
    /// The sensitive areas added, one per line.
    sensitive: Vec<String>,
    /// The sensitive area being typed.
    area: String,
    /// What the step still needs, after an Enter that could not go on.
    problem: Option<&'static str>,
    outcome: Option<Outcome>,
}

impl Form {
    /// A new form, on its first step. `agents`: the coding agents found on the `PATH` (claude,
    /// codex, opencode, in that order), which come checked.
    pub fn new(agents: Vec<String>) -> Form {
        Form {
            step: Step::Name,
            name: String::new(),
            description: String::new(),
            platform: List::of(init::PLATFORMS, |_| false),
            stacks: List::of(STACKS.into_iter().chain([NOT_SURE]), |_| false),
            agents: agent_list(&agents),
            language: LANGUAGE.to_string(),
            sensitive: Vec::new(),
            area: String::new(),
            problem: None,
            outcome: None,
        }
    }

    /// The form with the answers the flags of `gdd init` already gave in `given`, to change as
    /// any other: the coding agents they name come checked instead of those found (in the order
    /// of the list), and the language, the description and the sensitive areas are theirs. The
    /// name, the platform and the stacks are left to answer, and the yes to one model family to
    /// give on the summary.
    fn answered(mut self, given: &init::Options) -> Form {
        if !given.agents.is_empty() {
            self.agents = agent_list(&given.agents);
        }
        self.language.clone_from(&given.language);
        self.description.clone_from(&given.description);
        self.sensitive.clone_from(&given.sensitive);
        self
    }

    /// The human presses `key`: Ctrl-C quits, Esc goes back one step keeping the answers, Enter
    /// goes on when the step is answered, and the other keys answer it. Once the form has an
    /// outcome, keys do nothing.
    pub fn press(&mut self, key: Key) {
        if self.outcome.is_some() {
            return;
        }
        self.problem = None;
        match key {
            Key::CtrlC => self.outcome = Some(Outcome::Quit),
            Key::Esc => self.step = self.step.back(),
            Key::Enter => self.enter(),
            other => self.edit(other),
        }
    }

    /// The screen as text, `width` x `height` cells, its lines joined by `\n`.
    pub fn render(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        self.screen().render(area, &mut buffer);
        tui::screen_text(&buffer)
    }

    /// How the human finished the form: Enter on the summary (`y` when every coding agent is of
    /// one model family), or Ctrl-C; `None` until then.
    pub fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    /// Enter: the next step when this one is answered, else what it still needs; a name of
    /// spaces only is none. On the last step it adds the area typed, and goes on from a line
    /// empty or of spaces only; on the summary it ends.
    fn enter(&mut self) {
        match self.step {
            Step::Name if self.name.trim().is_empty() => self.problem = Some(NO_NAME),
            Step::Stacks if self.stacks.picked().is_empty() => self.problem = Some(NO_STACK),
            Step::Agents if self.agents.picked().is_empty() => self.problem = Some(NO_AGENT),
            Step::Sensitive if !self.area.trim().is_empty() => self.add_area(),
            Step::Summary => self.end(),
            step => self.step = step.next(),
        }
    }

    /// A key that answers the step.
    fn edit(&mut self, key: Key) {
        match self.step {
            Step::Name => type_in(&mut self.name, key),
            Step::Description => type_in(&mut self.description, key),
            Step::Platform => self.platform.press(key),
            Step::Stacks => self.stacks.press(key),
            Step::Agents => self.agents.press(key),
            Step::Language => type_in(&mut self.language, key),
            Step::Sensitive => self.edit_area(key),
            Step::Summary => self.accept(key),
        }
    }

    /// A key on the last step: Backspace on an empty line takes back the last area to edit it.
    fn edit_area(&mut self, key: Key) {
        if key == Key::Backspace && self.area.is_empty() {
            self.area = self.sensitive.pop().unwrap_or_default();
        } else {
            type_in(&mut self.area, key);
        }
    }

    /// Adds the area typed, as it is, to the sensitive ones.
    fn add_area(&mut self) {
        self.sensitive.push(std::mem::take(&mut self.area));
    }

    /// Enter on the summary: the flags command when the human is not sure of the technology;
    /// else the project is set up, unless every coding agent is of one model family, which
    /// waits for `y`.
    fn end(&mut self) {
        if self.not_sure() {
            self.outcome = Some(Outcome::NotSure(self.command()));
        } else if !self.one_family() {
            self.outcome = Some(Outcome::SetUp(self.options(false)));
        }
    }

    /// `y` on the summary that waits for it: the human accepts one model family, and the
    /// project is set up.
    fn accept(&mut self, key: Key) {
        if key == Key::Char('y') && !self.not_sure() && self.one_family() {
            self.outcome = Some(Outcome::SetUp(self.options(true)));
        }
    }

    /// Whether the human checked Not sure among the stacks.
    fn not_sure(&self) -> bool {
        self.stacks.picked().iter().any(|stack| stack == NOT_SURE)
    }

    /// Whether every coding agent checked is of one model family (see [`init::family_of`]).
    fn one_family(&self) -> bool {
        let agents = self.agents.picked();
        let families: Vec<Option<String>> = agents.iter().map(|a| init::family_of(a)).collect();
        families.windows(2).all(|pair| pair[0] == pair[1])
    }

    /// The answers as gdd init takes them: the text as typed, which it escapes, and the stacks
    /// without Not sure.
    fn options(&self, accept_same_family: bool) -> init::Options {
        let stacks = self.stacks.picked().into_iter();
        init::Options {
            name: self.name.clone(),
            platform: self.platform.current(),
            stacks: stacks.filter(|stack| stack != NOT_SURE).collect(),
            agents: self.agents.picked(),
            language: self.language(),
            description: self.description.clone(),
            sensitive: self.sensitive.clone(),
            accept_same_family,
        }
    }

    /// The language typed, `en` when none is (or only spaces).
    fn language(&self) -> String {
        if self.language.trim().is_empty() {
            LANGUAGE.to_string()
        } else {
            self.language.clone()
        }
    }

    /// The flags command with every answer but the stacks, to run once the human knows them;
    /// gdd init reads each answer from it as typed (see [`flag`]).
    fn command(&self) -> String {
        let options = self.options(false);
        let mut flags = vec![
            "gdd init".to_string(),
            flag("--name", &options.name),
            flag("--platform", &options.platform),
            format!("--agents {}", options.agents.join(" ")),
            flag("--language", &options.language),
        ];
        if !options.description.is_empty() {
            flags.push(flag("--description", &options.description));
        }
        let areas = options.sensitive.iter();
        flags.extend(areas.map(|area| flag("--sensitive", area)));
        flags.push("--stack <pick one>".to_string());
        flags.join(" ")
    }

    /// The screen: the step, framed with its title, what it asks, its answer, what it still
    /// needs and its keys.
    fn screen(&self) -> Paragraph<'static> {
        let step = self.step;
        let title = format!(" gdd init · {} ", step.title());
        let mut lines = vec![Line::from(step.asks()), Line::default()];
        lines.extend(self.body().into_iter().map(Line::from));
        if let Some(problem) = self.problem {
            lines.extend([Line::default(), Line::from(problem).red()]);
        }
        lines.extend([Line::default(), Line::from(step.keys()).dim()]);
        let paragraph = Paragraph::new(lines).block(Block::bordered().title(title));
        paragraph.wrap(Wrap { trim: false })
    }

    /// The lines of the answer of the step.
    fn body(&self) -> Vec<String> {
        match self.step {
            Step::Name => vec![field(&self.name)],
            Step::Description => vec![field(&self.description)],
            Step::Platform => self.platform.lines(false),
            Step::Stacks => self.stacks.lines(true),
            Step::Agents => self.agents.lines(true),
            Step::Language => vec![field(&self.language)],
            Step::Sensitive => self.areas(),
            Step::Summary => self.summary(),
        }
    }

    /// The areas added, then the one being typed.
    fn areas(&self) -> Vec<String> {
        let added = self.sensitive.iter().map(|area| format!("  - {area}"));
        added.chain([field(&self.area)]).collect()
    }

    /// Every answer and what Enter does; when the human is not sure of the technology, the
    /// flags command instead.
    fn summary(&self) -> Vec<String> {
        if self.not_sure() {
            return vec![
                "Not sure of the technology yet: once you know it, run".to_string(),
                String::new(),
                self.command(),
                String::new(),
                ASK_AGENT.to_string(),
                String::new(),
                "Enter to finish · Esc to go back".to_string(),
            ];
        }
        let options = self.options(false);
        let mut lines = vec![
            format!("Name: {}", options.name),
            format!("Description: {}", or_none(&options.description)),
            format!("Platform: {}", options.platform),
            format!("Stacks: {}", options.stacks.join(", ")),
            format!("Agents: {}", options.agents.join(", ")),
            format!("Language: {}", options.language),
            format!("Sensitive: {}", or_none(&options.sensitive.join("; "))),
            String::new(),
        ];
        if self.one_family() {
            lines.extend([SAME_FAMILY.to_string(), "Esc to go back".to_string()]);
        } else {
            lines.push("Enter to set up · Esc to go back".to_string());
        }
        lines
    }
}

/// What the CLI does once the form has an `outcome`, in the folder `root`, with git and the
/// agents on the `PATH` of `env`: sets the project up (see [`init::init`]), or gives the flags
/// command to run later; what it says, or why it wrote nothing.
pub fn finish(root: &Path, outcome: &Outcome, env: &[(String, String)]) -> Result<String, String> {
    match outcome {
        Outcome::SetUp(options) => set_up(root, options, env),
        Outcome::NotSure(command) => Ok(format!("{command}\n{ASK_AGENT}")),
        Outcome::Quit => Err(QUIT.to_string()),
    }
}

/// Sets the project up (see [`init::init`]) and installs its stacks (see [`init::bootstrap`]):
/// what both said, without the next step each gives, then the one for the person's agent. When
/// the install fails, the setup stays committed and the error says how to retry.
fn set_up(
    root: &Path,
    options: &init::Options,
    env: &[(String, String)],
) -> Result<String, String> {
    let set_up = init::init(root, options, env).map_err(|e| e.to_string())?;
    let mut lines = without_next(&set_up);
    match init::bootstrap(root, &options.stacks, env) {
        Ok(installed) => {
            lines.extend(without_next(&installed));
            lines.push(NEXT_AGENT.to_string());
            Ok(lines.join("\n"))
        }
        Err(stopped) => {
            lines.extend(without_next(&stopped.said));
            lines.extend([
                format!("error: {}", stopped.why),
                INSTALL_FAILED.to_string(),
            ]);
            Err(lines.join("\n"))
        }
    }
}

/// The lines of `text` but those that give the next step.
fn without_next(text: &str) -> Vec<String> {
    let lines = text.lines().filter(|line| !line.starts_with("Next:"));
    lines.map(str::to_string).collect()
}

/// `gdd init` in a terminal: asks the form (see [`ask`]), with the coding agents on the `PATH`
/// of the process checked and the answers its flags already gave in `given` (see
/// [`Form::answered`]), then does what the human answered in the folder `cwd` (see [`finish`]),
/// in the environment of the process.
pub fn in_terminal(cwd: &Path, given: &init::Options) -> crate::Outcome {
    let env = crate::process_env();
    let form = init::agents_on_path(&env).map(|found| Form::new(found).answered(given));
    match form
        .and_then(ask)
        .and_then(|outcome| finish(cwd, &outcome, &env))
    {
        Ok(said) => crate::Outcome::printed(said, 0),
        Err(why) => crate::Outcome::failure(format!("{why}\n")),
    }
}

/// Asks `form` in the terminal of the process (see [`tui::in_terminal`]): how the human finished
/// it.
fn ask(form: Form) -> Result<Outcome, String> {
    let mut form = tui::in_terminal(form)?;
    Ok(form.outcome.take().unwrap_or(Outcome::Quit))
}

impl Screen for Form {
    const NAME: &'static str = "the form";

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self.screen(), frame.area());
    }

    fn press(&mut self, key: Key) {
        Form::press(self, key);
    }

    fn done(&self) -> bool {
        self.outcome.is_some()
    }
}

/// A key on a step that takes text: a letter or Space types, Backspace deletes the last one.
fn type_in(text: &mut String, key: Key) {
    match key {
        Key::Char(letter) => text.push(letter),
        Key::Space => text.push(' '),
        Key::Backspace => {
            text.pop();
        }
        _ => {}
    }
}

/// A text field with what is typed in it.
fn field(text: &str) -> String {
    format!("> {text}█")
}

/// The coding agents of the form (claude, codex, opencode), those of `checked` checked.
fn agent_list(checked: &[String]) -> List {
    List::of(sheets::NAMES.split(", "), |name| {
        checked.iter().any(|agent| agent == name)
    })
}

/// `text`, or `none` when it is empty.
fn or_none(text: &str) -> &str {
    if text.is_empty() { "none" } else { text }
}

/// The option `name` with `value` as one word of the shell (see [`shell_word`]), which gdd init
/// reads whole as its value: after `=` when it starts with `-`, which it would take for an
/// option.
fn flag(name: &str, value: &str) -> String {
    let word = shell_word(value);
    if value.starts_with('-') {
        format!("{name}={word}")
    } else {
        format!("{name} {word}")
    }
}

/// `word` as the shell reads it: as it is when it has only letters, digits and `-_./,:@+=%`, else
/// between single quotes.
fn shell_word(word: &str) -> String {
    let plain = |c: char| c.is_alphanumeric() || "-_./,:@+=%".contains(c);
    if word.chars().all(plain) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Project, init_env, plain_folder};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    fn press(form: &mut Form, keys: &[Key]) {
        for key in keys {
            form.press(*key);
        }
    }

    fn type_text(form: &mut Form, text: &str) {
        for letter in text.chars() {
            form.press(Key::Char(letter));
        }
    }

    /// Asserts that the screen of `form` shows each of `texts`.
    fn assert_shows(form: &Form, texts: &[&str]) {
        let screen = form.render(100, 40);
        for text in texts {
            assert!(screen.contains(text), "no {text:?} in\n{screen}");
        }
    }

    /// Asserts that the screen of `form` shows none of `texts`.
    fn assert_hides(form: &Form, texts: &[&str]) {
        let screen = form.render(100, 40);
        for text in texts {
            assert!(!screen.contains(text), "{text:?} in\n{screen}");
        }
    }

    /// A form with the coding agents `found`, answered up to the stacks: Notes, its description
    /// and the platform cli.
    fn up_to_stacks(found: &[&str]) -> Form {
        let mut form = Form::new(names(found));
        type_text(&mut form, "Notes");
        form.press(Key::Enter);
        type_text(&mut form, "Personal notes");
        press(&mut form, &[Key::Enter, Key::Down, Key::Down]);
        press(&mut form, &[Key::Down, Key::Enter]);
        form
    }

    /// The same form, then rust, the agents found, the language and no sensitive area: on the
    /// summary.
    fn on_the_summary(found: &[&str]) -> Form {
        let mut form = up_to_stacks(found);
        form.press(Key::Space);
        press(&mut form, &[Key::Enter; 4]);
        form
    }

    /// What the form of [`on_the_summary`] sets up, with the coding agents `agents`.
    fn notes(agents: &[&str], accept_same_family: bool) -> init::Options {
        init::Options {
            name: "Notes".to_string(),
            platform: "cli".to_string(),
            stacks: names(&["rust"]),
            agents: names(agents),
            language: "en".to_string(),
            description: "Personal notes".to_string(),
            sensitive: Vec::new(),
            accept_same_family,
        }
    }

    #[test]
    fn the_form_starts_with_the_project_name() {
        let form = Form::new(Vec::new());
        let title = " gdd init · Step 1 of 7 · Project name ";
        assert_shows(&form, &[title, "What is the project called?"]);
        assert_shows(&form, &["\n│> █", "Enter: next · Ctrl-C: quit"]);
        assert_hides(&form, &["Esc"]);
        assert_eq!(form.outcome(), None);
    }

    #[test]
    fn enter_without_a_name_stays_and_says_so() {
        let mut form = Form::new(Vec::new());
        form.press(Key::Enter);
        assert_shows(&form, &["Step 1 of 7", NO_NAME]);
        press(&mut form, &[Key::Space, Key::Enter]);
        assert_shows(&form, &["Step 1 of 7", NO_NAME]);
        type_text(&mut form, "Notes");
        assert_hides(&form, &[NO_NAME]);
        form.press(Key::Enter);
        assert_shows(&form, &["Step 2 of 7 · Description", TEXT_KEYS]);
        assert_shows(&form, &["It may stay empty.", "\n│> █"]);
    }

    #[test]
    fn letters_and_space_type_and_backspace_deletes_the_last() {
        let mut form = Form::new(Vec::new());
        type_text(&mut form, "Nota");
        press(&mut form, &[Key::Backspace, Key::Up, Key::Down]);
        type_text(&mut form, "es");
        press(&mut form, &[Key::Space, Key::Char(' '), Key::Char('a')]);
        assert_shows(&form, &["> Notes  a█"]);
    }

    #[test]
    fn esc_goes_back_one_step_and_keeps_the_answers() {
        let mut form = on_the_summary(&["claude", "codex"]);
        assert_shows(&form, &[" gdd init · Summary ", "Ctrl-C: quit"]);
        for title in [
            "Step 7 of 7 · Sensitive areas",
            "Step 6 of 7 · Language",
            "Step 5 of 7 · Coding agents",
            "Step 4 of 7 · Stacks",
            "Step 3 of 7 · Platform",
            "Step 2 of 7 · Description",
            "Step 1 of 7 · Project name",
            "Step 1 of 7 · Project name",
        ] {
            form.press(Key::Esc);
            assert_shows(&form, &[title]);
        }
        assert_shows(&form, &["> Notes█"]);
        press(&mut form, &[Key::Enter; 7]);
        assert_shows(&form, &["Name: Notes", "Platform: cli"]);
        assert_shows(&form, &["Description: Personal notes"]);
        assert_eq!(form.outcome(), None);
    }

    #[test]
    fn up_and_down_move_in_the_platforms_within_their_ends() {
        let mut form = Form::new(names(&["claude", "codex"]));
        type_text(&mut form, "Notes");
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["Step 3 of 7", "Where does it run?"]);
        assert_shows(&form, &["\n│> mobile", "\n│  desktop"]);
        assert_shows(&form, &["Up/Down: move · Enter: pick · Esc: back"]);
        press(&mut form, &[Key::Up, Key::Down, Key::Down]);
        assert_shows(&form, &["\n│  mobile", "\n│> web"]);
        press(&mut form, &[Key::Up, Key::Space]);
        assert_shows(&form, &["\n│> desktop", "\n│  web"]);
        press(&mut form, &[Key::Down; 6]);
        assert_shows(&form, &["\n│  cli", "\n│> other"]);
        press(&mut form, &[Key::Enter, Key::Space]);
        press(&mut form, &[Key::Enter; 4]);
        assert_shows(&form, &["Platform: other"]);
    }

    #[test]
    fn a_stack_or_not_sure_is_needed_and_space_checks_or_unchecks() {
        let mut form = up_to_stacks(&["claude", "codex"]);
        assert_shows(&form, &["Step 4 of 7 · Stacks", "Not sure.", CHECK_KEYS]);
        assert_shows(&form, &["> [ ] rust", "  [ ] Not sure"]);
        form.press(Key::Enter);
        assert_shows(&form, &["Step 4 of 7", NO_STACK]);
        press(&mut form, &[Key::Space, Key::Space]);
        assert_shows(&form, &["> [ ] rust"]);
        assert_hides(&form, &[NO_STACK]);
        let keys = [Key::Space, Key::Down, Key::Down, Key::Space, Key::Up];
        press(&mut form, &keys);
        assert_shows(&form, &["  [x] rust", "> [ ] typescript", "  [x] python"]);
        press(&mut form, &[Key::Enter; 4]);
        assert_shows(&form, &["Stacks: rust, python"]);
    }

    #[test]
    fn the_agents_found_come_checked_and_one_is_needed() {
        let mut form = up_to_stacks(&["codex"]);
        press(&mut form, &[Key::Space, Key::Enter]);
        assert_shows(&form, &["Step 5 of 7 · Coding agents", "come checked."]);
        let agents = ["> [ ] claude", "  [x] codex", "  [ ] opencode"];
        assert_shows(&form, &agents);
        press(&mut form, &[Key::Down, Key::Space, Key::Enter]);
        assert_shows(&form, &["Step 5 of 7", NO_AGENT, "> [ ] codex"]);
        press(&mut form, &[Key::Down, Key::Space, Key::Up, Key::Up]);
        form.press(Key::Space);
        press(&mut form, &[Key::Enter; 3]);
        assert_shows(&form, &["Agents: claude, opencode"]);
    }

    #[test]
    fn the_language_is_en_unless_another_is_typed() {
        let mut form = up_to_stacks(&["claude", "codex"]);
        press(&mut form, &[Key::Space, Key::Enter, Key::Enter]);
        assert_shows(&form, &["Step 6 of 7 · Language", "(en, es...)"]);
        assert_shows(&form, &["\n│> en█", TEXT_KEYS]);
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["Language: en"]);
        press(&mut form, &[Key::Esc, Key::Esc, Key::Backspace]);
        press(&mut form, &[Key::Backspace, Key::Space]);
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["Language: en"]);
        press(&mut form, &[Key::Esc, Key::Esc, Key::Backspace]);
        type_text(&mut form, "es ");
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["Language: es"]);
        form.press(Key::Enter);
        let language = "es ".to_string();
        let set_up = init::Options {
            language,
            ..notes(&["claude", "codex"], false)
        };
        assert_eq!(form.outcome(), Some(&Outcome::SetUp(set_up)));
    }

    #[test]
    fn the_sensitive_areas_come_one_per_line_and_the_last_can_be_taken_back() {
        let mut form = up_to_stacks(&["claude", "codex"]);
        form.press(Key::Space);
        press(&mut form, &[Key::Enter; 3]);
        assert_shows(&form, &["Step 7 of 7 · Sensitive areas"]);
        assert_shows(&form, &["One area per line.", "Backspace: edit the last"]);
        type_text(&mut form, "Payments");
        form.press(Key::Enter);
        type_text(&mut form, "User  data");
        form.press(Key::Enter);
        assert_shows(&form, &["  - Payments", "  - User  data", "\n│> █"]);
        type_text(&mut form, "x");
        press(&mut form, &[Key::Backspace, Key::Backspace]);
        assert_shows(&form, &["  - Payments", "> User  data█"]);
        assert_hides(&form, &["  - User  data"]);
        press(&mut form, &[Key::Backspace, Key::Enter]);
        press(&mut form, &[Key::Space, Key::Enter]);
        assert_shows(&form, &["Sensitive: Payments; User  dat"]);
        assert_hides(&form, &["Sensitive: Payments; User  data", "User  dat;"]);
    }

    #[test]
    fn the_answers_go_to_gdd_init_as_typed() {
        let mut form = Form::new(names(&["claude", "codex"]));
        type_text(&mut form, " My  Notes ");
        form.press(Key::Enter);
        type_text(&mut form, "A  notes app ");
        press(&mut form, &[Key::Enter, Key::Enter, Key::Space]);
        press(&mut form, &[Key::Enter, Key::Enter, Key::Enter]);
        type_text(&mut form, " User  data");
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["Name:  My  Notes ", "Description: A  notes app "]);
        assert_shows(&form, &["Sensitive:  User  data"]);
        form.press(Key::Enter);
        let typed = init::Options {
            name: " My  Notes ".to_string(),
            platform: "mobile".to_string(),
            description: "A  notes app ".to_string(),
            sensitive: names(&[" User  data"]),
            ..notes(&["claude", "codex"], false)
        };
        assert_eq!(form.outcome(), Some(&Outcome::SetUp(typed)));
    }

    #[test]
    fn the_flags_given_are_the_first_answers() {
        let mut given = init::Options {
            name: String::new(),
            platform: String::new(),
            stacks: Vec::new(),
            agents: Vec::new(),
            language: "es".to_string(),
            description: "A  notes app".to_string(),
            sensitive: names(&["Leaks"]),
            accept_same_family: false,
        };
        let form = Form::new(names(&["codex"])).answered(&given);
        assert_eq!(form.agents.picked(), names(&["codex"]));
        given.agents = names(&["codex", "claude"]);
        let mut form = Form::new(names(&["opencode"])).answered(&given);
        type_text(&mut form, "Notes");
        form.press(Key::Enter);
        assert_shows(&form, &["> A  notes app█"]);
        press(&mut form, &[Key::Enter, Key::Enter, Key::Space, Key::Enter]);
        assert_shows(&form, &["> [x] claude", "  [x] codex", "  [ ] opencode"]);
        press(&mut form, &[Key::Enter, Key::Enter]);
        assert_shows(&form, &["  - Leaks", "\n│> █"]);
        press(&mut form, &[Key::Enter, Key::Enter]);
        let set_up = init::Options {
            platform: "mobile".to_string(),
            language: "es".to_string(),
            description: "A  notes app".to_string(),
            sensitive: names(&["Leaks"]),
            ..notes(&["claude", "codex"], false)
        };
        assert_eq!(form.outcome(), Some(&Outcome::SetUp(set_up)));
    }

    #[test]
    fn the_summary_shows_every_answer_and_enter_sets_the_project_up() {
        let mut form = on_the_summary(&["claude", "codex"]);
        let answers = [
            "Every answer, before anything is written.",
            "Name: Notes",
            "Description: Personal notes",
            "Platform: cli",
            "Stacks: rust",
            "Agents: claude, codex",
            "Language: en",
            "Sensitive: none",
            "Enter to set up · Esc to go back",
        ];
        assert_shows(&form, &answers);
        assert_hides(&form, &[SAME_FAMILY, "gdd init --name"]);
        press(&mut form, &[Key::Char('y'), Key::Char('n')]);
        assert_eq!(form.outcome(), None);
        form.press(Key::Enter);
        let set_up = Outcome::SetUp(notes(&["claude", "codex"], false));
        assert_eq!(form.outcome(), Some(&set_up));
    }

    #[test]
    fn an_empty_description_is_none_on_the_summary() {
        let mut form = Form::new(names(&["claude", "codex"]));
        type_text(&mut form, "Notes");
        press(&mut form, &[Key::Enter, Key::Enter]);
        press(&mut form, &[Key::Enter, Key::Space]);
        press(&mut form, &[Key::Enter; 4]);
        assert_shows(&form, &["Description: none", "Platform: mobile"]);
    }

    #[test]
    fn one_model_family_waits_for_y() {
        let mut form = on_the_summary(&["claude"]);
        assert_shows(&form, &[SAME_FAMILY, "\n│Esc to go back"]);
        assert_hides(&form, &["Enter to set up"]);
        press(&mut form, &[Key::Enter, Key::Char('n'), Key::Space]);
        assert_eq!(form.outcome(), None);
        assert_shows(&form, &["same model family"]);
        form.press(Key::Char('y'));
        let set_up = Outcome::SetUp(notes(&["claude"], true));
        assert_eq!(form.outcome(), Some(&set_up));
    }

    #[test]
    fn not_sure_of_the_technology_ends_with_the_flags_command() {
        let mut form = up_to_stacks(&["claude"]);
        let keys = [Key::Space, Key::Down, Key::Down, Key::Down, Key::Space];
        press(&mut form, &keys);
        press(&mut form, &[Key::Enter; 4]);
        let command = "gdd init --name Notes --platform cli --agents claude --language en \
                       --description 'Personal notes' --stack <pick one>";
        let wide = form.render(140, 40);
        assert!(wide.contains(&format!("│{command} ")), "{wide}");
        assert_shows(&form, &[ASK_AGENT, "Enter to finish · Esc to go back"]);
        let intro = "Not sure of the technology yet: once you know it, run";
        assert_shows(&form, &[intro]);
        assert_hides(&form, &["Enter to set up", SAME_FAMILY, "Stacks:"]);
        form.press(Key::Char('y'));
        assert_eq!(form.outcome(), None);
        form.press(Key::Enter);
        let not_sure = Outcome::NotSure(command.to_string());
        assert_eq!(form.outcome(), Some(&not_sure));
    }

    /// The words the shell reads in `command`: split at spaces, but not between single quotes,
    /// and the character after `\` as it is.
    fn shell_words(command: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word: Option<String> = None;
        let mut quoted = false;
        let mut chars = command.chars();
        while let Some(c) = chars.next() {
            match (quoted, c) {
                (_, '\'') => {
                    quoted = !quoted;
                    word.get_or_insert_default();
                }
                (true, c) => word.get_or_insert_default().push(c),
                (false, '\\') => word.get_or_insert_default().extend(chars.next()),
                (false, ' ') => words.extend(word.take()),
                (false, c) => word.get_or_insert_default().push(c),
            }
        }
        words.extend(word);
        words
    }

    #[test]
    fn the_flags_command_gives_gdd_init_each_answer_as_typed() {
        let mut form = Form::new(names(&["claude", "codex"]));
        type_text(&mut form, "2nd  Brain!");
        form.press(Key::Enter);
        type_text(&mut form, "-x --help");
        press(&mut form, &[Key::Enter, Key::Enter]);
        press(&mut form, &[Key::Down, Key::Down, Key::Down, Key::Space]);
        press(&mut form, &[Key::Enter, Key::Enter, Key::Enter]);
        type_text(&mut form, "R&D");
        form.press(Key::Enter);
        type_text(&mut form, "-  it's");
        press(&mut form, &[Key::Enter, Key::Enter, Key::Enter]);
        let command = "gdd init --name '2nd  Brain!' --platform mobile --agents claude codex \
                       --language en --description='-x --help' --sensitive 'R&D' \
                       --sensitive='-  it'\\''s' --stack <pick one>";
        let not_sure = Outcome::NotSure(command.to_string());
        assert_eq!(form.outcome(), Some(&not_sure));
        let words = shell_words(&command.replace("<pick one>", "rust"));
        let args = match crate::parse(&words[1..]).map(|cli| cli.command) {
            Ok(Some(crate::Run::Init(args))) => args,
            other => panic!("{command}: {other:?}"),
        };
        let typed = init::Options {
            name: "2nd  Brain!".to_string(),
            platform: "mobile".to_string(),
            stacks: names(&["rust"]),
            agents: names(&["claude", "codex"]),
            language: "en".to_string(),
            description: "-x --help".to_string(),
            sensitive: names(&["R&D", "-  it's"]),
            accept_same_family: false,
        };
        assert_eq!(args.options(), typed);
        let plain = "a-Z_0./,:@+=%ñ";
        assert_eq!(shell_word(plain), plain);
    }

    #[test]
    fn ctrl_c_quits_and_no_key_counts_after_the_end() {
        let mut form = up_to_stacks(&["claude", "codex"]);
        form.press(Key::CtrlC);
        assert_eq!(form.outcome(), Some(&Outcome::Quit));
        press(&mut form, &[Key::Space, Key::Enter, Key::Esc]);
        assert_eq!(form.outcome(), Some(&Outcome::Quit));
        assert_shows(&form, &["Step 4 of 7", "> [ ] rust"]);
        let mut form = on_the_summary(&["claude", "codex"]);
        press(&mut form, &[Key::Enter, Key::Esc, Key::CtrlC]);
        let set_up = Outcome::SetUp(notes(&["claude", "codex"], false));
        assert_eq!(form.outcome(), Some(&set_up));
    }

    #[test]
    fn the_screen_is_its_cells_a_line_per_row() {
        let mut form = Form::new(Vec::new());
        type_text(&mut form, "日本");
        assert_shows(&form, &["> 日本█"]);
        let small = form.render(20, 3);
        let rows: Vec<usize> = small.lines().map(|row| row.chars().count()).collect();
        assert_eq!(rows, [20, 20, 20]);
        assert!(small.starts_with("┌ gdd init · Step 1"), "{small}");
        assert_eq!(form.render(0, 0), "");
    }

    #[test]
    fn the_stacks_of_the_form_are_those_of_the_catalog() {
        let mut sorted = STACKS;
        sorted.sort_unstable();
        assert_eq!(sorted.join(", "), gdd_core::stacks::NAMES);
    }

    #[test]
    fn finish_does_what_the_human_answered() {
        let said = finish(Path::new("/"), &Outcome::NotSure("gdd init x".into()), &[]);
        let ask = "gdd init x\nYour coding agent can help you choose it.".to_string();
        assert_eq!(said, Ok(ask));
        let quit = Err("gdd init stopped: nothing was written".to_string());
        assert_eq!(finish(Path::new("/"), &Outcome::Quit, &[]), quit);
        let folder = plain_folder();
        let one = Outcome::SetUp(notes(&["claude"], false));
        let refused = finish(&folder.0, &one, &init_env()).unwrap_err();
        let one_family = "error: one model family only";
        assert!(refused.starts_with(one_family), "{refused}");
        assert!(!folder.0.join("gdd").exists());
        let two = Outcome::SetUp(notes(&["claude", "codex"], false));
        let (_bin, env) = with_cargo("echo locked > Cargo.lock");
        let said = finish(&folder.0, &two, &env).unwrap();
        assert!(said.starts_with("gdd/project.toml\n"), "{said}");
        let path = folder.0.join("gdd/project.toml");
        let project = std::fs::read_to_string(path).unwrap();
        assert!(project.contains("name = \"Notes\"\n"), "{project}");
    }

    /// A folder with a `cargo` that runs `script`, and the environment of [`init_env`] with it
    /// first on the `PATH`; the folder goes when dropped.
    fn with_cargo(script: &str) -> (Project, Vec<(String, String)>) {
        let bin = Project::new();
        bin.write("bin/cargo", &format!("#!/bin/sh\n{script}\n"));
        let program = bin.0.join("bin/cargo");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut env = init_env();
        for (name, value) in &mut env {
            if name == "PATH" {
                *value = format!("{}:{value}", bin.0.join("bin").display());
            }
        }
        (bin, env)
    }

    fn last_subject(folder: &Path) -> String {
        let mut log = Command::new("git");
        let out = log.args(["log", "-1", "--format=%s"]).current_dir(folder);
        let out = out.output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn setting_up_installs_the_stacks_and_ends_by_sending_you_to_your_agent() {
        let folder = plain_folder();
        let (_bin, env) = with_cargo("echo locked > Cargo.lock");
        let set_up = Outcome::SetUp(notes(&["claude", "codex"], false));
        let said = finish(&folder.0, &set_up, &env).unwrap();
        assert!(said.starts_with("gdd/project.toml\n"), "{said}");
        assert!(said.contains("\nrust: cargo generate-lockfile\n"), "{said}");
        assert!(said.contains("\nCommitted: Cargo.lock\n"), "{said}");
        assert_eq!(said.lines().last(), Some(NEXT_AGENT));
        assert_eq!(said.matches("Next:").count(), 1, "{said}");
        assert!(!said.contains("run gdd bootstrap"), "{said}");
        assert_eq!(last_subject(&folder.0), "gdd bootstrap");
    }

    #[test]
    fn a_failed_install_keeps_the_setup_and_says_how_to_retry() {
        let folder = plain_folder();
        let (_bin, env) = with_cargo("exit 3");
        let set_up = Outcome::SetUp(notes(&["claude", "codex"], false));
        let said = finish(&folder.0, &set_up, &env).unwrap_err();
        assert!(said.starts_with("gdd/project.toml\n"), "{said}");
        let failed = "rust: cargo generate-lockfile failed (exit 3)";
        assert!(said.contains(&format!("\nerror: {failed}\n")), "{said}");
        assert!(said.ends_with(INSTALL_FAILED), "{said}");
        assert!(!said.contains("Next:"), "{said}");
        assert!(folder.0.join("gdd/project.toml").is_file());
        assert_eq!(
            last_subject(&folder.0),
            format!("gdd init {}", gdd_core::VERSION)
        );
    }

    #[test]
    fn the_form_is_a_screen_that_is_done_with_its_outcome() {
        let mut form = Form::new(Vec::new());
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let keys = [None, Some(Key::Char('N')), None, Some(Key::CtrlC)];
        let mut keys = keys.into_iter();
        // Reading fails once the keys run out, so a form that never ends fails the test.
        let no_more = || io::Error::other("no more keys");
        let ended = tui::run(&mut form, &mut terminal, || keys.next().ok_or_else(no_more));
        assert_eq!(ended, Ok(()));
        assert_eq!(form.outcome(), Some(&Outcome::Quit));
        let shown = tui::screen_text(terminal.backend().buffer());
        assert!(shown.contains("│> N█"), "{shown}");
    }
}
