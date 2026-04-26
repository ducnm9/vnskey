// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-installer` — CLI entry point.
//
// Thin wrapper over the `vietime_installer` library: parses args,
// dispatches subcommands, and maps failures onto the three exit codes
// the spec calls out (`0` OK / `64` usage / `70` internal).
//
// Wired subcommands (Phase 2):
//
//   install     — plan + execute a Combo install.
//   uninstall   — walk back the latest snapshot (alias for rollback).
//   switch      — uninstall the active combo then install the new one.
//   verify      — shell out to `vietime-doctor check`.
//   status      — one-line summary of which combo is active.
//   list        — enumerate supported combos.
//   rollback    — undo a specific snapshot by id.
//   snapshots   — list snapshot manifests on disk.
//   doctor      — forward args to `vietime-doctor`.
//   version     — print the tool version.
//   hello       — smoke check.
//
// Spec ref: `spec/02-phase2-installer.md` §A.4.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use vietime_installer::executor::{self, ExecConfig, ExecReporter, Mode, StderrReporter};
use vietime_installer::snapshot::SnapshotStore;
use vietime_installer::{detect_pre_state, plan, Combo, Goal, TOOL_VERSION};

mod exit {
    pub const OK: u8 = 0;
    pub const USAGE_ERROR: u8 = 64;
    pub const INTERNAL_ERROR: u8 = 70;
}

/// One-click installer for Vietnamese input method stacks on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-installer", version, about, long_about = None)]
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
    /// Switch between combos atomically (uninstall old → install new).
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
        /// Roll back even if the manifest is flagged `incomplete = true`.
        #[arg(long)]
        force: bool,
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
    // Construct a tokio runtime lazily: `version` / `list` / `hello` don't
    // need one, but every mutating command does.
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("vietime-installer: cannot build tokio runtime: {err}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };
    runtime.block_on(dispatch(cli))
}

async fn dispatch(cli: Cli) -> ExitCode {
    let command = cli.command.unwrap_or(Command::Hello);
    let mode = if cli.dry_run { Mode::DryRun } else { Mode::Live };
    let config = ExecConfig::new(mode).with_assume_yes(cli.yes);
    let reporter: Arc<dyn ExecReporter> = Arc::new(StderrReporter);

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
        Command::Install { combo } => cmd_install(combo, &config, reporter).await,
        Command::Uninstall => cmd_rollback(None, false, &config, reporter).await,
        Command::Switch { combo } => cmd_switch(&combo, &config, reporter).await,
        Command::Verify => cmd_verify().await,
        Command::Status => cmd_status(&config),
        Command::Rollback { to, force } => cmd_rollback(to, force, &config, reporter).await,
        Command::Snapshots => cmd_snapshots(&config),
        Command::Doctor { args } => cmd_doctor(&args).await,
    }
}

// ─── install ──────────────────────────────────────────────────────────────

async fn cmd_install(
    combo: Option<String>,
    config: &ExecConfig,
    reporter: Arc<dyn ExecReporter>,
) -> ExitCode {
    let combo = match combo {
        Some(slug) => match slug.parse::<Combo>() {
            Ok(c) => c,
            Err(err) => {
                eprintln!("vietime-installer: {err}");
                return ExitCode::from(exit::USAGE_ERROR);
            }
        },
        None => match run_wizard() {
            Ok(c) => c,
            Err(code) => return ExitCode::from(code),
        },
    };

    let pre = detect_pre_state().await;

    let plan = match plan(pre, Goal::Install { combo }) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("vietime-installer: cannot plan install: {err}");
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };

    eprintln!(
        "Planned {n} steps for `install {combo}` (requires_sudo={s}, requires_logout={l})",
        n = plan.steps.len(),
        s = plan.requires_sudo,
        l = plan.requires_logout,
    );

    match executor::run_plan(plan, config, reporter).await {
        Ok(outcome) => {
            eprintln!(
                "Success: {n} steps executed. Snapshot id: {id}",
                n = outcome.steps_executed,
                id = outcome.snapshot_id,
            );
            if !outcome.dry_run {
                eprintln!("Run `vietime-installer rollback --to {}` to undo.", outcome.snapshot_id);
            }
            ExitCode::from(exit::OK)
        }
        Err(err) => {
            eprintln!("vietime-installer: install failed: {err}");
            ExitCode::from(exit::INTERNAL_ERROR)
        }
    }
}

/// Very small selection TUI — matches the spec's INS-32 wizard: prints
/// the four combos and asks the user to pick one. `--yes` bypasses this.
fn run_wizard() -> Result<Combo, u8> {
    let combos = Combo::all_supported();
    eprintln!("Pick a combo to install (you can also pass the slug directly):");
    for (i, c) in combos.iter().enumerate() {
        eprintln!("  [{i}] {}", c.slug());
    }
    let selection = dialoguer::Select::new()
        .with_prompt("Combo")
        .items(&combos.iter().map(|c| c.slug()).collect::<Vec<_>>())
        .default(0)
        .interact()
        .map_err(|err| {
            eprintln!("vietime-installer: wizard aborted: {err}");
            exit::USAGE_ERROR
        })?;
    combos.get(selection).copied().ok_or_else(|| {
        eprintln!("vietime-installer: invalid wizard selection");
        exit::USAGE_ERROR
    })
}

