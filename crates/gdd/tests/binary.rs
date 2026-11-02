//! The programs `tminus` and `gdd`: each hands its words and its folder to `gdd::entry`, which
//! runs them as `gdd::run_named` does with the name it was called by, prints what it says and
//! exits with its code.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;

/// The two programs, each with the name it shows.
const PROGRAMS: [(&str, &str); 2] = [
    (env!("CARGO_BIN_EXE_tminus"), "tminus"),
    (env!("CARGO_BIN_EXE_gdd"), "gdd"),
];

/// Runs the built program `binary`, called `name`, with `args` from `cwd` and checks it does
/// what `gdd::run_named` says for that name.
fn assert_like_run(binary: &str, name: &str, args: &[&str], cwd: &Path) {
    let words: Vec<String> = args.iter().map(|word| word.to_string()).collect();
    let expected = gdd::run_named(name, &words, cwd, &gdd::process_env());
    let out = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(i32::from(expected.code)),
        "{name} {args:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected.stdout,
        "{name} {args:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        expected.stderr,
        "{name} {args:?}"
    );
}

#[test]
fn the_program_prints_what_run_says_and_exits_with_its_code() {
    let folder = std::env::temp_dir().join(format!("gdd-binary-test-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    // The program sees its folder with the links resolved.
    let folder = folder.canonicalize().unwrap();
    for (binary, name) in PROGRAMS {
        assert_like_run(binary, name, &["--version"], &folder);
        assert_like_run(binary, name, &["frobnicate"], &folder);
        // Outside a project the error names the folder the program runs from.
        assert_like_run(binary, name, &["report"], &folder);
    }
    fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn each_program_says_its_own_name_with_its_version() {
    for (binary, name) in PROGRAMS {
        let out = Command::new(binary).arg("--version").output().unwrap();
        let version = format!("{name} {}\n", env!("CARGO_PKG_VERSION"));
        assert_eq!(String::from_utf8_lossy(&out.stdout), version);
    }
}

#[test]
fn an_agent_named_in_its_role_variable_records_no_gate() {
    let folder = std::env::temp_dir().join(format!("gdd-binary-role-{}", std::process::id()));
    fs::create_dir_all(folder.join("gdd")).unwrap();
    fs::write(folder.join("gdd/project.toml"), "name = \"demo\"\n").unwrap();
    // What the program says to `gate G0 ok --by machines` with a role set, or none.
    let gate = |variable: &str, role: Option<&OsStr>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
        command.args(["gate", "G0", "ok", "--by", "machines"]);
        command.env_remove("GDD_ROLE").env_remove("TMINUS_ROLE");
        if let Some(role) = role {
            command.env(variable, role);
        }
        let out = command.current_dir(&folder).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{variable} {role:?}");
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    let not_for_agents = "error: Agents that tminus started cannot run tminus gate: they may only \
                          run tminus g2, tminus guard, tminus note, tminus status, tminus \
                          classify, tminus inbox, tminus report and tminus scope.\n";
    for variable in ["GDD_ROLE", "TMINUS_ROLE"] {
        let refused = |role: &[u8]| gate(variable, Some(OsStr::from_bytes(role)));
        assert_eq!(refused(b"reviewer"), not_for_agents);
        assert_eq!(refused(b"builder-test"), not_for_agents);
        assert_eq!(refused(b"orchestrator"), not_for_agents);
        // A role that is not UTF-8 still names an agent.
        assert_eq!(refused(b"\xff\xfe"), not_for_agents);
        assert_eq!(refused(b"builder-\xff"), not_for_agents);
    }
    let untested = "error: G0 cannot open: the harness self-test is not 4/4 (tminus selftest).\n";
    assert_eq!(gate("GDD_ROLE", None), untested);
    fs::remove_dir_all(&folder).unwrap();
}
