// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-bench` — compatibility matrix runner for Vietnamese IMEs.
//
// P0-11 skeleton. Real commands ship in Phase 3 (`tasks/phase-3-bench.md`).
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.4.

use clap::{Parser, Subcommand};

/// Compatibility matrix benchmark runner for Vietnamese IMEs on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-bench", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a matrix of combos over test vectors.
    Run,
    /// List available engines / apps / modes.
    List,
    /// Render a report from `runs/<id>/`.
    Report,
    /// Compare two runs.
    Compare,
    /// Print which core library this build bundles.
    Hello,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Hello) {
        Command::Hello => {
            println!("vietime-bench {}", env!("CARGO_PKG_VERSION"));
            println!("{}", vietime_core::hello());
        }
        Command::Run | Command::List | Command::Report | Command::Compare => {
            eprintln!(
                "vietime-bench: this subcommand is not implemented yet. \
                 See tasks/phase-3-bench.md."
            );
            std::process::exit(2);
        }
    }
}
