// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-doctor` — CLI entry point.
//
// Thin wrapper over the `vietime_doctor` library: parses args, builds the
// orchestrator with the detectors available in this phase, runs it, prints
// the report in the requested format, and propagates the per-spec exit code.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.4 (CLI surface, exit codes).

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing::info;

use vietime_doctor::detector::{Detector, DetectorContext};
use vietime_doctor::detectors::{
    DesktopDetector, DistroDetector, EtcEnvironmentDetector, EtcProfileDDetector,
    HomeProfileDetector, ProcessEnvDetector, SessionDetector, SystemdEnvDetector,
};
use vietime_doctor::render::{render, render_json, RenderOptions};
use vietime_doctor::{Orchestrator, OrchestratorConfig};

/// Exit codes per `spec/01` §A.4. Non-zero severity codes (1 / 2) flow
/// directly from `Report::exit_code()` so we don't redefine them here.
mod exit {
    pub const OK: u8 = 0;
    pub const USAGE_ERROR: u8 = 64;
    pub const INTERNAL_ERROR: u8 = 70;
}

/// Diagnose Vietnamese input method setup on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-doctor", version, about, long_about = None)]
// Clap derives many bool flags; splitting them into a sub-struct would
// obscure the clap-derive surface without adding clarity.
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Output format: JSON (for maintainers / CI parsing).
    #[arg(long, global = true, conflicts_with = "plain")]
    json: bool,

    /// Output format: plain text (no markdown formatting).
    #[arg(long, global = true)]
    plain: bool,

    /// Include extra detector notes.
    #[arg(long, global = true, short)]
    verbose: bool,

    /// Disable PII redaction. Off by default; `--no-redact` prints raw data
    /// — useful only when debugging Doctor itself.
    #[arg(long = "no-redact", global = true)]
    no_redact: bool,

    /// Focus on a specific app (e.g. `vscode`, `chrome`). Not yet wired in
    /// Phase 1 Week 1.
    #[arg(long, global = true)]
    app: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run every detector and render the full report (default).
    Report {
        /// Write the report to `FILE` instead of stdout.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
    /// Run every check and print a 1-line status; exits 0/1/2.
    Check,
    /// List all detectors / checkers registered in this build.
    List,
    /// Run only a subset of detectors: `env`, `daemon`, `sys`.
    Diagnose {
        #[arg(value_parser = ["env", "daemon", "sys"])]
        topic: String,
    },
    /// Print `vietime-core ready` — retained from the Phase 0 smoke test
    /// so existing test harnesses keep working. Removed in v0.2.
    Hello,
}

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("initialise tokio runtime")
    {
        Ok(r) => r,
        Err(err) => {
            eprintln!("vietime-doctor: {err:?}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };

    runtime.block_on(async { dispatch(cli).await })
}

async fn dispatch(cli: Cli) -> ExitCode {
    let command = cli.command.unwrap_or(Command::Report { output: None });

    match command {
        Command::Hello => {
            println!("vietime-doctor {}", env!("CARGO_PKG_VERSION"));
            println!("{}", vietime_core::hello());
            ExitCode::from(exit::OK)
        }
        Command::List => {
            // Detector listing — Checker listing lands with the checker
            // engine in Week 5.
            let orch = build_orchestrator();
            println!("Detectors:");
            for d in orch.detectors() {
                println!("  - {}", d.id());
            }
            println!();
            println!("Checkers: (none in this build — Week 5)");
            ExitCode::from(exit::OK)
        }
        Command::Diagnose { topic } => {
            eprintln!(
                "vietime-doctor: `diagnose {topic}` lands in Phase 1 Week 2. \
                 Running full `report` in the meantime may give you what you need."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Check => {
            let orch = build_orchestrator();
            let ctx = DetectorContext::from_current_process();
            let report = orch.run(&ctx).await;
            // No checkers yet — the check subcommand is effectively a
            // smoke test that the orchestrator ran at all. Actual checker
            // engine ships in Week 5.
            let status = match report.exit_code() {
                0 => "ok",
                1 => "warn",
                _ => "error",
            };
            println!(
                "vietime-doctor: {status} ({} detectors, {} anomalies)",
                orch.detectors().len(),
                report.anomalies.len()
            );
            ExitCode::from(u8::try_from(report.exit_code().max(0)).unwrap_or(exit::INTERNAL_ERROR))
        }
        Command::Report { output } => {
            let orch = build_orchestrator();
            let ctx = DetectorContext::from_current_process();
            info!(detectors = orch.detectors().len(), "running detectors");
            let report = orch.run(&ctx).await;

            let rendered = if cli.json {
                match render_json(&report).context("serialise report as JSON") {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("vietime-doctor: {err:?}");
                        return ExitCode::from(exit::INTERNAL_ERROR);
                    }
                }
            } else {
                match render(&report, &RenderOptions { plain: cli.plain, verbose: cli.verbose })
                    .context("render report")
                {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("vietime-doctor: {err:?}");
                        return ExitCode::from(exit::INTERNAL_ERROR);
                    }
                }
            };

            let write_res = if let Some(path) = output {
                std::fs::write(&path, &rendered)
                    .with_context(|| format!("write report to {}", path.display()))
            } else {
                println!("{rendered}");
                Ok(())
            };
            if let Err(err) = write_res {
                eprintln!("vietime-doctor: {err:?}");
                return ExitCode::from(exit::INTERNAL_ERROR);
            }

            ExitCode::from(u8::try_from(report.exit_code().max(0)).unwrap_or(exit::INTERNAL_ERROR))
        }
    }
}

fn build_orchestrator() -> Orchestrator {
    let mut orch = Orchestrator::new(OrchestratorConfig::default());
    let detectors: Vec<Arc<dyn Detector>> = vec![
        Arc::new(DistroDetector::new()),
        Arc::new(SessionDetector::new()),
        Arc::new(DesktopDetector::new()),
        Arc::new(ProcessEnvDetector::new()),
        Arc::new(EtcEnvironmentDetector::new()),
        Arc::new(HomeProfileDetector::new()),
        Arc::new(EtcProfileDDetector::new()),
        Arc::new(SystemdEnvDetector::new()),
    ];
    for d in detectors {
        orch.add(d);
    }
    orch
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("VIETIME_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt().with_env_filter(filter).with_writer(std::io::stderr).try_init();
}
