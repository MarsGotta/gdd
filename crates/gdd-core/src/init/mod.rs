//! `gdd init`: sets up GDD in the folder it runs in, which becomes the project root, from what
//! the command line says. It checks everything before it writes anything, in this order: no GDD
//! project in the folder or above it, the stacks known, the agents and who plays each role (see
//! [`roles`]), the skeletons of the stacks (see [`skeletons`]), none of the files it writes
//! already there, for it never overwrites one, no symbolic link on the way to what it writes and
//! no other name (a hard link) for the file it appends to, for it never writes out of the folder.
//! Then it writes the harness (see [`files`]), AGENTS.md and CLAUDE.md when the project has none,
//! the skeleton of each stack the folder has no project of, the lines of every stack and `.gdd/`
//! in `.gitignore` and the agents' commands as `gdd agents install` does, and commits what it
//! wrote, and only that, in a new repository when the folder is in none. It stays offline:
//! [`bootstrap()`] runs the first install of each stack.
//!
//! Git runs in the environment gdd runs in, which says who commits (git's own rules).

mod bootstrap;
mod files;
mod roles;
mod skeletons;

use std::fmt;
use std::fs::{self, OpenOptions, Permissions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::agents::models;
use crate::agents::render;
use crate::agents::sheets::Sheet;
use crate::gates::{self, approve};
use crate::git::{self, Git};
use crate::{VERSION, guard, log, stacks};

pub use bootstrap::{Stopped, bootstrap};

/// The platforms a project may run on.
pub const PLATFORMS: [&str; 5] = ["mobile", "desktop", "web", "cli", "other"];

/// The file that marks the root of a GDD project.
const PROJECT: &str = "gdd/project.toml";
/// The zone map.
const ZONES: &str = "gdd/zones.toml";
/// Which gdd wrote the files written once.
const GENERATED: &str = "gdd/generated.toml";
/// The entry point of the agents and CI.
const BIN: &str = "bin/gdd";
/// The CI workflow.
const WORKFLOW: &str = ".github/workflows/gdd.yml";
/// What git leaves out.
const GITIGNORE: &str = ".gitignore";
/// The line of `.gitignore` that leaves out the local state of the tool.
const LOCAL_STATE: &str = ".gdd/";
/// The first branch of the repository gdd init makes.
const MAIN: &str = "main";

/// The files gdd init writes once: what a later `gdd upgrade` compares with what a newer gdd
/// would write.
const ONCE: [&str; 7] = [
    PROJECT,
    gates::COMMANDS,
    guard::RULES,
    ZONES,
    models::PATH,
    approve::BRIEF,
    WORKFLOW,
];

/// Why gdd init stops when one of its files is already there.
const NEVER_OVERWRITES: &str =
    "gdd init never overwrites a file: move or remove these, then run it again";

/// What to do once the project is set up.
const NEXT: &str = "Next: run bin/gdd bootstrap (first install of each stack and its lock file), \
                    bin/gdd agents check, then start your agent and run the gdd-setup command \
                    (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).";

/// An environment: its variables, with their values.
type Env = [(String, String)];

/// What the command line says of the new project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Its name.
    pub name: String,
    /// Where it runs: one of [`PLATFORMS`].
    pub platform: String,
    /// The names of its stacks.
    pub stacks: Vec<String>,
    /// The names of its coding agents, the first builds; none: those on the `PATH`.
    pub agents: Vec<String>,
    /// The language the agents use with the human.
    pub language: String,
    /// What it is, in words; empty when the command line does not say.
    pub description: String,
    /// What would be serious if it failed, one area each.
    pub sensitive: Vec<String>,
    /// The human accepts a Reviewer of the Builder's model family when every agent is of one.
    pub accept_same_family: bool,
}

/// Why gdd init wrote nothing, or stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Files gdd init writes are already there: their paths, from the folder.
    Exists(Vec<String>),
    /// Anything else: why.
    Failed(String),
}

impl fmt::Display for Error {
    /// As gdd init says it, without a line break at the end: a line per file already there,
    /// then `error: ` and why.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Exists(paths) => {
                for path in paths {
                    writeln!(f, "{path} already exists")?;
                }
                write!(f, "error: {NEVER_OVERWRITES}")
            }
            Error::Failed(why) => write!(f, "error: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(why: String) -> Error {
        Error::Failed(why)
    }
}

