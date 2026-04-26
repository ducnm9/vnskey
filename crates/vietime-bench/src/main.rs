// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-bench` — CLI entry point.
//
// Thin wrapper over the `vietime_bench` library: parses args, dispatches
// subcommands, and maps failures onto the three exit codes the spec calls
// out (`0` OK / `64` usage / `70` internal).
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.4.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use vietime_bench::{Cli, Command, InputMode, TOOL_VERSION};
use vietime_bench::runner::{self, RunCombo, RunResult};
use vietime_bench::vector;
use vietime_installer::Combo;

mod exit {
    pub const OK: u8 = 0;
    pub const USAGE_ERROR: u8 = 64;
    pub const INTERNAL_ERROR: u8 = 70;
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    dispatch(cli)
}

#[allow(clippy::too_many_lines)]
fn dispatch(cli: Cli) -> ExitCode {
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

            let vectors_dir = PathBuf::from("test-vectors");
            if !vectors_dir.is_dir() {
                eprintln!(
                    "vietime-bench: test-vectors/ directory not found. \
                     Create it with at least one .toml file."
                );
                return ExitCode::from(exit::USAGE_ERROR);
            }

            let vectors = match vector::load_vectors_from_dir(&vectors_dir) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("vietime-bench: failed to load test vectors: {e}");
                    return ExitCode::from(exit::INTERNAL_ERROR);
                }
            };

            if vectors.is_empty() {
                eprintln!("vietime-bench: no test vectors found in test-vectors/");
                return ExitCode::from(exit::USAGE_ERROR);
            }

            if let Err(e) = vector::validate_vectors(&vectors) {
                eprintln!("vietime-bench: {e}");
                return ExitCode::from(exit::USAGE_ERROR);
            }

            // Build the runtime and dispatch to the async runner.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("vietime-bench: failed to start async runtime: {e}");
                    return ExitCode::from(exit::INTERNAL_ERROR);
                }
            };

            let runs_dir = cli.runs_dir.unwrap_or_else(|| PathBuf::from("runs"));

            let engine_slug = cli.engine.unwrap_or_else(|| "ibus-bamboo".to_owned());
            let mode: InputMode = cli
                .mode
                .as_deref()
                .unwrap_or("telex")
                .parse()
                .unwrap_or(InputMode::Telex);
            let app_id = cli.app.unwrap_or_else(|| "gedit".to_owned());
            let session_str = cli.session.as_deref().unwrap_or("x11");

            let session_type = match session_str {
                "wayland" => vietime_core::SessionType::Wayland,
                _ => vietime_core::SessionType::X11,
            };

            let combo = RunCombo {
                engine: engine_slug,
                app_id: app_id.clone(),
                session_type,
                mode,
            };

            println!(
                "vietime-bench: running {} vectors — engine={}, app={}, session={}, mode={}",
                vectors.len(),
                combo.engine,
                combo.app_id,
                session_type.as_str(),
                mode,
            );

            let result = rt.block_on(async {
                let mut session_driver = vietime_bench::session::xvfb::XvfbDriver::new();
                let mut im_driver = vietime_bench::im_driver::ibus::IbusDriver::new();
                let mut app_runner = vietime_bench::app_runner::gedit::GeditRunner::new();
                let injector = vietime_bench::injector::xdotool::XdotoolInjector::new(
                    session_driver.display(),
                );

                let combo_result = runner::run_combo(
                    &mut session_driver,
                    &mut im_driver,
                    &mut app_runner,
                    &injector,
                    &vectors,
                    &combo,
                )
                .await?;

                let mut run_result = RunResult::new_empty();
                run_result.finished_at = chrono::Utc::now();
                run_result.matrix.push(combo_result);

                runner::save_run_result(&run_result, &runs_dir)?;

                Ok::<RunResult, runner::RunError>(run_result)
            });

            match result {
                Ok(r) => {
                    let combo = &r.matrix[0];
                    println!();
                    println!("=== Results ===");
                    println!("Run ID:    {}", r.run_id);
                    println!("Accuracy:  {:.1}%", combo.score.accuracy_pct);
                    println!(
                        "Exact:     {}/{}",
                        combo.score.exact_match_count, combo.score.vectors_tested
                    );
                    println!("Edit dist: {}", combo.score.edit_distance_total);
                    println!("Duration:  {}ms", combo.duration_ms);
                    if !combo.failures.is_empty() {
                        println!("Failures:  {}", combo.failures.len());
                        for f in &combo.failures {
                            println!(
                                "  {} — expected {:?}, got {:?} (dist={})",
                                f.vector_id, f.expected, f.actual, f.edit_distance
                            );
                        }
                    }
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("vietime-bench: run failed: {e}");
                    ExitCode::from(exit::INTERNAL_ERROR)
                }
            }
        }
        Command::Validate { path } => {
            let dir = path.unwrap_or_else(|| PathBuf::from("test-vectors"));
            if !dir.is_dir() {
                eprintln!(
                    "vietime-bench: {} is not a directory",
                    dir.display()
                );
                return ExitCode::from(exit::USAGE_ERROR);
            }

            let vectors = match vector::load_vectors_from_dir(&dir) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("vietime-bench: {e}");
                    return ExitCode::from(exit::INTERNAL_ERROR);
                }
            };

            println!(
                "Loaded {} vectors from {}",
                vectors.len(),
                dir.display()
            );

            match vector::validate_vectors(&vectors) {
                Ok(()) => {
                    println!("All vectors valid (NFC normalised, unique IDs).");
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("vietime-bench: {e}");
                    ExitCode::from(exit::USAGE_ERROR)
                }
            }
        }
        Command::Report { run_id, format } => {
            let run_label = run_id.as_deref().unwrap_or("most-recent");
            let fmt_label = format.as_deref().unwrap_or("markdown");
            eprintln!(
                "vietime-bench: `report {run_label} --format {fmt_label}` lands \
                 in BEN-61 (Week 7)."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Compare { base, head } => {
            eprintln!(
                "vietime-bench: `compare --base {base} --head {head}` lands in \
                 BEN-62 (Week 7)."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
        Command::Inspect { run_id, vector_id } => {
            eprintln!(
                "vietime-bench: `inspect {run_id} {vector_id}` lands in BEN-63 \
                 (Week 7)."
            );
            ExitCode::from(exit::USAGE_ERROR)
        }
    }
}

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
    println!("  - xvfb (X11)");
    println!("  - weston (Wayland, Week 4 — BEN-30, stub)");

    println!();
    println!("Registered keystroke injectors:");
    println!("  - xdotool (X11)");
    println!("  - ydotool (Wayland, Week 4 — BEN-31, stub)");

    println!();
    println!("Registered IM drivers:");
    println!("  - ibus (IBus, Week 2)");
    println!("  - fcitx5 (Fcitx5, Week 5 — BEN-40, stub)");

    println!();
    println!("Registered app runners:");
    println!("  - gedit (GTK, Week 2)");
    println!("  - kate (Qt, Week 3 — BEN-20, stub)");
    println!("  - firefox (Gecko/CDP, Week 3 — BEN-21, stub)");
}
