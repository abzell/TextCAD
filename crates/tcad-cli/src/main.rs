//! The `tcad` command-line tool. Subcommands arrive with later milestones.

fn main() {
    println!(
        "{} {} (no commands implemented yet)",
        tcad_cli::FORMAT_NAME,
        env!("CARGO_PKG_VERSION")
    );
}
