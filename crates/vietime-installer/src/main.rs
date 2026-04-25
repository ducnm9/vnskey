// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-installer` — one-click installer for Vietnamese IME stacks.
//
// P0-11 skeleton. Real commands ship in Phase 2 (`tasks/phase-2-installer.md`).
//
// Spec ref: `spec/02-phase2-installer.md` §A.4.

use clap::{Parser, Subcommand};

/// One-click installer for Vietnamese input method stacks on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-installer", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install a combo (e.g. `fcitx5-bamboo`).
    Install,
    /// Uninstall the most recent install.
    Uninstall,
    /// Switch combos atomically.
    Switch,
    /// Print current installation status.
    Status,
    /// Print which core library this build bundles.
    Hello,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Hello) {
        Command::Hello => {
            println!("vietime-installer {}", env!("CARGO_PKG_VERSION"));
            println!("{}", vietime_core::hello());
        }
        Command::Install | Command::Uninstall | Command::Switch | Command::Status => {
            eprintln!(
                "vietime-installer: this subcommand is not implemented yet. \
                 See tasks/phase-2-installer.md."
            );
            std::process::exit(2);
        }
    }
}
