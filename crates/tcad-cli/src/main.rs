//! The `tcad` command-line tool.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = tcad_cli::run(&args, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
