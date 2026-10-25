//! The `gdd` program: `main` hands its words and its folder to `gdd::run`, prints what it says
//! and exits with its code.

use std::fs;
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
