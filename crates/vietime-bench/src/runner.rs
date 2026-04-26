// SPDX-License-Identifier: GPL-3.0-or-later
//
// Matrix runner loop (BEN-14).
//
// Orchestrates: for each (engine × app × mode × session) combo, start the
// session, IM daemon, and app, then iterate over test vectors — inject keys,
// capture output, score. Aggregates per-combo `ComboResult` into a `RunResult`.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.6, §B.9.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_runner::{AppRunner, AppRunnerError};
use crate::im_driver::{ImDriver, ImDriverError};
use crate::injector::KeystrokeInjector;
use crate::model::InputMode;
use crate::scoring::{self, ComboScore};
use crate::session::{SessionDriver, SessionType};
use crate::vector::TestVector;

/// How long to wait after injecting keys before reading text — gives the IME
/// time to flush its compose buffer.
const POST_INJECT_DELAY: Duration = Duration::from_millis(200);

/// Default inter-key delay in milliseconds.
const DEFAULT_MS_PER_KEY: u32 = 30;

/// Describes one (engine × app × session × mode) combination to test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCombo {
    pub engine: String,
    pub app_id: String,
    pub session_type: SessionType,
    pub mode: InputMode,
}

/// Per-combo result including scores and failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboResult {
    pub engine: String,
    pub app: String,
    pub session: SessionType,
    pub mode: InputMode,
    pub score: ComboScore,
    pub failures: Vec<VectorFailure>,
    pub duration_ms: u64,
}

/// Detail about a single failed vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorFailure {
    pub vector_id: String,
    pub expected: String,
    pub actual: String,
    pub edit_distance: usize,
    pub screenshot_path: Option<PathBuf>,
}

/// Anomaly encountered during a run (non-fatal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAnomaly {
    pub kind: String,
    pub detail: String,
    pub retry_count: u32,
}

/// Complete result of a bench run across all combos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub schema_version: u32,
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub matrix: Vec<ComboResult>,
    pub anomalies: Vec<RunAnomaly>,
}

impl RunResult {
    /// Build an empty result with a fresh run id and timestamps set to now.
    #[must_use]
    pub fn new_empty() -> Self {
        let now = Utc::now();
        Self {
            schema_version: 1,
            run_id: format!(
                "{}-{}",
                now.format("%Y-%m-%dT%H-%M-%SZ"),
                &Uuid::new_v4().to_string()[..7]
            ),
            started_at: now,
            finished_at: now,
            matrix: Vec::new(),
            anomalies: Vec::new(),
        }
    }
}

/// Run a single combo: start session + IM + app, iterate vectors, score.
///
/// This is the inner loop of the matrix runner. The caller (the CLI `run`
/// subcommand) builds the list of combos and calls this once per combo.
pub async fn run_combo(
    session_driver: &mut dyn SessionDriver,
    im_driver: &mut dyn ImDriver,
    app_runner: &mut dyn AppRunner,
    injector: &dyn KeystrokeInjector,
    vectors: &[TestVector],
    combo: &RunCombo,
) -> Result<ComboResult, RunError> {
    let start = std::time::Instant::now();

    // 1. Start session.
    let session_handle = session_driver
        .start()
        .await
        .map_err(|e| RunError::Session(e.to_string()))?;

    // 2. Start IM daemon.
    im_driver
        .start(&session_handle)
        .await
        .map_err(|e| RunError::ImDriver(e.to_string()))?;

    // 3. Activate engine + set mode.
    im_driver
        .activate_engine(&combo.engine)
        .await
        .map_err(|e| RunError::ImDriver(e.to_string()))?;
    im_driver
        .set_mode(combo.mode)
        .await
        .map_err(|e| RunError::ImDriver(e.to_string()))?;

    // 4. Launch app.
    let inst = app_runner
        .launch(&session_handle)
        .await
        .map_err(|e| RunError::App(e.to_string()))?;

    // 5. Iterate vectors.
    let mut scores = Vec::with_capacity(vectors.len());
    let mut failures = Vec::new();

    for vector in vectors {
        // Clear + focus.
        if let Err(e) = app_runner.clear_text_area(&inst).await {
            tracing::warn!(vector_id = %vector.id, err = %e, "clear_text_area failed, skipping");
            continue;
        }
        if let Err(e) = app_runner.focus_text_area(&inst).await {
            tracing::warn!(vector_id = %vector.id, err = %e, "focus_text_area failed, skipping");
            continue;
        }

        // Inject keys.
        if let Err(e) = injector.type_raw(&vector.input_keys, DEFAULT_MS_PER_KEY).await {
            tracing::warn!(vector_id = %vector.id, err = %e, "injection failed, skipping");
            continue;
        }

        tokio::time::sleep(POST_INJECT_DELAY).await;

        // Capture output.
        let actual = match app_runner.read_text(&inst).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(vector_id = %vector.id, err = %e, "read_text failed, skipping");
                continue;
            }
        };

        let vs = scoring::score_vector(&vector.id, &vector.expected_output, &actual);
        if !vs.exact_match {
            failures.push(VectorFailure {
                vector_id: vector.id.clone(),
                expected: vector.expected_output.clone(),
                actual: actual.clone(),
                edit_distance: vs.edit_distance,
                screenshot_path: None,
            });
        }
        scores.push(vs);
    }

    // 6. Tear down.
    let _ = app_runner.close(inst).await;
    let _ = im_driver.stop().await;
    let _ = session_driver.stop().await;

    let score = scoring::aggregate_scores(&scores);
    let elapsed = start.elapsed();

    Ok(ComboResult {
        engine: combo.engine.clone(),
        app: combo.app_id.clone(),
        session: combo.session_type,
        mode: combo.mode,
        score,
        failures,
        #[allow(clippy::cast_possible_truncation)]
        duration_ms: elapsed.as_millis() as u64,
    })
}

