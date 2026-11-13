//! The command line: version, help, and the self-update command. Handled before `detach`, so
//! `marko update` and `marko --version` print in the terminal instead of giving the prompt back.

use crate::update;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Version,
    Help,
    Update { check: bool },
}

/// The command `args` asks for (without the program name). `None` opens the GUI.
pub fn command(args: &[String]) -> Option<Command> {
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return Some(Command::Version);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Some(Command::Help);
    }
    match args.first().map(String::as_str) {
        Some("update" | "upgrade") => {
            Some(Command::Update { check: args[1..].iter().any(|a| a == "--check" || a == "-n") })
        }
        _ => None,
    }
}

/// Runs the command, if any. Returns the process exit code.
pub fn run() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match command(&args)? {
        Command::Version => {
            println!("marko {}", env!("CARGO_PKG_VERSION"));
            Some(0)
        }
        Command::Help => {
            print_help();
            Some(0)
        }
        Command::Update { check } => Some(update::run(check)),
    }
}

fn print_help() {
    println!(
        "marko — a fast, light Markdown reader and editor.\n\
         \n\
         Usage:\n\
         \x20 marko [FILE...]           open the reader\n\
         \x20 marko --wait [FILE...]    keep the terminal attached\n\
         \x20 marko --version           print the version\n\
         \x20 marko update [--check]    install the latest release (or only check)\n\
         \n\
         Files and folders open as tabs and sidebar roots."
    );
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
