// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-bench` — CLI entry point.
//
// Thin wrapper over the `vietime_bench` library: parses args, dispatches
// subcommands, and maps failures onto the three exit codes the spec calls
// out (`0` OK / `64` usage / `70` internal).
//
// Week 1 wires up the argument surface (BEN-01) and `list` + `version` +
// `hello`. The mutating / long-running subcommands (`run`, `report`,
// `compare`, `validate`, `inspect`) print a friendly stub message pointing
// at the ticket that will fill them in; this keeps `--help` honest while
// the matrix runner is under construction.
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.4.

use std::process::ExitCode;

use clap::Parser;

use vietime_bench::{Cli, Command, InputMode, TOOL_VERSION};
use vietime_installer::Combo;

/// Exit codes mirrored from doctor / installer. `INTERNAL_ERROR` is declared
/// up-front so the matrix runner arriving in BEN-14 doesn't have to re-pick
/// a number — kept even though Week 1 never emits it.
mod exit {
    pub const OK: u8 = 0;
    pub const USAGE_ERROR: u8 = 64;
    #[allow(dead_code)] // Wired up when the matrix runner lands (BEN-14).
    pub const INTERNAL_ERROR: u8 = 70;
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    dispatch(cli)
}

// Week 1 just prints stub messages for most subcommands; every arm is a
// trivial `eprintln!` + exit. Once real executors land in Week 2+ each branch
// will move to its own function and this allow will go away.
#[allow(clippy::too_many_lines)]
fn dispatch(cli: Cli) -> ExitCode {
    // The global flags aren't threaded through to an executor yet; name them
    // here so clippy doesn't flag them as unread fields. Real wiring lands
    // with the matrix runner in Week 2 (BEN-14).
    let _ =
        (&cli.profile, &cli.engine, &cli.app, &cli.mode, &cli.session, cli.verbose, &cli.runs_dir);

    let command = cli.command.unwrap_or(Command::Hello);

    match command {
        Command::Hello => {
            println!("vietime-bench {TOOL_VERSION}");
            println!("{}", vietime_core::hello());
            ExitCode::from(exit::OK)
        }
        Command::Version => {
            println!("{TOOL_VERSION}");
            ExitCode::from(exit::OK)
        }
        Command::List => {
            print_list();
            ExitCode::from(exit::OK)
        }
        Command::Run => {
            // Validate `--engine` now so the stub surface still catches obvious
            // typos instead of only flagging them in Week 2.
            if let Some(slug) = cli.engine.as_deref() {
                if let Err(err) = slug.parse::<Combo>() {
                    eprintln!("vietime-bench: {err}");
                    return ExitCode::from(exit::USAGE_ERROR);
                }
            }
            if let Some(mode) = cli.mode.as_deref() {
                if let Err(err) = mode.parse::<InputMode>() {
                    eprintln!("vietime-bench: {err}");
                    return ExitCode::from(exit::USAGE_ERROR);
                }
            }
            eprintln!(
                "vietime-bench: `run` lands in BEN-14 (Week 2). The matrix \
                 orchestrator, test-vector loader, and scoring core all \
                 arrive together. Use `list` to see the shape of the matrix \
                 in the meantime."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Report { run_id, format } => {
            let run_label = run_id.as_deref().unwrap_or("most-recent");
            let fmt_label = format.as_deref().unwrap_or("markdown");
            eprintln!(
                "vietime-bench: `report {run_label} --format {fmt_label}` lands \
                 in BEN-61 (Week 7). Nothing has been written to the runs \
                 directory yet."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Compare { base, head } => {
            eprintln!(
                "vietime-bench: `compare --base {base} --head {head}` lands in \
                 BEN-62 (Week 7). Diffing needs the scoring core from BEN-13 \
                 first."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Validate { path } => {
            let where_ = path
                .as_deref()
                .map_or_else(|| "test-vectors/".to_owned(), |p| p.display().to_string());
            eprintln!(
                "vietime-bench: `validate {where_}` lands in BEN-12 (Week 2). \
                 The test-vector TOML schema isn't frozen yet."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Inspect { run_id, vector_id } => {
            eprintln!(
                "vietime-bench: `inspect {run_id} {vector_id}` lands in BEN-63 \
                 (Week 7). Needs stored per-vector traces that BEN-14 emits."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
    }
}

/// Print the `list` output. Combos are pulled straight from `vietime-installer`
/// so the two CLIs never drift. Modes / sessions / backends are hard-coded
/// because they're part of the bench's own surface area.
fn print_list() {
    println!("Supported sessions:");
    println!("  - x11");
    println!("  - wayland");

    println!();
    println!("Supported input modes:");
    for m in InputMode::all() {
        println!("  - {m}");
    }

    println!();
    println!("Supported combos:");
    for combo in Combo::all_supported() {
        println!("  - {}", combo.slug());
    }

    println!();
    println!("Registered session drivers:");
    println!("  - xvfb (X11, Week 1)");
    println!("  - weston (Wayland, Week 4 — BEN-30, stub)");

    println!();
    println!("Registered keystroke injectors:");
    println!("  - xdotool (X11, Week 1)");
    println!("  - ydotool (Wayland, Week 4 — BEN-31, stub)");
}