/// Save a `RunResult` as JSON to `<runs_dir>/<run_id>/summary.json`.
#[allow(clippy::similar_names)]
pub fn save_run_result(result: &RunResult, runs_dir: &std::path::Path) -> Result<(), RunError> {
    let run_dir = runs_dir.join(&result.run_id);
    std::fs::create_dir_all(&run_dir)
        .map_err(|e| RunError::Io(format!("creating {}: {e}", run_dir.display())))?;

    let summary_path = run_dir.join("summary.json");
    let json = serde_json::to_string_pretty(result)
        .map_err(|e| RunError::Io(format!("serialising run result: {e}")))?;
    std::fs::write(&summary_path, json)
        .map_err(|e| RunError::Io(format!("writing {}: {e}", summary_path.display())))?;

    // Save individual failure details.
    if result.matrix.iter().any(|c| !c.failures.is_empty()) {
        let failures_dir = run_dir.join("failures");
        std::fs::create_dir_all(&failures_dir)
            .map_err(|e| RunError::Io(format!("creating {}: {e}", failures_dir.display())))?;

        for combo in &result.matrix {
            for failure in &combo.failures {
                let path = failures_dir.join(format!("{}.json", failure.vector_id));
                let json = serde_json::to_string_pretty(failure)
                    .map_err(|e| RunError::Io(format!("serialising failure: {e}")))?;
                std::fs::write(&path, json)
                    .map_err(|e| RunError::Io(format!("writing {}: {e}", path.display())))?;
            }
        }
    }

    // Symlink `latest` → this run.
    let latest = runs_dir.join("latest");
    let _ = std::fs::remove_file(&latest);
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&result.run_id, &latest);
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("session error: {0}")]
    Session(String),

    #[error("IM driver error: {0}")]
    ImDriver(String),

    #[error("app runner error: {0}")]
    App(String),

    #[error("injection error: {0}")]
    Injection(String),

    #[error("i/o error: {0}")]
    Io(String),
}

impl From<AppRunnerError> for RunError {
    fn from(e: AppRunnerError) -> Self {
        Self::App(e.to_string())
    }
}

impl From<ImDriverError> for RunError {
    fn from(e: ImDriverError) -> Self {
        Self::ImDriver(e.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn run_result_new_empty_generates_id() {
        let r = RunResult::new_empty();
        assert!(!r.run_id.is_empty());
        assert_eq!(r.schema_version, 1);
        assert!(r.matrix.is_empty());
    }

    #[test]
    fn save_and_load_run_result() {
        let dir = tempfile::tempdir().unwrap();
        let mut result = RunResult::new_empty();
        result.matrix.push(ComboResult {
            engine: "ibus-bamboo".to_owned(),
            app: "gedit".to_owned(),
            session: SessionType::X11,
            mode: InputMode::Telex,
            score: scoring::aggregate_scores(&[
                scoring::score_vector("T001", "â", "â"),
            ]),
            failures: vec![],
            duration_ms: 1234,
        });
        result.finished_at = Utc::now();

        save_run_result(&result, dir.path()).unwrap();

        let summary_path = dir.path().join(&result.run_id).join("summary.json");
        assert!(summary_path.exists());

        let loaded: RunResult =
            serde_json::from_str(&std::fs::read_to_string(summary_path).unwrap()).unwrap();
        assert_eq!(loaded.run_id, result.run_id);
        assert_eq!(loaded.matrix.len(), 1);
        assert_eq!(loaded.matrix[0].engine, "ibus-bamboo");
    }

    #[test]
    fn save_writes_failure_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut result = RunResult::new_empty();
        result.matrix.push(ComboResult {
            engine: "ibus-bamboo".to_owned(),
            app: "gedit".to_owned(),
            session: SessionType::X11,
            mode: InputMode::Telex,
            score: scoring::aggregate_scores(&[
                scoring::score_vector("T001", "người", "ngưới"),
            ]),
            failures: vec![VectorFailure {
                vector_id: "T001".to_owned(),
                expected: "người".to_owned(),
                actual: "ngưới".to_owned(),
                edit_distance: 2,
                screenshot_path: None,
            }],
            duration_ms: 500,
        });

        save_run_result(&result, dir.path()).unwrap();

        let failure_path = dir
            .path()
            .join(&result.run_id)
            .join("failures")
            .join("T001.json");
        assert!(failure_path.exists());
    }

    #[test]
    fn latest_symlink_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let result = RunResult::new_empty();
        save_run_result(&result, dir.path()).unwrap();
        let latest = dir.path().join("latest");
        assert!(latest.exists() || latest.symlink_metadata().is_ok());
    }
}
