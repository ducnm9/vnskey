// SPDX-License-Identifier: GPL-3.0-or-later
//
// `vietime-bench` CLI surface.
//
// The Cli + Command types live here (rather than `main.rs`) so both the
// binary and the integration smoke tests can reuse them through the library
// crate. Week 1 only implements `list`, `version`, `hello` for real;
// everything else emits a stub pointing at the ticket that lands it, with
// the same exit-code discipline as doctor / installer.
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.4.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Compatibility matrix benchmark runner for Vietnamese IMEs on Linux.
#[derive(Debug, Parser)]
#[command(name = "vietime-bench", version, about, long_about = None)]
// Clap-derive naturally produces global `bool` flags. Grouping them into a
// sub-struct would obscure the clap surface without adding clarity.
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Profile slug (see `list`); overrides `--engine/--app/--mode/--session`.
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Combo slug (e.g. `fcitx5-bamboo`). See `vietime-installer list`.
    #[arg(long, global = true)]
    pub engine: Option<String>,

    /// Target application (`gedit`, `firefox`, …). Week 4 wiring.
    #[arg(long, global = true)]
    pub app: Option<String>,

    /// Typing mode: `telex`, `vni`, `viqr`, `simple-telex`.
    #[arg(long, global = true)]
    pub mode: Option<String>,

    /// Session type: `x11`, `wayland`.
    #[arg(long, global = true)]
    pub session: Option<String>,

    /// Extra tracing output on stderr.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Directory the runner stores artifacts under. Defaults to `runs/`.
    #[arg(long, global = true)]
    pub runs_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Every top-level subcommand the bench exposes. Only `List`, `Version`,
/// `Hello` are implemented this week; the rest print friendly stubs pointing
/// at the BEN ticket + week that lands them.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Execute a profile and write `runs/<id>/summary.json`. Lands in BEN-14
    /// (Week 2 — matrix orchestrator).
    Run,

    /// List available profiles / apps / engines / sessions / modes.
    List,

    /// Render a past run as markdown/json/html.
    Report {
        /// Run id to render. Defaults to the most recent run.
        #[arg(long)]
        run_id: Option<String>,

        /// Output format.
        #[arg(long, value_parser = ["json", "markdown", "html"])]
        format: Option<String>,
    },

    /// Diff two runs on accuracy.
    Compare {
        /// Baseline run id.
        #[arg(long)]
        base: String,

        /// Comparison run id.
        #[arg(long)]
        head: String,
    },

    /// Sanity-check `test-vectors/*.toml` (NFC, duplicate IDs, tags).
    Validate {
        /// Explicit directory to validate. Defaults to `test-vectors/`.
        path: Option<PathBuf>,
    },

    /// Drill down into a single failed vector from a run.
    Inspect {
        /// Run id containing the vector.
        run_id: String,

        /// Vector id to inspect.
        vector_id: String,
    },

    /// Print the bench version.
    Version,

    /// Smoke check — prints version + `vietime-core ready`.
    Hello,
}
