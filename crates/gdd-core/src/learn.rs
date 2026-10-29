//! Learn (`gdd learn`): the phase has no gate, it only hardens. The Learn tools of
//! `gdd/commands.toml` (`[learn]`: duplication, unused code) run once on the project; what each
//! one said stays in `.gdd/learn/` and a few lines of it are shown. Next to them, the polish notes
//! the human gave at G4, so the Orchestrator can spot the ones that repeat. A tool that finds
//! something is a warning, never a failure.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use toml::Spanned;

use crate::agents::files;
use crate::config;
use crate::exec;
use crate::flow::{self, DAY};
use crate::log;

/// Where what each tool said is kept, from the state root.
pub const FOLDER: &str = ".gdd/learn";

/// What `gdd learn` says when the project has no Learn tools.
pub const NO_TOOLS: &str = "No Learn tools in gdd/commands.toml: add a [learn] section (gdd stacks \
                            lists the stack sheets, whose [learn] has them).";

/// Lines of what a tool said that are shown; the file keeps all of it.
const SHOWN: usize = 10;

/// The exit codes of a shell that could not run the command: found but not executable, not found.
const NOT_EXECUTABLE: i32 = 126;
const NOT_FOUND: i32 = 127;

/// The variables the tools get: the ones of the gates, without a change (as `gdd adopt`).
const ENV: [(&str, &str); 4] = [
    ("GDD_CHANGE", ""),
    ("GDD_COLOR", ""),
    ("GDD_DIFF_FILES", ""),
    ("GDD_FEATURES", ""),
];

/// How a Learn tool ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Exit code 0.
    Nothing,
    /// It could not run: the shell did not find it, could not execute it or could not start.
    CouldNotRun(i32),
    /// It ran out of time.
    TimedOut,
    /// Any other exit code.
    Found(i32),
}

impl Verdict {
    fn of(code: i32) -> Verdict {
        match code {
            0 => Verdict::Nothing,
            NOT_EXECUTABLE | NOT_FOUND => Verdict::CouldNotRun(code),
            exec::TIMED_OUT => Verdict::TimedOut,
            _ => Verdict::Found(code),
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Nothing => f.write_str("nothing found"),
            Verdict::CouldNotRun(code) => write!(f, "could not run (exit code {code})"),
            Verdict::TimedOut => f.write_str("timed out"),
            Verdict::Found(code) => write!(f, "found something (exit code {code})"),
        }
    }
}

/// What one Learn tool did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// Its name in `[learn]`.
    pub name: String,
    /// How it ended.
    pub verdict: Verdict,
    /// The first lines of what it said, without colors.
    pub shown: Vec<String>,
}

impl fmt::Display for Tool {
    /// The line of the tool; when it said something, its first lines indented and where all of
    /// it is kept.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.verdict)?;
        if self.shown.is_empty() {
            return Ok(());
        }
        for line in &self.shown {
            write!(f, "\n  {line}")?;
        }
        write!(f, "\n  All it said: {}", kept(&self.name))
    }
}

/// A polish note the human gave at G4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The change it was given on.
    pub change: String,
    /// What the human asked to polish.
    pub text: String,
}

/// What `gdd learn` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Learned {
    /// The Learn tools, in the order of the file; none when the project has none.
    pub tools: Vec<Tool>,
    /// The polish notes at G4, in the order of the log.
    pub notes: Vec<Note>,
}

impl fmt::Display for Learned {
    /// What `gdd learn` prints, without a line break at the end: the tools, or how to add them,
    /// then the polish notes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tools.is_empty() {
            writeln!(f, "{NO_TOOLS}")?;
        }
        for tool in &self.tools {
            writeln!(f, "{tool}")?;
        }
        write!(f, "\nPolish notes at G4 ({})", self.notes.len())?;
        for note in &self.notes {
            write!(f, "\n- {}: {}", note.change, note.text)?;
        }
        Ok(())
    }
}

/// Where what the tool `name` said is kept, from the state root.
fn kept(name: &str) -> String {
    format!("{FOLDER}/{name}.txt")
}

#[derive(Deserialize)]
struct RawCommands {
    #[serde(default)]
    learn: BTreeMap<String, Spanned<toml::Value>>,
}

