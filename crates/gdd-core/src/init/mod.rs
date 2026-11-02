//! `tminus init`: sets up GDD in the folder it runs in, which becomes the project root, from what
//! the command line says, always in the new layout (`tminus/` and `.tminus/`, see
//! [`crate::layout`]). It checks everything before it writes anything, in this order: no GDD
//! project of either layout in the folder or above it, nor a project file there that cannot be
//! looked at, no state of the old layout (`.gdd/`) where its state goes (see
//! [`crate::layout::check`]), the stacks known, the agents and who plays each role (see
//! [`roles`]), the skeletons of the stacks (see [`skeletons`]), none of the files it writes
//! already there, for it never overwrites one, no symbolic link on the way to what it writes and
//! no other name (a hard link) for the file it appends to, for it never writes out of the folder.
//! Then it writes the harness (see [`files`]), AGENTS.md (with how to install and the G2
//! commands of the stacks) and CLAUDE.md when the project has none,
//! the skeleton of each stack the folder has no project of, the lines of every stack and `.tminus/`
//! in `.gitignore` and the agents' commands as `tminus agents install` does, and commits what it
//! wrote, and only that, in a new repository when the folder is in none. It stays offline:
//! [`bootstrap()`] runs the first install of each stack. What it was asked goes with the files it
//! writes once ([`Answers`]), so a later gdd renders them again for the same project (see
//! [`render_once`]).
//!
//! Git runs in the environment gdd runs in, which says who commits (git's own rules).

mod bootstrap;
mod files;
mod roles;
mod skeletons;

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agents::models;
use crate::agents::render;
use crate::agents::sheets::Sheet;
use crate::gates::{self, approve};
use crate::git::{self, Git};
use crate::layout::{self, Layout};
use crate::{VERSION, changelog, guard, log, stacks, state};

pub use bootstrap::{Stopped, bootstrap};
pub use roles::{agents_on_path, family_of, strong_model};

/// The platforms a project may run on.
pub const PLATFORMS: [&str; 5] = ["mobile", "desktop", "web", "cli", "other"];

/// The file that marks the root of a GDD project, in the new names (see [`crate::layout`]).
const PROJECT: &str = "tminus/project.toml";
/// The zone map.
const ZONES: &str = "tminus/zones.toml";
/// Which tminus wrote the files written once, and what tminus init was asked.
pub(crate) const GENERATED: &str = "tminus/generated.toml";
/// The CI workflow.
const WORKFLOW: &str = ".github/workflows/tminus.yml";
/// What git leaves out.
const GITIGNORE: &str = ".gitignore";
/// The line of `.gitignore` that leaves out the local state of the tool.
const LOCAL_STATE: &str = ".tminus/";
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

/// Why tminus init stops when one of its files is already there.
const NEVER_OVERWRITES: &str =
    "tminus init never overwrites a file: move or remove these, then run it again";

/// What to do once the project is set up.
const NEXT: &str = "Next: run tminus bootstrap (first install of each stack and its lock file), \
                    then start your agent and run the tminus-setup command \
                    (/tminus-setup in Claude Code and OpenCode, $tminus-setup in Codex).";

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

/// What gdd init was asked, as `gdd/generated.toml` records it under `[answers]`: what the files
/// it writes once are rendered from (see [`render_once`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answers {
    /// The name of the project.
    pub name: String,
    /// What it is, in words; empty when the command line did not say.
    pub description: String,
    /// Where it runs: one of [`PLATFORMS`].
    pub platform: String,
    /// Its stacks, each once.
    pub stacks: Vec<String>,
    /// The agents of the cast, in order, the first builds: the ones named, or those gdd init
    /// found on the `PATH`.
    pub agents: Vec<String>,
    /// The language the agents use with the human.
    pub language: String,
    /// What would be serious if it failed, one area each.
    pub sensitive: Vec<String>,
    /// The human accepted a Reviewer of the Builder's model family when every agent is of one.
    pub accept_same_family: bool,
    /// The day gdd init ran: `YYYY-MM-DD`.
    pub day: String,
    /// The main branch it wrote into the files.
    pub main: String,
}

