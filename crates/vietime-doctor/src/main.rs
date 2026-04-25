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
use vietime_doctor::detectors::{DesktopDetector, DistroDetector, SessionDetector};
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
                match serde_json::to_string_pretty(&report).context("serialise report as JSON") {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("vietime-doctor: {err:?}");
                        return ExitCode::from(exit::INTERNAL_ERROR);
                    }
                }
            } else {
                render_plain(&report, cli.verbose)
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
    ];
    for d in detectors {
        orch.add(d);
    }
    orch
}

/// Minimal plain-text rendering used until the `minijinja` renderer lands
/// in Week 2 (DOC-14). Keeping the logic small means the Report data model
/// can evolve without fighting a template.
fn render_plain(report: &vietime_core::Report, verbose: bool) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "# VietIME Doctor Report\nGenerated: {}\nvietime-doctor v{}\n",
        report.generated_at.to_rfc3339(),
        report.tool_version
    );

    let _ = writeln!(out, "## Environment");
    if let Some(d) = &report.facts.system.distro {
        let pretty = d
            .pretty
            .clone()
            .unwrap_or_else(|| format!("{} {}", d.id, d.version_id.clone().unwrap_or_default()));
        let _ = writeln!(out, "- Distro: {pretty}");
    }
    if let Some(de) = &report.facts.system.desktop {
        let _ = writeln!(out, "- Desktop: {}", de.display_name());
    }
    if let Some(s) = report.facts.system.session {
        let _ = writeln!(out, "- Session: {}", s.as_str());
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## IM Framework");
    let _ = writeln!(out, "- Active: {:?}", report.facts.im.active_framework);
    let _ = writeln!(out);

    if !report.anomalies.is_empty() {
        let _ = writeln!(out, "## Detector anomalies");
        for a in &report.anomalies {
            let _ = writeln!(out, "- {}: {}", a.detector, a.reason);
        }
        let _ = writeln!(out);
    }

    if verbose {
        let _ = writeln!(
            out,
            "(verbose) schema_version={}, issues={}, recommendations={}",
            report.schema_version,
            report.issues.len(),
            report.recommendations.len()
        );
    }

    out
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("VIETIME_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt().with_env_filter(filter).with_writer(std::io::stderr).try_init();
}