/// The Learn tools of a commands file, in the order of the file (a table sorts its entries by
/// name: the place of each value in the text puts them back): text that is not blank. A missing
/// section has none; a value that is not text is not a tool.
fn tools_of(text: &str) -> Result<Vec<(String, String)>, config::Error> {
    let raw: RawCommands = toml::from_str(text).map_err(|e| config::Error::Toml(e.to_string()))?;
    let mut entries: Vec<_> = raw.learn.into_iter().collect();
    entries.sort_by_key(|(_, value)| value.span().start);
    Ok(entries
        .into_iter()
        .filter_map(|(name, value)| match value.into_inner() {
            toml::Value::String(command) if !command.trim().is_empty() => Some((name, command)),
            _ => None,
        })
        .collect())
}

/// Whether `name` can name a file of its own: letters, digits, `-` and `_`.
fn names_a_file(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    !name.is_empty() && name.chars().all(plain)
}

/// Runs the Learn tools of the commands file `commands`, from the project root `root`, keeps
/// what each said under the state root `state`, and lists the polish notes at G4 of the log under
/// `state`, since the day `since` (see [`flow::day`]) when given. A tool that finds something, or
/// cannot run, is part of the result, not an error.
pub fn learn(
    root: &Path,
    state: &Path,
    commands: &str,
    since: Option<i64>,
) -> Result<Learned, String> {
    learn_within(root, state, commands, since, exec::TIMEOUT)
}

/// [`learn`], with `timeout` for each tool.
fn learn_within(
    root: &Path,
    state: &Path,
    commands: &str,
    since: Option<i64>,
    timeout: Duration,
) -> Result<Learned, String> {
    let found = tools_of(commands).map_err(|e| format!("{}: {e}", crate::gates::COMMANDS))?;
    if let Some((name, _)) = found.iter().find(|(name, _)| !names_a_file(name)) {
        return Err(format!(
            "{}: [learn] {name:?}: gdd learn keeps what a tool says in a file named after it, so \
             its name takes only letters, digits, - and _",
            crate::gates::COMMANDS
        ));
    }
    let notes = polish_notes(state, since)?;
    let mut tools = Vec::new();
    for (name, command) in found {
        tools.push(run_tool(root, state, name, &command, timeout)?);
    }
    Ok(Learned { tools, notes })
}

/// Runs one tool and keeps all it said.
fn run_tool(
    root: &Path,
    state: &Path,
    name: String,
    command: &str,
    timeout: Duration,
) -> Result<Tool, String> {
    let (code, output) = match exec::run(command, root, &ENV, timeout) {
        Ok(ran) => (ran.code, ran.output),
        Err(e) => (NOT_FOUND, format!("cannot run sh: {e}")),
    };
    let path = kept(&name);
    files::replace(state, &path, output.as_bytes())
        .map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(Tool {
        verdict: Verdict::of(code),
        shown: first_lines(&output),
        name,
    })
}

/// The first lines of `output` without colors; none when it says nothing.
fn first_lines(output: &str) -> Vec<String> {
    let plain = without_colors(output);
    if plain.trim().is_empty() {
        return Vec::new();
    }
    plain.lines().take(SHOWN).map(str::to_string).collect()
}

/// `text` without the ANSI escape sequences that set colors and move the cursor.
fn without_colors(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            plain.push(c);
        } else if chars.next_if_eq(&'[').is_some() {
            // A control sequence ends with the first character from `@` to `~`.
            while chars.next().is_some_and(|c| !('@'..='~').contains(&c)) {}
        }
    }
    plain
}

