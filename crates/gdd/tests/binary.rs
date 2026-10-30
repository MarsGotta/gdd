//! The `gdd` program: `main` hands its words and its folder to `gdd::run`, prints what it says
//! and exits with its code.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;

/// Runs the built program with `args` from `cwd` and checks it does what `gdd::run` says.
fn assert_like_run(args: &[&str], cwd: &Path) {
    let words: Vec<String> = args.iter().map(|word| word.to_string()).collect();
    let expected = gdd::run(&words, cwd);
    let out = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(i32::from(expected.code)),
        "{args:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        expected.stdout,
        "{args:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        expected.stderr,
        "{args:?}"
    );
}

#[test]
fn the_program_prints_what_run_says_and_exits_with_its_code() {
    let folder = std::env::temp_dir().join(format!("gdd-binary-test-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    // The program sees its folder with the links resolved.
    let folder = folder.canonicalize().unwrap();
    assert_like_run(&["--version"], &folder);
    assert_like_run(&["frobnicate"], &folder);
    // Outside a project the error names the folder the program runs from.
    assert_like_run(&["report"], &folder);
    fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn an_agent_named_in_gdd_role_records_no_gate() {
    let folder = std::env::temp_dir().join(format!("gdd-binary-role-{}", std::process::id()));
    fs::create_dir_all(folder.join("gdd")).unwrap();
    fs::write(folder.join("gdd/project.toml"), "name = \"demo\"\n").unwrap();
    // What the program says to `gdd gate G0 ok --by machines` with GDD_ROLE set, or unset.
    let gate = |role: Option<&OsStr>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gdd"));
        command.args(["gate", "G0", "ok", "--by", "machines"]);
        match role {
            Some(role) => command.env("GDD_ROLE", role),
            None => command.env_remove("GDD_ROLE"),
        };
        let out = command.current_dir(&folder).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{role:?}");
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    let refused = "error: A Builder or Reviewer cannot open or close gates.\n";
    assert_eq!(gate(Some(OsStr::new("reviewer"))), refused);
    assert_eq!(gate(Some(OsStr::new("builder-test"))), refused);
    let not_for_agents = "error: Builders and Reviewers cannot run gdd gate: an agent gdd started \
                          may only run gdd g2, gdd guard, gdd note, gdd status, gdd classify, gdd \
                          inbox, gdd report and gdd scope.\n";
    assert_eq!(gate(Some(OsStr::new("orchestrator"))), not_for_agents);
    // A role that is not UTF-8 still names an agent.
    assert_eq!(gate(Some(OsStr::from_bytes(b"\xff\xfe"))), not_for_agents);
    assert_eq!(gate(Some(OsStr::from_bytes(b"builder-\xff"))), refused);
    let untested = "error: G0 cannot open: the harness self-test is not 4/4 (gdd selftest).\n";
    assert_eq!(gate(None), untested);
    fs::remove_dir_all(&folder).unwrap();
}
