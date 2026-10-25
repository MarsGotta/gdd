fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", gdd_core::version_line());
        return;
    }
    println!(
        "{}: under construction. See https://github.com/MarsGotta/gdd",
        gdd_core::version_line()
    );
}