// ─── rollback / uninstall ────────────────────────────────────────────────

async fn cmd_rollback(
    to: Option<String>,
    force: bool,
    config: &ExecConfig,
    reporter: Arc<dyn ExecReporter>,
) -> ExitCode {
    let store = SnapshotStore::new(config.snapshots_root.clone());
    let Some(id) = to.or_else(|| store.latest_id()) else {
        eprintln!(
            "vietime-installer: no snapshots found under {}",
            config.snapshots_root.display()
        );
        return ExitCode::from(exit::USAGE_ERROR);
    };
    let handle = match store.load(&id) {
        Ok(h) => h,
        Err(err) => {
            eprintln!("vietime-installer: cannot load snapshot `{id}`: {err}");
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };
    if handle.manifest().incomplete && !force {
        eprintln!(
            "vietime-installer: snapshot `{id}` is flagged incomplete — \
             the previous run was killed mid-flight. Re-run with `--force` \
             to roll back anyway.",
        );
        return ExitCode::from(exit::USAGE_ERROR);
    }

    eprintln!(
        "Rolling back snapshot `{id}` ({n} artifacts) …",
        n = handle.manifest().artifacts.len()
    );
    executor::rollback_from_handle(&handle, reporter.as_ref(), config).await;
    eprintln!("Rollback complete.");
    ExitCode::from(exit::OK)
}

// ─── switch ───────────────────────────────────────────────────────────────

async fn cmd_switch(
    combo_slug: &str,
    config: &ExecConfig,
    reporter: Arc<dyn ExecReporter>,
) -> ExitCode {
    let target = match combo_slug.parse::<Combo>() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("vietime-installer: {err}");
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };
    // Uninstall-then-install = rollback latest + install target.
    let store = SnapshotStore::new(config.snapshots_root.clone());
    if let Some(id) = store.latest_id() {
        eprintln!("Rolling back current snapshot `{id}` …");
        if let Ok(handle) = store.load(&id) {
            executor::rollback_from_handle(&handle, reporter.as_ref(), config).await;
        }
    }
    cmd_install(Some(target.slug()), config, reporter).await
}

// ─── verify ───────────────────────────────────────────────────────────────

async fn cmd_verify() -> ExitCode {
    match tokio::process::Command::new("vietime-doctor").arg("check").status().await {
        Ok(s) if s.success() => ExitCode::from(exit::OK),
        Ok(s) => exit_from_status(s.code()),
        Err(err) => {
            eprintln!("vietime-installer: cannot exec vietime-doctor: {err}");
            ExitCode::from(exit::INTERNAL_ERROR)
        }
    }
}

/// Coerce a child-process exit code into our `ExitCode`. Negative / > 255
/// codes get mapped onto `INTERNAL_ERROR` so we never panic on weird
/// child-process exits (e.g. a signal-terminated doctor).
fn exit_from_status(code: Option<i32>) -> ExitCode {
    let raw = code.unwrap_or(i32::from(exit::INTERNAL_ERROR));
    ExitCode::from(u8::try_from(raw).unwrap_or(exit::INTERNAL_ERROR))
}

// ─── status ───────────────────────────────────────────────────────────────

fn cmd_status(config: &ExecConfig) -> ExitCode {
    let store = SnapshotStore::new(config.snapshots_root.clone());
    if let Some(handle) = store.latest_id().and_then(|id| store.load(&id).ok()) {
        let m = handle.manifest();
        println!(
            "active snapshot: {id}\ngoal: {goal}\ncreated: {created}\ncomplete: {ok}",
            id = m.id,
            goal = m.goal_summary(),
            created = m.created_at,
            ok = !m.incomplete,
        );
    } else {
        println!("no snapshots found — nothing installed by vietime");
    }
    ExitCode::from(exit::OK)
}

// ─── snapshots ────────────────────────────────────────────────────────────

fn cmd_snapshots(config: &ExecConfig) -> ExitCode {
    let store = SnapshotStore::new(config.snapshots_root.clone());
    match store.list() {
        Ok(rows) if rows.is_empty() => {
            println!("no snapshots under {}", config.snapshots_root.display());
            ExitCode::from(exit::OK)
        }
        Ok(rows) => {
            println!("{:<24} {:<12} goal", "id", "complete");
            for row in rows {
                let state = if row.incomplete { "incomplete" } else { "ok" };
                println!("{:<24} {:<12} {}", row.id, state, row.goal);
            }
            ExitCode::from(exit::OK)
        }
        Err(err) => {
            eprintln!("vietime-installer: cannot list snapshots: {err}");
            ExitCode::from(exit::INTERNAL_ERROR)
        }
    }
}

// ─── doctor ───────────────────────────────────────────────────────────────

async fn cmd_doctor(args: &[String]) -> ExitCode {
    match tokio::process::Command::new("vietime-doctor").args(args).status().await {
        Ok(s) => exit_from_status(s.code()),
        Err(err) => {
            eprintln!("vietime-installer: cannot exec vietime-doctor: {err}");
            ExitCode::from(exit::INTERNAL_ERROR)
        }
    }
}