impl Answers {
    /// What the command line said of the project, for the texts that read it.
    fn options(&self) -> Options {
        Options {
            name: self.name.clone(),
            platform: self.platform.clone(),
            stacks: self.stacks.clone(),
            agents: self.agents.clone(),
            language: self.language.clone(),
            description: self.description.clone(),
            sensitive: self.sensitive.clone(),
            accept_same_family: self.accept_same_family,
        }
    }
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
    if let Some((project, file)) = layout::project_above(root)? {
        let why = format!("already a GDD project: {}", project.join(file).display());
        return Err(Error::Failed(why));
    }
    // It writes the new layout: where the state of the old one is, every later command would
    // refuse the project it set up (see [`layout::check`]).
    layout::check(root, &state::root(root)?)?;
    let now = log::now();
    let (today, _) = now.split_once('T').unwrap_or((now.as_str(), ""));
    let plan = Plan::new(root, options, env, today)?;
    plan.carry_out(root)
}

/// The texts of the files tminus init writes once ([`ONCE`]) and of `tminus/generated.toml`,
/// which keeps the `answers`, in the order tminus init writes them: what it writes for the
/// `answers`, with the `PATH` of `env` when they name no agent, as a project of `layout` has them
/// (paths and texts, see [`Layout::localize`]): what tminus init writes in the new layout, what
/// gdd wrote before the rename in the old one. What the person typed (the name, the description,
/// the sensitive areas) stays as typed. Nothing is read from or written to the disk.
pub(crate) fn render_once(
    answers: &Answers,
    env: &Env,
    layout: Layout,
) -> Result<Vec<(String, String)>, Error> {
    let stacks = stacks::once(&answers.stacks);
    let sheets = stack_sheets(&stacks)?;
    let accept = answers.accept_same_family;
    let cast = roles::Cast::new(&answers.agents, env, accept, &answers.day)?;
    let options = answers.options();
    let values = files::Values::new(&options, &sheets, &answers.main);
    // Each template in the names of the layout first, then its placeholders filled.
    let fill = |template: &str| values.fill(&layout.localize(template));
    let once = ONCE.map(|path| layout.path(path));
    let texts = [
        (PROJECT, files::project(&options, &stacks, &sheets, layout)),
        (gates::COMMANDS, layout.localize(&files::commands(&stacks)?)),
        (guard::RULES, fill(files::RULES)),
        (ZONES, fill(files::ZONES)),
        (models::PATH, layout.localize(&cast.models(VERSION))),
        (GENERATED, files::generated(&once, answers, layout)?),
        (approve::BRIEF, fill(files::BRIEF)),
        (WORKFLOW, fill(files::WORKFLOW)),
    ];
    let texts = texts.map(|(path, text)| (layout.path(path), text));
    Ok(Vec::from(texts))
}