/// Sets up GDD in the folder `root` for the project of `options` (see the module), with git and
/// the agents on the `PATH` of `env`, the environment gdd runs in. What it says: the files it
/// wrote, one per line, what to add to the instructions the project already had, and the next
/// step; without a line break at the end.
pub fn init(root: &Path, options: &Options, env: &Env) -> Result<String, Error> {
    if let Some(project) = project_above(root) {
        let why = format!("already a GDD project: {}", project.display());
        return Err(Error::Failed(why));
    }
    let now = log::now();
    let (today, _) = now.split_once('T').unwrap_or((now.as_str(), ""));
    let plan = Plan::new(root, options, env, today)?;
    plan.carry_out(root)
}

/// The `gdd/project.toml` of the GDD project the folder `folder` is in: in it or in a folder
/// above it.
fn project_above(folder: &Path) -> Option<PathBuf> {
    folder
        .ancestors()
        .map(|above| above.join(PROJECT))
        .find(|project| project.is_file())
}

/// What gdd init does, once everything is checked.
struct Plan {
    /// The files it writes, from the root, with their text, in order.
    files: Vec<(String, String)>,
    /// What it appends to `.gitignore`, when it does.
    ignore: Option<String>,
    /// The agents that get the commands.
    agents: Vec<Sheet>,
    /// The stacks whose instructions they get.
    stacks: Vec<String>,
    /// Git in the folder, in the environment gdd runs in.
    git: Git,
    /// Whether the folder is in a git repository already.
    repository: bool,
    /// What to add to the instructions the project already has.
    notes: Vec<String>,
}

impl Plan {
    /// What gdd init does in the folder `root` for the project of `options`, in the environment
    /// `env`, on the day `today`, or why it must not: the checks, in order, and nothing written.
    fn new(root: &Path, options: &Options, env: &Env, today: &str) -> Result<Plan, Error> {
        let stacks = stacks::once(&options.stacks);
        let sheets = stack_sheets(&stacks)?;
        let accept = options.accept_same_family;
        let cast = roles::Cast::new(&options.agents, env, accept, today)?;
        let agents = cast.sheets();
        let commands = render::paths(&agents, &stacks)?;
        let package = skeletons::package(&options.name);
        let skeleton = skeletons::files(root, &stacks, &sheets, &package)?;
        check_targets(root, &skeleton, &commands)?;
        let mut notes = Vec::new();
        let mut instructions = Vec::new();
        for item in files::INSTRUCTIONS {
            if exists(root, item.file)? {
                notes.push((item.yours)());
            } else {
                instructions.push((item.file.to_string(), item.template.to_string()));
            }
        }
        let lines = sheets.iter().flat_map(stacks::Sheet::ignore);
        let lines: Vec<&str> = lines.map(String::as_str).collect();
        let ignore = ignore(root, &lines)?;
        let git = Git::in_env(root, env);
        let repository = git::is_repository(root);
        let main = if repository {
            git.checked_out().map_err(|e| e.to_string())?
        } else {
            MAIN.to_string()
        };
        let values = files::Values::new(options, &sheets, &main);
        let harness = [
            (PROJECT, files::project(options, &stacks, &sheets)),
            (gates::COMMANDS, files::commands(&stacks)?),
            (guard::RULES, values.fill(files::RULES)),
            (ZONES, values.fill(files::ZONES)),
            (models::PATH, cast.models(VERSION)),
            (GENERATED, files::generated(&ONCE)),
            (approve::BRIEF, values.fill(files::BRIEF)),
            (BIN, values.fill(files::BIN)),
            (WORKFLOW, values.fill(files::WORKFLOW)),
        ];
        let harness = harness.map(|(path, text)| (path.to_string(), text));
        let mut texts = Vec::from(harness);
        texts.extend(instructions);
        texts.extend(skeleton);
        let appended = ignore.as_ref().map(|_| GITIGNORE);
        let paths = texts.iter().map(|(path, _)| path.as_str()).chain(appended);
        check_links(root, paths.chain(commands.iter().map(String::as_str)))?;
        if let Some(path) = appended {
            check_names(root, path)?;
        }
        Ok(Plan {
            files: texts,
            ignore,
            agents,
            stacks,
            git,
            repository,
            notes,
        })
    }

