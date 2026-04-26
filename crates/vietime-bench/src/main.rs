// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-bench` — CLI entry point.
// Spec ref: `spec/03-phase3-test-suite.md` §A.4.

#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::manual_let_else,
    clippy::single_match_else,
    clippy::uninlined_format_args,
    clippy::format_push_string,
    clippy::needless_pass_by_value,
)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use vietime_bench::app_runner::{self, ALL_APP_IDS};
use vietime_bench::im_driver;
use vietime_bench::injector;
use vietime_bench::profile::{self, Profile};
use vietime_bench::runner::{self, RunAnomaly, RunResult};
use vietime_bench::session;
use vietime_bench::vector;
use vietime_bench::{Cli, Command, InputMode, TOOL_VERSION};
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
fn dispatch(mut cli: Cli) -> ExitCode {
    let command = cli.command.take().unwrap_or(Command::Hello);

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
        Command::Run => dispatch_run(cli),
        Command::Validate { path } => dispatch_validate(path),
        Command::Report { run_id, format } => dispatch_report(
            run_id,
            format.as_deref().unwrap_or("markdown"),
            cli.runs_dir.as_deref(),
        ),
        Command::Compare { base, head } => dispatch_compare(
            &base,
            &head,
            cli.runs_dir.as_deref(),
        ),
        Command::Inspect { run_id, vector_id } => dispatch_inspect(
            &run_id,
            &vector_id,
            cli.runs_dir.as_deref(),
        ),
    }
}