/// The agents of a new project, in order: the ones `named`, each once; when none is, those whose
/// program is on the `PATH` of `env`.
fn agents_of(named: &[String], env: &Env) -> Result<Vec<String>, String> {
    if named.is_empty() {
        agents_on_path(env)
    } else {
        Ok(stacks::once(named))
    }
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
        let named = agents_of(&options.agents, env)?;
        let cast = roles::Cast::new(&named, env, accept, today)?;
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
                let text = (item.text)(item.template, &stacks)?;
                instructions.push((item.file.to_string(), text));
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
        let answers = Answers {
            name: options.name.clone(),
            description: options.description.clone(),
            platform: options.platform.clone(),
            stacks: stacks.clone(),
            agents: named,
            language: options.language.clone(),
            sensitive: options.sensitive.clone(),
            accept_same_family: accept,
            day: today.to_string(),
            main,
        };
        // The harness: the files written once, always in the new layout.
        let mut texts = render_once(&answers, env, Layout::Tminus)?;
        texts.extend(instructions);
        if !exists(root, changelog::PATH)? {
            texts.push((changelog::PATH.to_string(), changelog::started()));
        }
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
        if let Some(line) = &self.ignore {
            append(root, GITIGNORE, line)?;
            written.push(GITIGNORE.to_string());
        }
        written.extend(render::install(root, &self.agents, &self.stacks)?);
        let message = format!("tminus init {VERSION}");
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
                "cannot write {path}: {link} is a symbolic link, which tminus init does not \
                 write through: remove the link and run it again"
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
            "{path} has other names (a hard link): tminus init never writes through one: copy \
             it to a file of its own and run it again"
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

/// What tminus init appends to the `.gitignore` of the folder `root`: each of `lines`, then
/// `.tminus/`, that no line of the file is exactly, in that order and each once, after a line break
/// when the file does not end with one. `None` when it has them all already.
pub(crate) fn ignore(root: &Path, lines: &[&str]) -> Result<Option<String>, String> {
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
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
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
            "tminus/project.toml",
            "tminus/commands.toml",
            "tminus/rules.toml",
            "tminus/zones.toml",
            "tminus/models.toml",
            "tminus/generated.toml",
            "docs/brief.md",
            ".github/workflows/tminus.yml",
            "AGENTS.md",
            "CLAUDE.md",
            "CHANGELOG.md",
            "Cargo.toml",
            "deny.toml",
            "features/.gitkeep",
            "src/lib.rs",
            "tests/acceptance.rs",
            "tests/acceptance/world.rs",
            ".gitignore",
            ".claude/skills/tminus-build/SKILL.md",
        ];
        assert_eq!(lines[..first.len()], first);
        let skill = ".agents/skills/tminus-stack-rust/SKILL.md";
        assert_eq!(lines[lines.len() - 2..], [skill, NEXT]);
        let next = "Next: run tminus bootstrap (first install of each stack and its lock file), \
                    then start your agent and run the tminus-setup command \
                    (/tminus-setup in Claude Code and OpenCode, $tminus-setup in Codex).";
        assert_eq!(NEXT, next);
        assert!(!folder.0.join("bin").exists());
        assert!(!folder.0.join("gdd").exists() && !folder.0.join(".gdd").exists());
        assert_eq!(Layout::at(&folder.0), Layout::Tminus);
        assert_eq!(git(&folder, &["branch", "--show-current"]), "main");
        let subject = git(&folder, &["log", "-1", "--format=%s"]);
        assert_eq!(subject, format!("tminus init {VERSION}"));
        let committed = git(&folder, &["ls-files"]);
        let mut written = lines[..lines.len() - 1].to_vec();
        written.sort_unstable();
        let mut committed: Vec<&str> = committed.lines().collect();
        committed.sort_unstable();
        assert_eq!(committed, written);
        assert_eq!(git(&folder, &["status", "--porcelain"]), "");
    }

    #[test]
    fn a_new_folder_starts_its_changelog_and_a_changelog_already_there_stays() {
        let folder = Folder::new();
        init_notes(&folder).unwrap();
        assert_eq!(read(&folder, changelog::PATH), changelog::started());
        let tracked = git(&folder, &["ls-files", changelog::PATH]);
        assert_eq!(tracked, changelog::PATH);
        let theirs = Folder::new();
        theirs.write(changelog::PATH, "# Our notes\n");
        let said = init_notes(&theirs).unwrap();
        assert!(!said.lines().any(|line| line == changelog::PATH), "{said}");
        assert_eq!(read(&theirs, changelog::PATH), "# Our notes\n");
        let tracked = git(&theirs, &["ls-files", changelog::PATH]);
        assert_eq!(tracked, "");
    }

    #[test]
    fn the_files_say_the_project() {
        let folder = Folder::new();
        init_notes(&folder).unwrap();
        assert_eq!(read(&folder, GITIGNORE), "/target\n.tminus/\n");
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
        let agents = read(&folder, "AGENTS.md");
        let unit = "\n- `unit`: `cargo test --workspace`\n";
        assert!(agents.contains(unit), "{agents}");
        assert!(!agents.contains("{checks}"), "{agents}");
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
        assert_eq!(read(&folder, GITIGNORE), "target/\n/target\n.tminus/\n");
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
        folder.write(GITIGNORE, "/target\n.tminus/\n");
        folder.write("CLAUDE.md", "mine\n");
        let said = init_notes(&folder).unwrap();
        assert!(!said.contains(".gitignore"), "{said}");
        assert_eq!(read(&folder, GITIGNORE), "/target\n.tminus/\n");
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

    /// What `.gitignore` gets when it lacks only the state folder.
    fn state() -> Result<Option<String>, String> {
        Ok(Some(".tminus/\n".to_string()))
    }

    #[test]
    fn what_gitignore_gets() {
        let folder = Folder::new();
        assert_eq!(ignored(&folder), state());
        folder.write(GITIGNORE, "");
        assert_eq!(ignored(&folder), state());
        folder.write(GITIGNORE, "a\n");
        assert_eq!(ignored(&folder), state());
        folder.write(GITIGNORE, "a");
        assert_eq!(ignored(&folder), Ok(Some("\n.tminus/\n".to_string())));
        // The state folder of the old layout is not the one tminus init sets up.
        for close in [
            "/.tminus/\n",
            ".tminus\n",
            ".tminus/ \n",
            "# .tminus/\n",
            ".gdd/\n",
        ] {
            folder.write(GITIGNORE, close);
            assert_eq!(ignored(&folder), state(), "{close:?}");
        }
        folder.write(GITIGNORE, "a\n.tminus/");
        assert_eq!(ignored(&folder), Ok(None));
        // A .gitignore that cannot be read is not a missing one.
        fs::remove_file(folder.0.join(GITIGNORE)).unwrap();
        fs::create_dir(folder.0.join(GITIGNORE)).unwrap();
        let said = ignored(&folder).unwrap_err();
        assert!(said.starts_with("cannot read .gitignore: "), "{said}");
    }

    #[test]
    fn gitignore_gets_the_lines_of_the_stacks_then_the_state_folder_each_once() {
        let folder = Folder::new();
        let lines = ["/target", "node_modules/", "/target"];
        let all = "/target\nnode_modules/\n.tminus/\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(all)));
        folder.write(GITIGNORE, "node_modules/\n/target/");
        let rest = "\n/target\n.tminus/\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(rest)));
        folder.write(GITIGNORE, ".tminus/\nnode_modules/\n");
        let target = "/target\n".to_string();
        assert_eq!(ignore(&folder.0, &lines), Ok(Some(target)));
        folder.write(GITIGNORE, "/target\n.tminus/\nnode_modules/\n");
        assert_eq!(ignore(&folder.0, &lines), Ok(None));
    }

    #[test]
    fn a_file_already_there_is_named_and_nothing_is_written() {
        let folder = Folder::new();
        folder.write(approve::BRIEF, "mine\n");
        folder.write(".claude/skills/tminus-setup/SKILL.md", "mine\n");
        let said = init_notes(&folder).unwrap_err();
        let paths = names(&[approve::BRIEF, ".claude/skills/tminus-setup/SKILL.md"]);
        assert_eq!(said, Error::Exists(paths));
        let shown = "docs/brief.md already exists\n\
                     .claude/skills/tminus-setup/SKILL.md already exists\n\
                     error: tminus init never overwrites a file: move or remove these, then run \
                     it again";
        assert_eq!(said.to_string(), shown);
        assert!(!folder.0.join("tminus").exists());
        assert!(!folder.0.join(".git").exists());
        assert_eq!(read(&folder, approve::BRIEF), "mine\n");
        // A link counts, even one to nothing.
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join(".github/workflows")).unwrap();
        std::os::unix::fs::symlink("nowhere", folder.0.join(WORKFLOW)).unwrap();
        let said = init_notes(&folder).unwrap_err();
        assert_eq!(said, Error::Exists(vec![WORKFLOW.to_string()]));
    }

    #[test]
    fn a_skeleton_file_already_there_is_named_with_the_others() {
        let folder = Folder::new();
        folder.write("src/lib.rs", "mine\n");
        folder.write(approve::BRIEF, "mine\n");
        folder.write(".claude/skills/tminus-setup/SKILL.md", "mine\n");
        let said = init_notes(&folder).unwrap_err();
        let paths = [
            approve::BRIEF,
            "src/lib.rs",
            ".claude/skills/tminus-setup/SKILL.md",
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
        assert!(
            ignored.ends_with("\n.stryker-tmp/\n.tminus/\n"),
            "{ignored}"
        );
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
        assert!(!folder.0.join("tminus").exists());
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
        let why = "error: .gitignore has other names (a hard link): tminus init never writes \
                   through one: copy it to a file of its own and run it again";
        assert_eq!(said, why);
        assert!(!folder.0.join(".git").exists());
        assert!(!folder.0.join("tminus").exists());
        assert_eq!(read(&elsewhere, GITIGNORE), "target/\n");
        // One that already has every line gets nothing, so its other names do not matter.
        elsewhere.write(GITIGNORE, "/target\n.tminus/\n");
        init_notes(&folder).unwrap();
        assert_eq!(read(&elsewhere, GITIGNORE), "/target\n.tminus/\n");
    }

    #[test]
    fn nothing_is_written_through_a_symbolic_link() {
        let elsewhere = Folder::new();
        elsewhere.write(GITIGNORE, "target/\n");
        let skill = ".claude/skills/tminus-build/SKILL.md";
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
                "error: cannot write {path}: {link} is a symbolic link, which tminus init does \
                 not write through: remove the link and run it again"
            );
            assert_eq!(said, why);
            assert!(!folder.0.join(".git").exists());
            assert!(!folder.0.join("tminus").exists());
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
    fn a_project_set_up_before_the_rename_is_a_gdd_project_too() {
        let folder = Folder::new();
        folder.write("gdd/project.toml", "name = \"Notes\"\n");
        let project = folder.0.join("gdd/project.toml");
        let old = format!("error: already a GDD project: {}", project.display());
        assert_eq!(init_notes(&folder).unwrap_err().to_string(), old);
        let inside = folder.0.join("app");
        fs::create_dir_all(&inside).unwrap();
        let options = options(&["rust"], &["claude", "codex"]);
        let said = init(&inside, &options, &env()).unwrap_err();
        assert_eq!(said.to_string(), old);
        assert_eq!(fs::read_dir(&inside).unwrap().count(), 0);
        assert!(!folder.0.join("tminus").exists() && !folder.0.join(".git").exists());
        // A folder named like it is not one either.
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join("gdd/project.toml")).unwrap();
        init_notes(&folder).unwrap();
        assert!(folder.0.join(PROJECT).is_file());
    }

    #[test]
    fn a_project_file_that_cannot_be_looked_at_is_refused_not_passed_over() {
        let folder = Folder::new();
        folder.write("gdd/project.toml", "name = \"Notes\"\n");
        let inside = folder.0.join("app");
        fs::create_dir_all(&inside).unwrap();
        // A link to itself: what is under it cannot be looked at. The project above is not the
        // one this folder is in, nor is the folder free to set up.
        std::os::unix::fs::symlink("tminus", inside.join("tminus")).unwrap();
        let options = options(&["rust"], &["claude", "codex"]);
        let said = init(&inside, &options, &env()).unwrap_err().to_string();
        let path = inside.join("tminus/project.toml");
        let start = format!("error: cannot read {}: ", path.display());
        assert!(said.starts_with(&start), "{said}");
        assert_eq!(fs::read_dir(&inside).unwrap().count(), 1);
        assert!(!folder.0.join(".git").exists());
    }

    #[test]
    fn a_folder_with_the_state_of_the_old_layout_is_refused_before_anything_is_written() {
        let folder = Folder::new();
        folder.write(".gdd/log.jsonl", "");
        let refused = format!(
            "error: {} keeps its files in tminus/ but has .gdd/, the state of the old layout: \
             move .gdd to .tminus or remove it",
            folder.0.display()
        );
        assert_eq!(init_notes(&folder).unwrap_err().to_string(), refused);
        assert!(!folder.0.join("tminus").exists() && !folder.0.join(".git").exists());
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1);
        // The state of the new layout is its own.
        let fresh = Folder::new();
        fresh.write(".tminus/log.jsonl", "");
        init_notes(&fresh).unwrap();
        assert!(fresh.0.join(PROJECT).is_file());
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
        assert!(!folder.0.join("tminus").exists());
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
        let skill = ".claude/skills/tminus-stack-typescript/SKILL.md";
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
        assert!(!folder.0.join("tminus").exists());
        // A repository whose HEAD is not a branch has no current branch to name.
        git(&folder, &["init", "-q", "-b", "main"]);
        git(&folder, &["commit", "--allow-empty", "-qm", "one"]);
        git(&folder, &["checkout", "-q", "--detach"]);
        let said = init_notes(&folder).unwrap_err().to_string();
        let failed = "error: git symbolic-ref --short HEAD failed";
        assert!(said.starts_with(failed), "{said}");
        assert!(!folder.0.join("tminus").exists());
    }

    /// What `tminus/generated.toml` of `folder` says tminus init was asked.
    fn recorded(folder: &Folder) -> Answers {
        let generated: toml::Table = toml::from_str(&read(folder, GENERATED)).unwrap();
        generated["answers"].clone().try_into().unwrap()
    }

    #[test]
    fn what_init_was_asked_renders_what_it_wrote_byte_for_byte() {
        let folder = Folder::new();
        git(&folder, &["init", "-q", "-b", "trunk"]);
        let stacks = ["rust", "typescript", "rust"];
        let mut options = options(&stacks, &["codex", "claude", "codex"]);
        options.description = "Say \"hi\"".to_string();
        options.sensitive = vec!["Losing notes".to_string()];
        init(&folder.0, &options, &env()).unwrap();
        let answers = recorded(&folder);
        let today = &log::now()[..10];
        let expected = Answers {
            name: "Notes".to_string(),
            description: "Say \"hi\"".to_string(),
            platform: "cli".to_string(),
            stacks: names(&["rust", "typescript"]),
            agents: names(&["codex", "claude"]),
            language: "en".to_string(),
            sensitive: names(&["Losing notes"]),
            accept_same_family: false,
            day: today.to_string(),
            main: "trunk".to_string(),
        };
        assert_eq!(answers, expected);
        let rendered = render_once(&answers, &env(), Layout::Tminus).unwrap();
        let paths: Vec<&str> = rendered.iter().map(|(path, _)| path.as_str()).collect();
        let mut once = ONCE.to_vec();
        once.insert(5, GENERATED);
        assert_eq!(paths, once);
        for (path, text) in &rendered {
            assert_eq!(&read(&folder, path), text, "{path}");
        }
    }

    #[test]
    fn the_answers_name_the_agents_found_on_the_path_and_the_day_one_family_was_accepted() {
        // A PATH with Claude Code and git only, whatever agents the machine has.
        let found = Folder::new();
        found.write("claude", "#!/bin/sh\n");
        let runnable = Permissions::from_mode(0o755);
        fs::set_permissions(found.0.join("claude"), runnable).unwrap();
        let path = std::env::var_os("PATH").unwrap();
        let mut programs = std::env::split_paths(&path).map(|folder| folder.join("git"));
        let program = programs.find(|git| git.is_file()).unwrap();
        std::os::unix::fs::symlink(program, found.0.join("git")).unwrap();
        let mut env = env();
        env.retain(|(name, _)| name != "PATH");
        env.push(("PATH".to_string(), found.0.display().to_string()));
        let folder = Folder::new();
        let mut options = options(&["rust"], &[]);
        options.accept_same_family = true;
        init(&folder.0, &options, &env).unwrap();
        let answers = recorded(&folder);
        assert_eq!(answers.agents, ["claude"]);
        assert!(answers.accept_same_family);
        // Rendered on another day, with another PATH, the models file keeps the day it was set up.
        let mut later = answers.clone();
        later.day = "2030-01-01".to_string();
        let rendered = render_once(&answers, &[], Layout::Tminus).unwrap();
        let models = &rendered[4];
        assert_eq!(models.0, models::PATH);
        assert_eq!(models.1, read(&folder, models::PATH));
        let other = render_once(&later, &[], Layout::Tminus).unwrap();
        assert!(other[4].1.contains("\"2030-01-01\""), "{}", other[4].1);
        // Answers that cannot be rendered say why.
        later.stacks = names(&["cobol"]);
        let said = render_once(&later, &[], Layout::Gdd)
            .unwrap_err()
            .to_string();
        assert!(said.starts_with("error: unknown stack cobol"), "{said}");
    }

    /// What `tminus init` records it was asked for a project named `name`.
    fn answers_of(name: &str) -> Answers {
        Answers {
            name: name.to_string(),
            description: "Notes from tminus".to_string(),
            platform: "cli".to_string(),
            stacks: names(&["rust", "python"]),
            agents: names(&["claude", "codex"]),
            language: "en".to_string(),
            sensitive: names(&["Losing tminus notes"]),
            accept_same_family: false,
            day: "2026-10-28".to_string(),
            main: "main".to_string(),
        }
    }

    #[test]
    fn a_project_of_the_old_layout_gets_its_files_in_the_old_names() {
        let old = render_once(&answers_of("Notes"), &[], Layout::Gdd).unwrap();
        let new = render_once(&answers_of("Notes"), &[], Layout::Tminus).unwrap();
        let paths: Vec<&str> = old.iter().map(|(path, _)| path.as_str()).collect();
        let expected = [
            "gdd/project.toml",
            "gdd/commands.toml",
            "gdd/rules.toml",
            "gdd/zones.toml",
            "gdd/models.toml",
            "gdd/generated.toml",
            "docs/brief.md",
            ".github/workflows/gdd.yml",
        ];
        assert_eq!(paths, expected);
        // Each text is the new one in the old names, but for what the person typed.
        for ((path, text), (_, new_text)) in old.iter().zip(&new) {
            let typed = ["Notes from tminus", "Losing tminus notes"];
            let mut tool = text.clone();
            for words in typed {
                tool = tool.replace(words, "");
            }
            assert!(!tool.to_lowercase().contains("tminus"), "{path}: {text}");
            let localized = Layout::Gdd.localize(new_text);
            if typed.iter().any(|words| new_text.contains(words)) {
                assert_ne!(*text, localized, "{path}");
            } else {
                assert_eq!(*text, localized, "{path}");
            }
        }
        let text_of = |path: &str| {
            let found = old.iter().find(|(at, _)| at == path);
            found.map(|(_, text)| text.as_str()).unwrap_or_default()
        };
        let project = text_of("gdd/project.toml");
        let head = format!("# Project basics · GDD (written by gdd init {VERSION})\n");
        assert!(project.starts_with(&head), "{project}");
        assert!(
            project.contains("   # the stack sheets (gdd stacks)\n"),
            "{project}"
        );
        assert!(
            project.contains("description = \"Notes from tminus\"\n"),
            "{project}"
        );
        let generated = text_of("gdd/generated.toml");
        let comment = "# Written by gdd init: the files it writes once and what it was asked, \
                       for gdd upgrade.\n";
        assert!(generated.starts_with(comment), "{generated}");
        let table: toml::Table = toml::from_str(generated).unwrap();
        let listed: Vec<toml::Value> = ONCE.iter().map(|&p| Layout::Gdd.path(p).into()).collect();
        assert_eq!(table["files"], toml::Value::Array(listed));
        let workflow = text_of(".github/workflows/gdd.yml");
        assert!(workflow.starts_with("name: gdd\n"), "{workflow}");
        assert!(workflow.ends_with("\n        run: gdd ci\n"), "{workflow}");
        let models = text_of("gdd/models.toml");
        let models_head = format!("# Models · GDD (written by gdd init {VERSION})\n");
        assert!(models.starts_with(&models_head), "{models}");
        let brief = text_of("docs/brief.md");
        assert!(brief.contains("\n1. Losing tminus notes"), "{brief}");
    }

    #[test]
    fn a_name_with_the_new_name_of_the_tool_stays_as_typed_in_the_old_layout() {
        let old = render_once(&answers_of("tminus"), &[], Layout::Gdd).unwrap();
        let project: toml::Table = toml::from_str(&old[0].1).unwrap();
        assert_eq!(project["name"].as_str(), Some("tminus"));
        let brief = &old[6].1;
        assert!(brief.starts_with("# Project Brief · tminus\n"), "{brief}");
        let generated: toml::Table = toml::from_str(&old[5].1).unwrap();
        let recorded: Answers = generated["answers"].clone().try_into().unwrap();
        assert_eq!(recorded, answers_of("tminus"));
    }

    #[test]
    fn named_agents_count_once_in_their_order_and_none_named_are_those_on_the_path() {
        let named = agents_of(&names(&["codex", "claude", "codex"]), &[]).unwrap();
        assert_eq!(named, ["codex", "claude"]);
        let found = Folder::new();
        for program in ["opencode", "claude"] {
            found.write(program, "#!/bin/sh\n");
            let runnable = Permissions::from_mode(0o755);
            fs::set_permissions(found.0.join(program), runnable).unwrap();
        }
        let path = [("PATH".to_string(), found.0.display().to_string())];
        assert_eq!(agents_of(&[], &path).unwrap(), ["claude", "opencode"]);
        let nowhere = [("PATH".to_string(), "/nonexistent".to_string())];
        assert_eq!(agents_of(&[], &nowhere).unwrap(), Vec::<String>::new());
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
