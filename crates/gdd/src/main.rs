use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let out = gdd::run(&args, &cwd);
    // A closed output (`gdd report | head -1`) is not worth a panic.
    let mut stdout = std::io::stdout();
    let _ = stdout
        .write_all(out.stdout.as_bytes())
        .and_then(|()| stdout.flush());
    let _ = std::io::stderr().write_all(out.stderr.as_bytes());
    ExitCode::from(out.code)
}