    /// Writes the files in the folder `root` and commits them, in a new repository when the
    /// folder is in none: what gdd init says (see [`init`]).
    fn carry_out(self, root: &Path) -> Result<String, Error> {
        if !self.repository {
            self.git.init(MAIN).map_err(|e| e.to_string())?;
        }
        let mut written = Vec::new();
        for (path, text) in &self.files {
            create(root, path, text)?;
            written.push(path.to_string());
        }
        let runnable = Permissions::from_mode(0o755);
        let made = fs::set_permissions(root.join(BIN), runnable);
        made.map_err(|e| cannot_write(BIN, &e))?;
        if let Some(line) = &self.ignore {
            append(root, GITIGNORE, line)?;
            written.push(GITIGNORE.to_string());
        }
        written.extend(render::install(root, &self.agents, &self.stacks)?);
        let message = format!("gdd init {VERSION}");
        let committed = self.git.commit_files(&written, &message);
        committed.map_err(|e| e.to_string())?;
        let mut said = written;
        said.extend(self.notes);
        said.push(NEXT.to_string());
        Ok(said.join("\n"))
    }
}

/// The sheets of the `stacks`; a stack without one is an error.
fn stack_sheets(stacks: &[String]) -> Result<Vec<stacks::Sheet>, String> {
    let sheet = |name: &String| stacks::sheet(name).map_err(|e| e.to_string());
    stacks.iter().map(sheet).collect()
}

/// Checks that none of the files gdd init writes in the folder `root` is there: the harness, the
/// files of the `skeleton` and the agents' `commands`.
fn check_targets(
    root: &Path,
    skeleton: &[(String, String)],
    commands: &[String],
) -> Result<(), Error> {
    let harness = [
        PROJECT,
        gates::COMMANDS,
        guard::RULES,
        ZONES,
        models::PATH,
        GENERATED,
        approve::BRIEF,
        BIN,
        WORKFLOW,
    ];
    let skeleton = skeleton.iter().map(|(path, _)| path.as_str());
    let commands = commands.iter().map(String::as_str);
    let mut there = Vec::new();
    for path in harness.into_iter().chain(skeleton).chain(commands) {
        if exists(root, path)? {
            there.push(path.to_string());
        }
    }
    if there.is_empty() {
        Ok(())
    } else {
        Err(Error::Exists(there))
    }
}

/// Checks that no symbolic link is on the way from the folder `root` to any of the `paths` gdd
/// init writes or appends to, the paths themselves included: through a link, it would write out
/// of the folder.
fn check_links<'a>(root: &Path, paths: impl Iterator<Item = &'a str>) -> Result<(), String> {
    for path in paths {
        let link = render::first_link(root, path);
        if let Some(link) = link.map_err(|e| format!("cannot read {path}: {e}"))? {
            let link = link.display();
            return Err(format!(
                "cannot write {path}: {link} is a symbolic link, which gdd init does not write \
                 through: remove the link and run it again"
            ));
        }
    }
    Ok(())
}