/// The polish notes the human gave at G4 in the log under `state`, in log order: only the ones
/// of the changes whose first line is on or after the day `since`. No log is no notes.
fn polish_notes(state: &Path, since: Option<i64>) -> Result<Vec<Note>, String> {
    let text = match fs::read_to_string(state.join(log::PATH)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", log::PATH)),
    };
    let lines = flow::read(&text).map_err(|e| format!("{}, {e}", log::PATH))?;
    let from = since.map(|day| day * DAY);
    let mut first = BTreeMap::new();
    let mut notes = Vec::new();
    for line in &lines {
        let change = line.record.text("change");
        let start = *first.entry(change.clone()).or_insert(line.at);
        let wanted = from.is_none_or(|from| start >= from);
        let record = &line.record;
        let polish = record.text("gate") == "G4" && record.text("result") == "polish";
        if wanted && polish && record.text("role") == "human" {
            let text = line.record.text("note");
            notes.push(Note { change, text });
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::Folder;
    use std::os::unix::fs::symlink;

    fn line(ts: &str, change: &str, gate: &str, result: &str, note: &str) -> String {
        format!(
            "{{\"ts\": \"{ts}\", \"change\": \"{change}\", \"phase\": \"4-release\", \
             \"gate\": \"{gate}\", \"role\": \"human\", \"result\": \"{result}\", \
             \"note\": \"{note}\"}}\n"
        )
    }

    /// A line of the log on a day and hour of October 2026, `20T11`; its note says the change,
    /// the gate and the result, as the scenarios of the notes expect.
    fn step(day_hour: &str, change: &str, gate: &str, result: &str) -> String {
        let ts = format!("2026-10-{day_hour}:00:00+00:00");
        let note = match (change, gate, result, day_hour) {
            ("a", "G4", "polish", "20T11") => "Shorter error messages",
            ("b", "G4", "polish", _) => "Shorter, again",
            ("a", "G4", "polish", _) => "Later note of a",
            ("a", "G3", "polish", _) => "not at G4",
            _ => result,
        };
        line(&ts, change, gate, result, note)
    }

    /// A project folder with a state root apart.
    fn project() -> (Folder, Folder) {
        (Folder::new(), Folder::new())
    }

    fn commands(tools: &[(&str, &str)]) -> String {
        let mut text = String::from("[g2]\nunit = \"true\"\n\n[learn]\n");
        for (name, command) in tools {
            text.push_str(&format!("{name} = {command:?}\n"));
        }
        text
    }

    /// What the tool `name` said, as kept under the state root `state`.
    fn said_by(state: &Folder, name: &str) -> String {
        fs::read_to_string(state.0.join(kept(name))).unwrap()
    }

    fn run(tools: &[(&str, &str)]) -> Learned {
        let (root, state) = project();
        learn(&root.0, &state.0, &commands(tools), None).unwrap()
    }

    fn verdict_of(command: &str) -> Verdict {
        run(&[("t", command)]).tools[0].verdict
    }

    #[test]
    fn a_tool_that_exits_with_zero_found_nothing() {
        assert_eq!(verdict_of("echo ok"), Verdict::Nothing);
    }

    #[test]
    fn any_other_exit_code_found_something() {
        assert_eq!(verdict_of("exit 3"), Verdict::Found(3));
        assert_eq!(verdict_of("exit 1"), Verdict::Found(1));
        assert_eq!(verdict_of("exit 125"), Verdict::Found(125));
        assert_eq!(verdict_of("exit 128"), Verdict::Found(128));
    }

    #[test]
    fn a_tool_the_shell_cannot_run_could_not_run() {
        assert_eq!(verdict_of("no-such-tool-xyz"), Verdict::CouldNotRun(127));
        assert_eq!(verdict_of("exit 126"), Verdict::CouldNotRun(126));
        let tool = &run(&[("t", "no-such-tool-xyz")]).tools[0];
        let first = tool.to_string().lines().next().map(str::to_string);
        assert_eq!(first.as_deref(), Some("t: could not run (exit code 127)"));
    }

    #[test]
    fn a_tool_that_cannot_start_could_not_run() {
        let (_, state) = project();
        let missing = state.0.join("no-such-folder");
        let tool = run_tool(&missing, &state.0, "t".into(), "true", exec::TIMEOUT).unwrap();
        assert_eq!(tool.verdict, Verdict::CouldNotRun(127));
        assert!(tool.shown[0].starts_with("cannot run sh: "), "{tool}");
    }

    #[test]
    fn a_tool_that_runs_out_of_time_timed_out() {
        let (root, state) = project();
        let text = commands(&[("slow", "sleep 5")]);
        let timeout = Duration::from_millis(200);
        let learned = learn_within(&root.0, &state.0, &text, None, timeout).unwrap();
        assert_eq!(learned.tools[0].verdict, Verdict::TimedOut);
        let said = learned.to_string();
        assert!(said.starts_with("slow: timed out\n  timed out ("), "{said}");
    }

    #[test]
    fn the_tools_run_in_the_order_of_the_file_and_the_others_still_run() {
        let learned = run(&[("zeta", "exit 2"), ("alpha", "nope-xyz"), ("mid", "true")]);
        let names: Vec<&str> = learned.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["zeta", "alpha", "mid"]);
        let said = learned.to_string();
        assert!(said.starts_with("zeta: found something (exit code 2)\nalpha: could not run"));
        assert!(said.contains("\nmid: nothing found\n"), "{said}");
    }

    #[test]
    fn a_blank_command_or_one_that_is_not_text_is_not_a_tool() {
        let (root, state) = project();
        let text = "[learn]\nblank = \"  \"\nnumber = 3\nreal = \"true\"\n";
        let learned = learn(&root.0, &state.0, text, None).unwrap();
        assert_eq!(learned.tools.len(), 1);
        assert_eq!(learned.tools[0].name, "real");
    }

    #[test]
    fn without_a_learn_section_it_says_how_to_add_one() {
        let (root, state) = project();
        for text in ["", "[g2]\nunit = \"true\"\n", "[learn]\n"] {
            let learned = learn(&root.0, &state.0, text, None).unwrap();
            assert!(learned.tools.is_empty(), "{text}");
            let said = learned.to_string();
            assert!(said.starts_with(NO_TOOLS), "{said}");
            assert!(said.ends_with("Polish notes at G4 (0)"), "{said}");
        }
        assert!(!state.0.join(FOLDER).exists());
    }

    #[test]
    fn a_commands_file_that_is_not_toml_is_an_error() {
        let (root, state) = project();
        let err = learn(&root.0, &state.0, "[learn", None).unwrap_err();
        assert!(err.starts_with("gdd/commands.toml: "), "{err}");
        let err = learn(&root.0, &state.0, "learn = 3", None).unwrap_err();
        assert!(err.starts_with("gdd/commands.toml: "), "{err}");
    }

    #[test]
    fn letters_digits_dashes_and_underscores_name_a_file() {
        for name in ["a", "Z9", "a-b", "a_b", "-", "_"] {
            assert!(names_a_file(name), "{name}");
        }
        for name in ["", "a b", "a.b", "a/b", "é"] {
            assert!(!names_a_file(name), "{name}");
        }
    }

    #[test]
    fn a_name_that_cannot_name_a_file_is_refused_before_anything_runs() {
        let (root, state) = project();
        for name in ["\"a/b\"", "\"..\"", "\"\"", "\"x.y\""] {
            let text = format!("[learn]\nfirst = \"touch ran\"\n{name} = \"true\"\n");
            let err = learn(&root.0, &state.0, &text, None).unwrap_err();
            let start = format!("gdd/commands.toml: [learn] {name}: gdd learn keeps");
            assert!(err.starts_with(&start), "{name}: {err}");
        }
        assert!(!root.0.join("ran").exists());
        assert!(!state.0.join(FOLDER).exists());
    }

    #[test]
    fn the_tools_run_from_the_root_with_the_variables_of_the_gates_and_no_change() {
        let (root, state) = project();
        root.write("README.md", "");
        let vars = "$GDD_CHANGE$GDD_COLOR$GDD_DIFF_FILES$GDD_FEATURES";
        let check = format!("test -f README.md && test -z \"{vars}\"");
        let learned = learn(&root.0, &state.0, &commands(&[("here", &check)]), None).unwrap();
        assert_eq!(learned.tools[0].verdict, Verdict::Nothing);
    }

    #[test]
    fn what_a_tool_said_is_shown_and_kept_whole_and_replaced_at_each_run() {
        let (root, state) = project();
        let text = commands(&[("unused", "echo old.py:3 unused; echo more >&2; exit 3")]);
        let learned = learn(&root.0, &state.0, &text, None).unwrap();
        let tool = &learned.tools[0];
        assert_eq!(tool.shown, ["old.py:3 unused", "more"]);
        let shown = "unused: found something (exit code 3)\n  old.py:3 unused\n  more\n  \
                     All it said: .gdd/learn/unused.txt";
        assert_eq!(tool.to_string(), shown);
        assert_eq!(said_by(&state, "unused"), "old.py:3 unused\nmore\n");
        let again = commands(&[("unused", "echo fixed")]);
        learn(&root.0, &state.0, &again, None).unwrap();
        assert_eq!(said_by(&state, "unused"), "fixed\n");
        assert!(!root.0.join(".gdd").exists());
    }

    #[test]
    fn a_tool_that_said_nothing_has_no_indented_lines_but_its_file_is_replaced() {
        let (root, state) = project();
        state.write(".gdd/learn/quiet.txt", "from a past run\n");
        let learned = learn(&root.0, &state.0, &commands(&[("quiet", "true")]), None).unwrap();
        assert_eq!(learned.tools[0].to_string(), "quiet: nothing found");
        assert_eq!(said_by(&state, "quiet"), "");
        let blank = commands(&[("blank", "echo; echo '  '")]);
        let learned = learn(&root.0, &state.0, &blank, None).unwrap();
        assert_eq!(learned.tools[0].to_string(), "blank: nothing found");
    }

    #[test]
    fn only_ten_lines_are_shown_and_all_of_them_are_kept() {
        let (root, state) = project();
        let text = commands(&[("many", "seq 1 11; exit 1")]);
        let learned = learn(&root.0, &state.0, &text, None).unwrap();
        let tool = &learned.tools[0];
        assert_eq!(tool.shown.len(), 10);
        assert_eq!(tool.shown[9], "10");
        assert!(!tool.to_string().contains("\n  11"));
        assert_eq!(said_by(&state, "many").lines().count(), 11);
        let ten = commands(&[("ten", "seq 1 10; exit 1")]);
        let learned = learn(&root.0, &state.0, &ten, None).unwrap();
        assert_eq!(learned.tools[0].shown.len(), 10);
    }

    #[test]
    fn colors_are_removed_from_what_is_shown_but_not_from_the_file() {
        let (root, state) = project();
        let paint = "printf '\\033[31mred\\033[0m \\033[1;32mgreen\\033[0m\\n'";
        let text = commands(&[("c", paint)]);
        let learned = learn(&root.0, &state.0, &text, None).unwrap();
        assert_eq!(learned.tools[0].shown, ["red green"]);
        let kept = said_by(&state, "c");
        assert!(kept.contains('\u{1b}'), "{kept:?}");
        assert_eq!(without_colors("a\u{1b}[2Kb\u{1b}[m"), "ab");
        assert_eq!(without_colors("a\u{1b}b"), "ab");
        assert_eq!(without_colors("a\u{1b}[31"), "a");
        assert_eq!(without_colors("[31m plain"), "[31m plain");
    }

    fn log_text() -> String {
        [
            step("20T09", "a", "-", "change-created"),
            step("20T11", "a", "G4", "polish"),
            step("21T00", "b", "-", "change-created"),
            step("21T11", "b", "G4", "polish"),
            step("21T12", "b", "G4", "ok"),
            step("21T13", "a", "G3", "polish"),
            step("22T09", "a", "G4", "polish"),
        ]
        .concat()
    }

    fn notes(since: Option<i64>) -> Vec<String> {
        let (root, state) = project();
        state.write(".gdd/log.jsonl", &log_text());
        let learned = learn(&root.0, &state.0, "", since).unwrap();
        let notes = learned.notes.iter();
        notes.map(|n| format!("{}: {}", n.change, n.text)).collect()
    }

    #[test]
    fn the_polish_notes_at_g4_come_in_log_order() {
        let all = [
            "a: Shorter error messages",
            "b: Shorter, again",
            "a: Later note of a",
        ];
        assert_eq!(notes(None), all);
        let (root, state) = project();
        state.write(".gdd/log.jsonl", &log_text());
        let said = learn(&root.0, &state.0, "", None).unwrap().to_string();
        let tail = "Polish notes at G4 (3)\n- a: Shorter error messages\n- b: Shorter, again\n- \
                    a: Later note of a";
        assert!(said.ends_with(tail), "{said}");
    }

    #[test]
    fn since_counts_the_changes_created_on_or_after_the_day() {
        // `b` is created at 00:00:00 UTC of 2026-10-21: that very day counts.
        assert_eq!(notes(flow::day("2026-10-21")), ["b: Shorter, again"]);
        assert_eq!(notes(flow::day("2026-10-20")).len(), 3);
        // The day after, `b` is out; `a` stays out too, though its note is from the 22nd.
        assert!(notes(flow::day("2026-10-22")).is_empty());
    }

    #[test]
    fn a_change_that_starts_one_second_before_the_day_is_out() {
        let (root, state) = project();
        let log = line("2026-10-20T23:59:59+00:00", "a", "G4", "polish", "early")
            + &line("2026-10-21T00:00:00+00:00", "b", "G4", "polish", "on time");
        state.write(".gdd/log.jsonl", &log);
        let since = flow::day("2026-10-21");
        let learned = learn(&root.0, &state.0, "", since).unwrap();
        assert_eq!(learned.notes.len(), 1);
        assert_eq!(learned.notes[0].change, "b");
    }

    #[test]
    fn only_the_polish_notes_of_the_human_count() {
        let (root, state) = project();
        let machine = line(
            "2026-10-20T10:00:00+00:00",
            "a",
            "G4",
            "polish",
            "by a machine",
        )
        .replace("\"human\"", "\"machine\"");
        let agent = machine.replace("\"machine\"", "\"builder\"");
        let human = line(
            "2026-10-20T11:00:00+00:00",
            "a",
            "G4",
            "polish",
            "by a person",
        );
        state.write(".gdd/log.jsonl", &(machine + &agent + &human));
        let learned = learn(&root.0, &state.0, "", None).unwrap();
        let texts: Vec<&str> = learned.notes.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, ["by a person"]);
    }

    #[test]
    fn without_a_log_there_are_no_notes() {
        let (root, state) = project();
        let learned = learn(&root.0, &state.0, &commands(&[("t", "true")]), None).unwrap();
        assert!(learned.notes.is_empty());
        assert!(learned.to_string().ends_with("\n\nPolish notes at G4 (0)"));
    }

    #[test]
    fn a_log_line_that_cannot_be_read_is_an_error_naming_it() {
        let (root, state) = project();
        state.write(".gdd/log.jsonl", &(log_text() + "not json\n"));
        let err = learn(&root.0, &state.0, "", None).unwrap_err();
        assert!(err.starts_with(".gdd/log.jsonl, line 8: "), "{err}");
        let bad_time = line("yesterday", "a", "G4", "polish", "x");
        state.write(".gdd/log.jsonl", &bad_time);
        let err = learn(&root.0, &state.0, "", None).unwrap_err();
        assert!(err.starts_with(".gdd/log.jsonl, line 1: "), "{err}");
    }

    #[test]
    fn a_log_that_cannot_be_read_is_an_error_not_an_empty_log() {
        let (root, state) = project();
        // A folder where the log goes.
        fs::create_dir_all(state.0.join(".gdd/log.jsonl")).unwrap();
        let err = learn(&root.0, &state.0, "", None).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
        // A file where the folder of the log goes: not a folder, not a missing file.
        let (root, state) = project();
        state.write(".gdd", "");
        let err = learn(&root.0, &state.0, "", None).unwrap_err();
        assert!(err.starts_with("cannot read .gdd/log.jsonl: "), "{err}");
        assert!(err.contains("Not a directory"), "{err}");
    }

    #[test]
    fn a_file_that_cannot_be_written_is_an_error_naming_it() {
        let (root, state) = project();
        let long = "x".repeat(300);
        let text = commands(&[(long.as_str(), "true")]);
        let err = learn(&root.0, &state.0, &text, None).unwrap_err();
        let start = format!("cannot write .gdd/learn/{long}.txt: ");
        assert!(err.starts_with(&start), "{err}");
        assert!(err.contains("File name too long"), "{err}");
    }

    #[test]
    fn what_a_tool_said_is_never_kept_through_a_link() {
        let (root, state) = project();
        let outside = Folder::new();
        outside.write("t.txt", "outside");
        fs::create_dir_all(state.0.join(".gdd")).unwrap();
        symlink(&outside.0, state.0.join(FOLDER)).unwrap();
        let text = commands(&[("t", "echo x")]);
        learn(&root.0, &state.0, &text, None).unwrap();
        let untouched = fs::read_to_string(outside.0.join("t.txt")).unwrap();
        assert_eq!(untouched, "outside");
        let kept = fs::read_to_string(state.0.join(".gdd/learn/t.txt")).unwrap();
        assert_eq!(kept, "x\n");
        let link = state.0.join(".gdd/learn/t.txt");
        assert!(!fs::symlink_metadata(link).unwrap().is_symlink());
    }

    #[test]
    fn a_file_in_the_place_of_the_folder_of_the_tools_is_replaced() {
        let (root, state) = project();
        state.write(".gdd/learn", "in the way");
        let text = commands(&[("t", "echo x")]);
        learn(&root.0, &state.0, &text, None).unwrap();
        let kept = fs::read_to_string(state.0.join(".gdd/learn/t.txt")).unwrap();
        assert_eq!(kept, "x\n");
    }
}