fn dispatch_run(cli: Cli) -> ExitCode {
    // Validate flags early.
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
        eprintln!("vietime-bench: test-vectors/ directory not found.");
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

    // Resolve profile → combos.
    let profiles_dir = PathBuf::from("profiles");
    let resolved_profile: Profile = if let Some(profile_name) = cli.profile.as_deref() {
        match profile::resolve_profile(profile_name, &profiles_dir) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("vietime-bench: {e}");
                return ExitCode::from(exit::USAGE_ERROR);
            }
        }
    } else {
        // Build an ad-hoc profile from CLI flags.
        Profile {
            name: "adhoc".to_owned(),
            description: None,
            engines: vec![cli.engine.unwrap_or_else(|| "ibus-bamboo".to_owned())],
            apps: vec![cli.app.unwrap_or_else(|| "gedit".to_owned())],
            sessions: vec![cli.session.unwrap_or_else(|| "x11".to_owned())],
            modes: vec![cli.mode.unwrap_or_else(|| "telex".to_owned())],
            vector_tags: vec![],
        }
    };

    let combos = resolved_profile.expand_combos();
    if combos.is_empty() {
        eprintln!("vietime-bench: profile expands to zero combos");
        return ExitCode::from(exit::USAGE_ERROR);
    }

    // Filter vectors by profile tags.
    let filtered_vectors: Vec<_> = if resolved_profile.vector_tags.is_empty() {
        vectors
    } else {
        vectors
            .into_iter()
            .filter(|v| {
                v.tags
                    .iter()
                    .any(|t| resolved_profile.vector_tags.contains(t))
            })
            .collect()
    };

    if filtered_vectors.is_empty() {
        eprintln!("vietime-bench: no vectors match profile tag filter");
        return ExitCode::from(exit::USAGE_ERROR);
    }

    let runs_dir = cli.runs_dir.unwrap_or_else(|| PathBuf::from("runs"));

    println!(
        "vietime-bench: profile={}, {} combos × {} vectors",
        resolved_profile.name,
        combos.len(),
        filtered_vectors.len(),
    );

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("vietime-bench: failed to start async runtime: {e}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };

    let result = rt.block_on(async {
        let mut run_result = RunResult::new_empty();

        for combo in &combos {
            println!(
                "\n--- {} × {} × {} × {} ---",
                combo.engine,
                combo.app_id,
                combo.session_type.as_str(),
                combo.mode,
            );

            // Resolve session driver.
            let mut session_driver =
                match session::resolve_session(combo.session_type.as_str()) {
                    Some(d) => d,
                    None => {
                        let msg = format!(
                            "no session driver for {}",
                            combo.session_type.as_str()
                        );
                        eprintln!("  SKIP: {msg}");
                        run_result.anomalies.push(RunAnomaly {
                            kind: "SessionSkipped".to_owned(),
                            detail: msg,
                            retry_count: 0,
                        });
                        continue;
                    }
                };

            // Resolve IM driver.
            let (mut im_driver, engine_name) =
                match im_driver::resolve_im_driver(&combo.engine) {
                    Some(pair) => pair,
                    None => {
                        let msg = format!("no IM driver for {}", combo.engine);
                        eprintln!("  SKIP: {msg}");
                        run_result.anomalies.push(RunAnomaly {
                            kind: "ImDriverSkipped".to_owned(),
                            detail: msg,
                            retry_count: 0,
                        });
                        continue;
                    }
                };

            // Resolve app runner.
            let mut app_runner = match app_runner::resolve_app(&combo.app_id) {
                Some(r) => r,
                None => {
                    let msg = format!("no app runner for {}", combo.app_id);
                    eprintln!("  SKIP: {msg}");
                    run_result.anomalies.push(RunAnomaly {
                        kind: "AppSkipped".to_owned(),
                        detail: msg,
                        retry_count: 0,
                    });
                    continue;
                }
            };

            // Override the engine name in the combo for the actual run.
            let mut run_combo = combo.clone();
            run_combo.engine = engine_name;

            // Resolve injector based on session type.
            let display = session_driver
                .id()
                .to_owned();
            let injector = injector::resolve_injector(
                combo.session_type.as_str(),
                &format!(":{}", 99), // placeholder, real display comes from start()
            );

            match runner::run_combo(
                session_driver.as_mut(),
                im_driver.as_mut(),
                app_runner.as_mut(),
                injector.as_ref(),
                &filtered_vectors,
                &run_combo,
            )
            .await
            {
                Ok(combo_result) => {
                    println!(
                        "  accuracy={:.1}%  exact={}/{}  duration={}ms",
                        combo_result.score.accuracy_pct,
                        combo_result.score.exact_match_count,
                        combo_result.score.vectors_tested,
                        combo_result.duration_ms,
                    );
                    run_result.matrix.push(combo_result);
                }
                Err(e) => {
                    eprintln!("  FAIL: {e}");
                    run_result.anomalies.push(RunAnomaly {
                        kind: "ComboFailed".to_owned(),
                        detail: format!("{}: {e}", display),
                        retry_count: 0,
                    });
                }
            }
        }

        run_result.finished_at = chrono::Utc::now();
        runner::save_run_result(&run_result, &runs_dir)?;
        Ok::<RunResult, runner::RunError>(run_result)
    });

    match result {
        Ok(r) => {
            println!("\n=== Summary ===");
            println!("Run ID:  {}", r.run_id);
            println!("Combos:  {} tested, {} skipped", r.matrix.len(), r.anomalies.len());
            for c in &r.matrix {
                println!(
                    "  {:<20} {:<10} {:<8} {:<12} accuracy={:.1}%  failures={}",
                    c.engine,
                    c.app,
                    c.session.as_str(),
                    c.mode.as_str(),
                    c.score.accuracy_pct,
                    c.failures.len(),
                );
            }
            if !r.anomalies.is_empty() {
                println!("Anomalies:");
                for a in &r.anomalies {
                    println!("  [{}] {}", a.kind, a.detail);
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

fn dispatch_validate(path: Option<PathBuf>) -> ExitCode {
    let dir = path.unwrap_or_else(|| PathBuf::from("test-vectors"));
    if !dir.is_dir() {
        eprintln!("vietime-bench: {} is not a directory", dir.display());
        return ExitCode::from(exit::USAGE_ERROR);
    }
    let vectors = match vector::load_vectors_from_dir(&dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("vietime-bench: {e}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };
    println!("Loaded {} vectors from {}", vectors.len(), dir.display());
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

fn dispatch_report(
    run_id: Option<String>,
    format: &str,
    runs_dir: Option<&std::path::Path>,
) -> ExitCode {
    let runs = runs_dir.unwrap_or_else(|| std::path::Path::new("runs"));
    let run_dir = if let Some(id) = &run_id {
        runs.join(id)
    } else {
        runs.join("latest")
    };

    let summary_path = run_dir.join("summary.json");
    if !summary_path.exists() {
        // Try resolving "latest" symlink.
        let resolved = std::fs::read_link(&run_dir)
            .map(|target| runs.join(target).join("summary.json"))
            .unwrap_or(summary_path);
        if !resolved.exists() {
            eprintln!(
                "vietime-bench: no run found at {}",
                run_dir.display()
            );
            return ExitCode::from(exit::USAGE_ERROR);
        }
        return render_report(&resolved, format);
    }
    render_report(&summary_path, format)
}

fn render_report(summary_path: &std::path::Path, format: &str) -> ExitCode {
    let content = match std::fs::read_to_string(summary_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("vietime-bench: {e}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };
    let result: RunResult = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("vietime-bench: invalid summary JSON: {e}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };

    match format {
        "json" => {
            match serde_json::to_string_pretty(&result) {
                Ok(j) => println!("{j}"),
                Err(e) => {
                    eprintln!("vietime-bench: {e}");
                    return ExitCode::from(exit::INTERNAL_ERROR);
                }
            }
        }
        "html" => {
            println!("{}", render_html(&result));
        }
        _ => {
            println!("{}", render_markdown(&result));
        }
    }
    ExitCode::from(exit::OK)
}

fn render_markdown(r: &RunResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("# VietIME Bench — {}\n\n", r.run_id));
    out.push_str(&format!(
        "Started: {}  \nFinished: {}\n\n",
        r.started_at, r.finished_at
    ));
    out.push_str("| Engine | App | Session | Mode | Accuracy | Exact | Edit Dist | Duration |\n");
    out.push_str("|--------|-----|---------|------|----------|-------|-----------|----------|\n");
    for c in &r.matrix {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}% | {}/{} | {} | {}ms |\n",
            c.engine,
            c.app,
            c.session.as_str(),
            c.mode.as_str(),
            c.score.accuracy_pct,
            c.score.exact_match_count,
            c.score.vectors_tested,
            c.score.edit_distance_total,
            c.duration_ms,
        ));
    }
    if !r.anomalies.is_empty() {
        out.push_str("\n## Anomalies\n\n");
        for a in &r.anomalies {
            out.push_str(&format!("- **{}**: {}\n", a.kind, a.detail));
        }
    }
    out
}

fn render_html(r: &RunResult) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<title>VietIME Bench Matrix</title>\n");
    out.push_str("<style>\n");
    out.push_str("body { font-family: -apple-system, sans-serif; max-width: 1200px; margin: 2em auto; padding: 0 1em; }\n");
    out.push_str("table { border-collapse: collapse; width: 100%; }\n");
    out.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
    out.push_str("th { background: #f5f5f5; }\n");
    out.push_str(".good { background: #d4edda; }\n");
    out.push_str(".warn { background: #fff3cd; }\n");
    out.push_str(".bad { background: #f8d7da; }\n");
    out.push_str("</style>\n</head>\n<body>\n");
    out.push_str(&format!("<h1>VietIME Bench — {}</h1>\n", r.run_id));
    out.push_str(&format!(
        "<p>Started: {} | Finished: {}</p>\n",
        r.started_at, r.finished_at
    ));
    out.push_str("<table>\n<tr><th>Engine</th><th>App</th><th>Session</th><th>Mode</th><th>Accuracy</th><th>Exact</th><th>Edit Dist</th><th>Duration</th></tr>\n");
    for c in &r.matrix {
        let class = if c.score.accuracy_pct >= 95.0 {
            "good"
        } else if c.score.accuracy_pct >= 80.0 {
            "warn"
        } else {
            "bad"
        };
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{class}\">{:.1}%</td><td>{}/{}</td><td>{}</td><td>{}ms</td></tr>\n",
            c.engine,
            c.app,
            c.session.as_str(),
            c.mode.as_str(),
            c.score.accuracy_pct,
            c.score.exact_match_count,
            c.score.vectors_tested,
            c.score.edit_distance_total,
            c.duration_ms,
        ));
    }
    out.push_str("</table>\n");
    if !r.anomalies.is_empty() {
        out.push_str("<h2>Anomalies</h2>\n<ul>\n");
        for a in &r.anomalies {
            out.push_str(&format!("<li><strong>{}</strong>: {}</li>\n", a.kind, a.detail));
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</body>\n</html>");
    out
}

fn dispatch_compare(base: &str, head: &str, runs_dir: Option<&std::path::Path>) -> ExitCode {
    let runs = runs_dir.unwrap_or_else(|| std::path::Path::new("runs"));
    let load = |id: &str| -> Result<RunResult, String> {
        let path = runs.join(id).join("summary.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
    };

    let base_result = match load(base) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("vietime-bench: {e}");
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };
    let head_result = match load(head) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("vietime-bench: {e}");
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };

    println!("# Compare: {} vs {}\n", base, head);
    println!(
        "| Engine | App | Session | Mode | Base Acc | Head Acc | Delta |"
    );
    println!(
        "|--------|-----|---------|------|----------|----------|-------|"
    );

    for hc in &head_result.matrix {
        let base_acc = base_result
            .matrix
            .iter()
            .find(|bc| {
                bc.engine == hc.engine
                    && bc.app == hc.app
                    && bc.session == hc.session
                    && bc.mode == hc.mode
            })
            .map_or(f64::NAN, |bc| bc.score.accuracy_pct);

        let delta = hc.score.accuracy_pct - base_acc;
        let marker = if delta < -5.0 { " !!REGRESSION" } else { "" };
        println!(
            "| {} | {} | {} | {} | {:.1}% | {:.1}% | {:+.1}%{} |",
            hc.engine,
            hc.app,
            hc.session.as_str(),
            hc.mode.as_str(),
            base_acc,
            hc.score.accuracy_pct,
            delta,
            marker,
        );
    }
    ExitCode::from(exit::OK)
}

fn dispatch_inspect(
    run_id: &str,
    vector_id: &str,
    runs_dir: Option<&std::path::Path>,
) -> ExitCode {
    let runs = runs_dir.unwrap_or_else(|| std::path::Path::new("runs"));

    // Load summary.
    let summary_path = runs.join(run_id).join("summary.json");
    let content = match std::fs::read_to_string(&summary_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "vietime-bench: cannot read {}: {e}",
                summary_path.display()
            );
            return ExitCode::from(exit::USAGE_ERROR);
        }
    };
    let result: RunResult = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("vietime-bench: invalid JSON: {e}");
            return ExitCode::from(exit::INTERNAL_ERROR);
        }
    };

    // Find the failure.
    for combo in &result.matrix {
        for failure in &combo.failures {
            if failure.vector_id == vector_id {
                println!("Run:       {}", result.run_id);
                println!("Combo:     {} × {} × {} × {}",
                    combo.engine, combo.app, combo.session.as_str(), combo.mode.as_str());
                println!("Vector:    {}", failure.vector_id);
                println!("Expected:  {:?}", failure.expected);
                println!("Actual:    {:?}", failure.actual);
                println!("Edit dist: {}", failure.edit_distance);
                if let Some(screenshot) = &failure.screenshot_path {
                    println!("Screenshot: {}", screenshot.display());
                }
                println!("\nReproducer:");
                println!("  vietime-bench run --engine {} --app {} --session {} --mode {}",
                    combo.engine, combo.app, combo.session.as_str(), combo.mode.as_str());
                return ExitCode::from(exit::OK);
            }
        }
    }

    // Also check per-failure JSON files.
    let failure_path = runs
        .join(run_id)
        .join("failures")
        .join(format!("{vector_id}.json"));
    if failure_path.exists() {
        match std::fs::read_to_string(&failure_path) {
            Ok(c) => {
                println!("{c}");
                return ExitCode::from(exit::OK);
            }
            Err(e) => {
                eprintln!("vietime-bench: {e}");
                return ExitCode::from(exit::INTERNAL_ERROR);
            }
        }
    }

    eprintln!(
        "vietime-bench: vector `{vector_id}` not found in run `{run_id}` failures"
    );
    ExitCode::from(exit::USAGE_ERROR)
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
    println!("  - weston (Wayland)");

    println!();
    println!("Registered keystroke injectors:");
    println!("  - xdotool (X11)");
    println!("  - ydotool (Wayland, wtype fallback)");

    println!();
    println!("Registered IM drivers:");
    println!("  - ibus (IBus)");
    println!("  - fcitx5 (Fcitx5)");

    println!();
    println!("Registered app runners:");
    for id in ALL_APP_IDS {
        println!("  - {id}");
    }

    println!();
    println!("Built-in profiles:");
    println!("  - smoke (3 apps × 1 engine × x11)");
    println!("  - full (6 apps × 4 engines × x11+wayland)");
    println!("  - bugs (regression vectors only)");
}