/// Checks that the file `path` of the folder `root`, which gdd init appends to, has no other
/// name: through a hard link, it would write to a file that may be out of the folder too. A file
/// still to be made has none.
fn check_names(root: &Path, path: &str) -> Result<(), String> {
    match fs::symlink_metadata(root.join(path)) {
        Ok(meta) if meta.nlink() > 1 => Err(format!(
            "{path} has other names (a hard link): gdd init never writes through one: copy it \
             to a file of its own and run it again"
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// Whether there is something at `path` in the folder `root`: a file, a folder or a link, even
/// a broken one.
fn exists(root: &Path, path: &str) -> Result<bool, String> {
    match fs::symlink_metadata(root.join(path)) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

/// What gdd init appends to the `.gitignore` of the folder `root`: each of `lines`, then
/// `.gdd/`, that no line of the file is exactly, in that order and each once, after a line break
/// when the file does not end with one. `None` when it has them all already.
fn ignore(root: &Path, lines: &[&str]) -> Result<Option<String>, String> {
    let text = match fs::read_to_string(root.join(GITIGNORE)) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {GITIGNORE}: {e}")),
    };
    let wanted = stacks::once(lines.iter().chain([&LOCAL_STATE]));
    let there = |line: &&str| text.lines().any(|known| known == *line);
    let missing: Vec<&str> = wanted.into_iter().filter(|line| !there(line)).collect();
    if missing.is_empty() {
        return Ok(None);
    }
    let after = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    Ok(Some(format!("{after}{}\n", missing.join("\n"))))
}

/// Writes `text` as the new file `path` of the folder `root`, creating its folders; a file
/// already there is never overwritten.
fn create(root: &Path, path: &str, text: &str) -> Result<(), String> {
    let file = root.join(path);
    let folder = file.parent().unwrap_or(root);
    let mut new = OpenOptions::new();
    new.write(true).create_new(true);
    let written = fs::create_dir_all(folder)
        .and_then(|()| new.open(&file))
        .and_then(|mut opened| opened.write_all(text.as_bytes()));
    written.map_err(|e| cannot_write(path, &e))
}

/// Adds `text` at the end of the file `path` of the folder `root`, which it creates when absent.
fn append(root: &Path, path: &str, text: &str) -> Result<(), String> {
    let mut at_end = OpenOptions::new();
    at_end.append(true).create(true);
    let written = at_end
        .open(root.join(path))
        .and_then(|mut file| file.write_all(text.as_bytes()));
    written.map_err(|e| cannot_write(path, &e))
}

/// Why `path` could not be written.
fn cannot_write(path: &str, e: &std::io::Error) -> String {
    format!("cannot write {path}: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Folder, env};
    use std::process::Command;

    fn names(named: &[&str]) -> Vec<String> {
        named.iter().map(|name| name.to_string()).collect()
    }

    fn options(stacks: &[&str], agents: &[&str]) -> Options {
        Options {
            name: "Notes".to_string(),
            platform: "cli".to_string(),
            stacks: names(stacks),
            agents: names(agents),
            language: "en".to_string(),
            description: String::new(),
            sensitive: Vec::new(),
            accept_same_family: false,
        }
    }

    /// Sets up the folder for a Rust project built by Claude Code and reviewed by Codex.
    fn init_notes(folder: &Folder) -> Result<String, Error> {
        let options = options(&["rust"], &["claude", "codex"]);
        init(&folder.0, &options, &env())
    }

    fn read(folder: &Folder, path: &str) -> String {
        fs::read_to_string(folder.0.join(path)).unwrap()
    }

    /// Runs git in the folder, in the environment of the tests: what it printed, trimmed.
    pub(super) fn git(folder: &Folder, args: &[&str]) -> String {
        let mut git = Command::new("git");
        git.args(args).current_dir(&folder.0);
        let out = git.env_clear().envs(env()).output().unwrap();
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "git {args:?}: {said}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn a_new_folder_gets_the_harness_in_its_first_commit() {
        let folder = Folder::new();
        let said = init_notes(&folder).unwrap();
        let lines: Vec<&str> = said.lines().collect();
        let first = [
            "gdd/project.toml",
            "gdd/commands.toml",
            "gdd/rules.toml",
            "gdd/zones.toml",
            "gdd/models.toml",
            "gdd/generated.toml",
            "docs/brief.md",
            "bin/gdd",
            ".github/workflows/gdd.yml",
            "AGENTS.md",
            "CLAUDE.md",
            "Cargo.toml",
            "deny.toml",
            "features/.gitkeep",
            "src/lib.rs",
            "tests/acceptance.rs",
            "tests/acceptance/world.rs",
            ".gitignore",
            ".claude/skills/gdd-build/SKILL.md",
        ];
        assert_eq!(lines[..first.len()], first);
        let skill = ".agents/skills/gdd-stack-rust/SKILL.md";
        assert_eq!(lines[lines.len() - 2..], [skill, NEXT]);
        let next = "Next: run bin/gdd bootstrap (first install of each stack and its lock file), \
                    bin/gdd agents check, then start your agent and run the gdd-setup command \
                    (/gdd-setup in Claude Code and OpenCode, $gdd-setup in Codex).";
        assert_eq!(NEXT, next);
        assert_eq!(git(&folder, &["branch", "--show-current"]), "main");
        let subject = git(&folder, &["log", "-1", "--format=%s"]);
        assert_eq!(subject, format!("gdd init {VERSION}"));
        let committed = git(&folder, &["ls-files"]);
        let mut written = lines[..lines.len() - 1].to_vec();
        written.sort_unstable();
        let mut committed: Vec<&str> = committed.lines().collect();
        committed.sort_unstable();
        assert_eq!(committed, written);
        assert_eq!(git(&folder, &["status", "--porcelain"]), "");
    }

    #[test]
    fn the_files_say_the_project_and_bin_gdd_runs() {
        let folder = Folder::new();
        init_notes(&folder).unwrap();
        let bin = fs::metadata(folder.0.join(BIN)).unwrap();
        assert_eq!(bin.permissions().mode() & 0o777, 0o755);
        assert_eq!(read(&folder, GITIGNORE), "/target\n.gdd/\n");
        let cargo = read(&folder, "Cargo.toml");
        let package = "[package]\nname = \"notes\"\n";
        assert!(cargo.starts_with(package), "{cargo}");
        let workflow = read(&folder, WORKFLOW);
        let toolchain = "\n      - uses: dtolnay/rust-toolchain@stable\n";
        assert!(workflow.contains(toolchain), "{workflow}");
        let generated: toml::Table = toml::from_str(&read(&folder, GENERATED)).unwrap();
        let once: Vec<toml::Value> = ONCE.iter().map(|&p| p.into()).collect();
        assert_eq!(generated["files"], toml::Value::Array(once));
        let rules = read(&folder, guard::RULES);
        assert!(rules.contains("main_branch = \"main\"\n"), "{rules}");
        let build = "builder_build = [\"features/**\", \"tests/acceptance/**\", \
                     \"tests/acceptance.rs\"]   #";
        assert!(rules.contains(build), "{rules}");
        let project: toml::Table = toml::from_str(&read(&folder, PROJECT)).unwrap();
        assert_eq!(project["scenario_runner"].as_str(), Some("cucumber-rs"));
        let models = read(&folder, models::PATH);
        let reviewer = "[reviewer]\nintegration = \"codex\"\n";
        assert!(models.contains(reviewer), "{models}");
        let commands = read(&folder, gates::COMMANDS);
        let rust = files::commands(&names(&["rust"])).unwrap();
        assert_eq!(commands, rust);
        assert_eq!(read(&folder, "CLAUDE.md"), "@AGENTS.md\n");
        let brief = read(&folder, approve::BRIEF);
        let title = "# Project Brief · Notes\n";
        assert!(brief.starts_with(title), "{brief}");
    }

    #[test]
    fn in_a_repository_only_what_gdd_init_wrote_is_committed_on_the_current_branch() {
        let folder = Folder::new();
        git(&folder, &["init", "-q", "-b", "trunk"]);
        folder.write("src/main.rs", "fn main() {}\n");
        folder.write("AGENTS.md", "Our rules\n");
        folder.write(".gitignore", "target/");
        git(&folder, &["add", "--", "src/main.rs"]);
        git(&folder, &["commit", "-q", "-m", "mine"]);
        folder.write("src/main.rs", "fn main() { todo!() }\n");
        folder.write("notes.txt", "mine\n");
        let said = init_notes(&folder).unwrap();
        let rules = read(&folder, guard::RULES);
        assert!(rules.contains("main_branch = \"trunk\"\n"), "{rules}");
        let workflow = read(&folder, WORKFLOW);
        let branches = "branches: [\"trunk\", ";
        assert!(workflow.contains(branches), "{workflow}");
        assert_eq!(read(&folder, "AGENTS.md"), "Our rules\n");
        assert_eq!(read(&folder, GITIGNORE), "target/\n/target\n.gdd/\n");
        let yours = format!("\n{}\n{NEXT}", (files::INSTRUCTIONS[0].yours)());
        assert!(said.ends_with(&yours), "{said}");
        assert!(!said.contains("\nAGENTS.md\n"), "{said}");
        let status = git(&folder, &["status", "--porcelain"]);
        assert_eq!(status, "M src/main.rs\n?? AGENTS.md\n?? notes.txt");
        let last = git(&folder, &["show", "--name-only", "--format=", "HEAD"]);
        assert!(!last.contains("src/main.rs"), "{last}");
        assert!(last.contains(".gitignore"), "{last}");
        assert_eq!(git(&folder, &["rev-list", "--count", "trunk"]), "2");
    }

    #[test]
    fn a_gitignore_that_says_it_already_is_left_alone() {
        let folder = Folder::new();
        folder.write(GITIGNORE, "/target\n.gdd/\n");
        folder.write("CLAUDE.md", "mine\n");
        let said = init_notes(&folder).unwrap();
        assert!(!said.contains(".gitignore"), "{said}");
        assert_eq!(read(&folder, GITIGNORE), "/target\n.gdd/\n");
        let claude = "\nCLAUDE.md is yours: add to CLAUDE.md: @AGENTS.md\n";
        assert!(said.contains(claude), "{said}");
        assert!(said.contains("\nAGENTS.md\n"), "{said}");
        let tracked = git(&folder, &["ls-files", "--", GITIGNORE, "CLAUDE.md"]);
        assert_eq!(tracked, "");
    }

    /// What `.gitignore` gets in `folder` when the stacks leave out nothing.
    fn ignored(folder: &Folder) -> Result<Option<String>, String> {
        ignore(&folder.0, &[])
    }

    #[test]
    fn what_gitignore_gets() {
        let folder = Folder::new();
        assert_eq!(ignored(&folder), Ok(Some(".gdd/\n".to_string())));
        folder.write(GITIGNORE, "");
        assert_eq!(ignored(&folder), Ok(Some(".gdd/\n".to_string())));
        folder.write(GITIGNORE, "a\n");
        assert_eq!(ignored(&folder), Ok(Some(".gdd/\n".to_string())));
        folder.write(GITIGNORE, "a");
        assert_eq!(ignored(&folder), Ok(Some("\n.gdd/\n".to_string())));
        for close in ["/.gdd/\n", ".gdd\n", ".gdd/ \n", "# .gdd/\n"] {
            folder.write(GITIGNORE, close);
            let said = ignored(&folder);
            assert_eq!(said, Ok(Some(".gdd/\n".to_string())), "{close:?}");
        }
        folder.write(GITIGNORE, "a\n.gdd/");
        assert_eq!(ignored(&folder), Ok(None));
        // A .gitignore that cannot be read is not a missing one.
        fs::remove_file(folder.0.join(GITIGNORE)).unwrap();
        fs::create_dir(folder.0.join(GITIGNORE)).unwrap();
        let said = ignored(&folder).unwrap_err();
        assert!(said.starts_with("cannot read .gitignore: "), "{said}");
    }

    #[test]
    fn gitignore_gets_the_lines_of_the_stacks_then_gdd_each_once() {
        let folder = Folder::new();
        let lines = ["/target", "node_modules/", "/target"];
        let all = "/target\nnode_modules/\n.gdd/\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(all)));
        folder.write(GITIGNORE, "node_modules/\n/target/");
        let rest = "\n/target\n.gdd/\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(rest)));
        folder.write(GITIGNORE, ".gdd/\nnode_modules/\n");
        let target = "/target\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(target)));
        folder.write(GITIGNORE, "/target\n.gdd/\nnode_modules/\n");
        assert_eq!(ignore(&folder.0, &lines), Ok(None));
    }

    #[test]
    fn a_file_already_there_is_named_and_nothing_is_written() {
        let folder = Folder::new();
        folder.write(approve::BRIEF, "mine\n");
        folder.write(".claude/skills/gdd-setup/SKILL.md", "mine\n");
        let said = init_notes(&folder).unwrap_err();
        let paths = names(&[approve::BRIEF, ".claude/skills/gdd-setup/SKILL.md"]);
        assert_eq!(said, Error::Exists(paths));
        let shown = "docs/brief.md already exists\n\
                     .claude/skills/gdd-setup/SKILL.md already exists\n\
                     error: gdd init never overwrites a file: move or remove these, then run \
                     it again";
        assert_eq!(said.to_string(), shown);
        assert!(!folder.0.join("gdd").exists());
        assert!(!folder.0.join(".git").exists());
        assert_eq!(read(&folder, approve::BRIEF), "mine\n");
        // A link counts, even one to nothing.
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join("bin")).unwrap();
        std::os::unix::fs::symlink("nowhere", folder.0.join(BIN)).unwrap();
        let said = init_notes(&folder).unwrap_err();
        assert_eq!(said, Error::Exists(vec![BIN.to_string()]));
    }

    #[test]
    fn a_skeleton_file_already_there_is_named_with_the_others() {
        let folder = Folder::new();
        folder.write("src/lib.rs", "mine\n");
        folder.write(approve::BRIEF, "mine\n");
        folder.write(".claude/skills/gdd-setup/SKILL.md", "mine\n");
        let said = init_notes(&folder).unwrap_err();
        let paths = [
            approve::BRIEF,
            "src/lib.rs",
            ".claude/skills/gdd-setup/SKILL.md",
        ];
        assert_eq!(said, Error::Exists(names(&paths)));
        assert!(!folder.0.join("Cargo.toml").exists());
        assert_eq!(read(&folder, "src/lib.rs"), "mine\n");
    }

    #[test]
    fn a_folder_with_a_project_of_a_stack_gets_only_the_other_skeletons() {
        let folder = Folder::new();
        let mine = "[package]\nname = \"mine\"\n";
        folder.write("Cargo.toml", mine);
        let options = options(&["rust", "typescript"], &["claude", "codex"]);
        let said = init(&folder.0, &options, &env()).unwrap();
        assert_eq!(read(&folder, "Cargo.toml"), mine);
        assert!(!folder.0.join("src/lib.rs").exists());
        assert!(said.contains("\npackage.json\n"), "{said}");
        assert!(!said.contains("Cargo.toml"), "{said}");
        let ignored = read(&folder, GITIGNORE);
        assert!(ignored.starts_with("/target\nnode_modules/\n"), "{ignored}");
        assert!(ignored.ends_with("\n.stryker-tmp/\n.gdd/\n"), "{ignored}");
        let committed = git(&folder, &["ls-files", "--", "package.json", "Cargo.toml"]);
        assert_eq!(committed, "package.json");
    }

    #[test]
    fn stacks_that_share_a_skeleton_file_write_it_once() {
        let folder = Folder::new();
        let options = options(&["python", "typescript"], &["claude", "codex"]);
        let said = init(&folder.0, &options, &env()).unwrap();
        let shared = said.lines().filter(|line| *line == "osv-scanner.toml");
        assert_eq!(shared.count(), 1, "{said}");
        let package = read(&folder, "src/notes/greeting.py");
        assert!(package.contains("def greeting"), "{package}");
        assert_eq!(git(&folder, &["status", "--porcelain"]), "");
    }

    #[test]
    fn a_path_that_cannot_be_looked_at_is_not_a_missing_one() {
        let folder = Folder::new();
        folder.write("docs", "a file where a folder goes\n");
        let said = init_notes(&folder).unwrap_err().to_string();
        let start = "error: cannot read docs/brief.md: ";
        assert!(said.starts_with(start), "{said}");
        assert!(!folder.0.join("gdd").exists());
        assert!(exists(&folder.0, "docs").unwrap());
        assert!(!exists(&folder.0, "none").unwrap());
        // Each check on its own: a path under a file is not a missing one.
        let said = exists(&folder.0, "docs/brief.md").unwrap_err();
        assert!(said.starts_with("cannot read docs/brief.md: "), "{said}");
        let said = check_links(&folder.0, ["docs/brief.md"].into_iter()).unwrap_err();
        assert!(said.starts_with("cannot read docs/brief.md: "), "{said}");
        let said = check_names(&folder.0, "docs/.gitignore").unwrap_err();
        assert!(said.starts_with("cannot read docs/.gitignore: "), "{said}");
    }

    #[test]
    fn nothing_is_appended_through_a_hard_link() {
        let elsewhere = Folder::new();
        elsewhere.write(GITIGNORE, "target/\n");
        let folder = Folder::new();
        fs::hard_link(elsewhere.0.join(GITIGNORE), folder.0.join(GITIGNORE)).unwrap();
        let said = init_notes(&folder).unwrap_err().to_string();
        let why = "error: .gitignore has other names (a hard link): gdd init never writes \
                   through one: copy it to a file of its own and run it again";
        assert_eq!(said, why);
        assert!(!folder.0.join(".git").exists());
        assert!(!folder.0.join("gdd").exists());
        assert_eq!(read(&elsewhere, GITIGNORE), "target/\n");
        // One that already has every line gets nothing, so its other names do not matter.
        elsewhere.write(GITIGNORE, "/target\n.gdd/\n");
        init_notes(&folder).unwrap();
        assert_eq!(read(&elsewhere, GITIGNORE), "/target\n.gdd/\n");
    }

    #[test]
    fn nothing_is_written_through_a_symbolic_link() {
        let elsewhere = Folder::new();
        elsewhere.write(GITIGNORE, "target/\n");
        let skill = ".claude/skills/gdd-build/SKILL.md";
        let links = [
            ("docs", elsewhere.0.clone(), approve::BRIEF),
            (".claude", elsewhere.0.clone(), skill),
            (GITIGNORE, elsewhere.0.join(GITIGNORE), GITIGNORE),
            ("src", elsewhere.0.clone(), "src/lib.rs"),
        ];
        for (link, to, path) in links {
            let folder = Folder::new();
            std::os::unix::fs::symlink(&to, folder.0.join(link)).unwrap();
            let said = init_notes(&folder).unwrap_err().to_string();
            let why = format!(
                "error: cannot write {path}: {link} is a symbolic link, which gdd init does not \
                 write through: remove the link and run it again"
            );
            assert_eq!(said, why);
            assert!(!folder.0.join(".git").exists());
            assert!(!folder.0.join("gdd").exists());
        }
        assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 1);
        assert_eq!(read(&elsewhere, GITIGNORE), "target/\n");
    }

    #[test]
    fn a_gdd_project_cannot_be_set_up_twice_nor_inside_another() {
        let folder = Folder::new();
        init_notes(&folder).unwrap();
        let project = folder.0.join(PROJECT);
        let twice = format!("error: already a GDD project: {}", project.display());
        assert_eq!(init_notes(&folder).unwrap_err().to_string(), twice);
        let inside = folder.0.join("app");
        fs::create_dir_all(&inside).unwrap();
        let options = options(&["python"], &["codex", "claude"]);
        let said = init(&inside, &options, &env()).unwrap_err();
        assert_eq!(said.to_string(), twice);
        assert_eq!(fs::read_dir(&inside).unwrap().count(), 0);
        // A folder named like the file is not a project.
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(PROJECT)).unwrap();
        let said = init_notes(&folder).unwrap_err();
        assert_eq!(said, Error::Exists(vec![PROJECT.to_string()]));
    }

    #[test]
    fn the_checks_come_in_order_and_write_nothing() {
        let folder = Folder::new();
        folder.write(approve::BRIEF, "mine\n");
        let wrong: [(&[&str], &[&str], &str); 3] = [
            (&["cobol"], &["gemini"], "unknown stack cobol"),
            (&["rust"], &["gemini"], "no agent sheet for \"gemini\""),
            (&["rust"], &["claude"], "one model family only (anthropic)"),
        ];
        for (stacks, agents, start) in wrong {
            let options = options(stacks, agents);
            let said = init(&folder.0, &options, &env()).unwrap_err();
            let (said, start) = (said.to_string(), format!("error: {start}"));
            assert!(said.starts_with(&start), "{said}");
        }
        assert!(!folder.0.join(".git").exists());
        assert!(!folder.0.join("gdd").exists());
    }

    #[test]
    fn one_family_accepted_is_set_up() {
        let folder = Folder::new();
        let mut options = options(&["rust", "typescript", "rust"], &["claude"]);
        options.accept_same_family = true;
        init(&folder.0, &options, &env()).unwrap();
        let models = read(&folder, models::PATH);
        let now = log::now();
        let accepted = format!("same_family_accepted = \"{}\"", &now[..10]);
        assert!(models.contains(&accepted), "{models}");
        let project: toml::Table = toml::from_str(&read(&folder, PROJECT)).unwrap();
        let stacks = toml::Value::Array(vec!["rust".into(), "typescript".into()]);
        assert_eq!(project["stack"], stacks);
        let skill = ".claude/skills/gdd-stack-typescript/SKILL.md";
        assert!(folder.0.join(skill).is_file());
    }

    #[test]
    fn what_git_refuses_stops_the_run() {
        // Without git, no repository and nothing written.
        let folder = Folder::new();
        let nowhere = [("PATH".to_string(), "/nonexistent".to_string())];
        let options = options(&["rust"], &["claude", "codex"]);
        let said = init(&folder.0, &options, &nowhere).unwrap_err();
        let said = said.to_string();
        assert!(said.starts_with("error: cannot run git: "), "{said}");
        assert!(!folder.0.join("gdd").exists());
        // A repository whose HEAD is not a branch has no current branch to name.
        git(&folder, &["init", "-q", "-b", "main"]);
        git(&folder, &["commit", "--allow-empty", "-qm", "one"]);
        git(&folder, &["checkout", "-q", "--detach"]);
        let said = init_notes(&folder).unwrap_err().to_string();
        let failed = "error: git symbolic-ref --short HEAD failed";
        assert!(said.starts_with(failed), "{said}");
        assert!(!folder.0.join("gdd").exists());
    }

    #[test]
    fn what_cannot_be_written_stops_the_run() {
        let folder = Folder::new();
        let said = create(&folder.0, "", "x").unwrap_err();
        assert!(said.starts_with("cannot write : "), "{said}");
        folder.write("a", "file\n");
        let said = append(&folder.0, "a/b", "x").unwrap_err();
        assert!(said.starts_with("cannot write a/b: "), "{said}");
        create(&folder.0, "c/d", "x").unwrap();
        let said = create(&folder.0, "c/d", "y").unwrap_err();
        assert!(said.starts_with("cannot write c/d: "), "{said}");
        assert_eq!(read(&folder, "c/d"), "x");
        append(&folder.0, "c/d", "z").unwrap();
        assert_eq!(read(&folder, "c/d"), "xz");
    }
}
