// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-installer` — CLI entry point.
//
// Thin wrapper over the `vietime_installer` library: parses args, dispatches
// subcommands, and maps failures onto the three exit codes the spec calls
// out (`0` OK / `64` usage / `70` internal).
//
// Week 1 wires up the argument surface (INS-01) and `list` + `version` +
// `hello`. The mutating subcommands (`install`, `uninstall`, `switch`,
// `rollback`, …) print a friendly stub message pointing at the ticket that
// will fill them in; this keeps `--help` honest while the executor is
// under construction.
//
// Spec ref: `spec/02-phase2-installer.md` §A.4 (CLI surface + exit codes).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use vietime_installer::{Combo, TOOL_VERSION};

/// Exit codes per `spec/02-phase2-installer.md` §A.4. `INTERNAL_ERROR` is
/// declared up-front so Week 3's executor (which may actually fail) doesn't
/// have to re-pick a number — kept even though Week 1 never emits it.
mod exit {
    pub const OK: u8 = 0;
    pub const USAGE_ERROR: u8 = 64;
    #[allow(dead_code)] // Wired up when the executor lands (INS-20+).
    pub const INTERNAL_ERROR: u8 = 70;
}

/// One-click installer for Vietnamese input method stacks on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-installer", version, about, long_about = None)]
// Clap-derive naturally produces several `bool` global flags. Grouping them
// into a sub-struct would obscure the clap surface without adding clarity.
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Plan the work but don't mutate anything.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Assume "yes" on every confirmation prompt.
    #[arg(short = 'y', long, global = true)]
    yes: bool,

    /// Extra tracing output on stderr.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Append run logs to `FILE` instead of printing them to stderr.
    #[arg(long, global = true)]
    log_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install a combo (e.g. `fcitx5-bamboo`). Runs the wizard if no combo.
    Install {
        /// Combo slug. See `list` for supported values.
        combo: Option<String>,
    },
    /// Roll back to the state before the most recent install.
    Uninstall,
    /// Switch between combos atomically (backup → install → uninstall old).
    Switch {
        /// Target combo slug.
        combo: String,
    },
    /// Run `vietime-doctor check` against the current state.
    Verify,
    /// Print a one-line status summary (exits 0/1/2 like doctor).
    Status,
    /// List the combos supported by this build.
    List,
    /// Roll back a previous installer run.
    Rollback {
        /// Snapshot id to roll back to. Defaults to the most recent.
        #[arg(long)]
        to: Option<String>,
    },
    /// List snapshots saved by previous installer runs.
    Snapshots,
    /// Pass arguments through to `vietime-doctor`.
    Doctor {
        /// Arguments forwarded verbatim to `vietime-doctor`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Print the installer version.
    Version,
    /// Smoke check — prints the version and `vietime-core` banner.
    Hello,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    dispatch(cli)
}

// Week 1 just prints stub messages per subcommand; every arm is a
// trivial `eprintln!` + exit. Once real executors land in Week 3+ each
// branch will move to its own function and this allow will go away.
#[allow(clippy::too_many_lines)]
fn dispatch(cli: Cli) -> ExitCode {
    // Week 1 doesn't thread the global flags through to an executor yet; name
    // them here so clippy doesn't flag them as unread fields. Real wiring
    // lands with the executor in Week 3 (INS-20+).
    let _ = (cli.dry_run, cli.yes, cli.verbose, &cli.log_file);

    let command = cli.command.unwrap_or(Command::Hello);

    match command {
        Command::Hello => {
            println!("vietime-installer {TOOL_VERSION}");
            println!("{}", vietime_core::hello());
            ExitCode::from(exit::OK)
        }
        Command::Version => {
            println!("{TOOL_VERSION}");
            ExitCode::from(exit::OK)
        }
        Command::List => {
            println!("Supported combos:");
            for combo in Combo::all_supported() {
                println!("  - {}", combo.slug());
            }
            ExitCode::from(exit::OK)
        }
        Command::Install { combo } => {
            if let Some(slug) = combo.as_deref() {
                match slug.parse::<Combo>() {
                    Ok(parsed) => {
                        eprintln!(
                            "vietime-installer: `install {parsed}` is planned but not yet \
                             executed. See tasks/phase-2-installer.md INS-20..INS-30 \
                             (Week 3). Use `--dry-run` once INS-30 lands to preview \
                             the plan."
                        );
                    }
                    Err(err) => {
                        eprintln!("vietime-installer: {err}");
                        return ExitCode::from(exit::USAGE_ERROR);
                    }
                }
            } else {
                eprintln!(
                    "vietime-installer: interactive wizard not yet implemented. \
                     Run `vietime-installer list` to see supported combos, then \
                     call `install <combo>` explicitly. (INS-32, Week 4.)"
                );
            }
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Uninstall => {
            eprintln!(
                "vietime-installer: `uninstall` lands in INS-44 (Week 5). \
                 Use your distro's package manager to remove fcitx5-*/ibus-* \
                 packages in the meantime."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Switch { combo } => {
            eprintln!(
                "vietime-installer: `switch {combo}` lands in INS-60 (Week 7). \
                 For now: run `uninstall` then `install <combo>` once those \
                 subcommands are implemented."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Verify => {
            eprintln!(
                "vietime-installer: `verify` forwards to `vietime-doctor check` \
                 starting in INS-41 (Week 4). Run `vietime-doctor check` directly \
                 in the meantime."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Status => {
            eprintln!(
                "vietime-installer: `status` lands in INS-42 (Week 4). Use \
                 `vietime-doctor report` for a fuller picture right now."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Rollback { to } => {
            let label = to.as_deref().unwrap_or("most-recent");
            eprintln!(
                "vietime-installer: `rollback {label}` lands in INS-13 (Week 2). \
                 Snapshots aren't persisted yet — nothing to roll back to."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Snapshots => {
            eprintln!(
                "vietime-installer: `snapshots` lands in INS-12 (Week 2). \
                 The snapshot store under ~/.local/state/vietime/snapshots \
                 isn't created yet."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Doctor { args } => {
            let quoted = args.join(" ");
            eprintln!(
                "vietime-installer: `doctor` will shell out to vietime-doctor \
                 in INS-41 (Week 4). For now run `vietime-doctor {quoted}` \
                 directly."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
    }
}
