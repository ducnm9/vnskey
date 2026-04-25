// SPDX-License-Identifier: GPL-3.0-or-later
//
// `im.ibus.engines` — enumerates engines that IBus has registered via
// `ibus list-engine`.
//
// Output shape (per line):
//
//     <language-group> - <display-name> - <engine-id>
//
// We extract the trailing `<engine-id>` (split on `-`, trim, last
// non-empty token) because that's the stable ID used elsewhere in the
// IBus stack. For lines like `English (US) - xkb:us::eng` the split on
// `-` is unambiguous; for the corner case where an engine's id itself
// contains `-` we still take only the final trimmed segment which
// matches what IBus's own tooling does.
//
// This detector contributes:
//
//   * `partial.engines` — one `EngineFact` per listed engine, with
//     `is_registered = true` and `is_vietnamese` flagged via
//     [`vietime_core::is_vietnamese_engine`].
//   * `partial.ibus.registered_engines` — the raw name list. The
//     orchestrator's Week-3 elementwise merge keeps this from clobbering
//     `IbusDaemonDetector`'s version/pid output.
//
// On any failure (binary missing, unreadable output) we return an empty
// partial rather than an error — the checker layer gets to decide what
// "no IBus engines registered" means (it's not a bug per se, just an
// uncommon config).
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3 (DOC-21).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use vietime_core::{is_vietnamese_engine, EngineFact, IbusFacts, ImFramework};

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};
use crate::process::{CommandRunner, TokioCommandRunner};

#[derive(Debug)]
pub struct IbusEnginesDetector {
    runner: Arc<dyn CommandRunner>,
}

impl Default for IbusEnginesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl IbusEnginesDetector {
    #[must_use]
    pub fn new() -> Self {
        Self { runner: Arc::new(TokioCommandRunner::with_timeout(Duration::from_secs(2))) }
    }

    #[must_use]
    pub fn with_runner(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Detector for IbusEnginesDetector {
    fn id(&self) -> &'static str {
        "im.ibus.engines"
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(3)
    }

    async fn run(&self, _ctx: &DetectorContext) -> DetectorResult {
        let stdout = match self.runner.run("ibus", &["list-engine"]).await {
            Ok(s) => s,
            Err(e) => {
                debug!("ibus list-engine failed: {e}");
                return Ok(DetectorOutput::default());
            }
        };
        let names = parse_engine_ids(&stdout);
        if names.is_empty() {
            return Ok(DetectorOutput::default());
        }

        let engines: Vec<EngineFact> = names
            .iter()
            .map(|n| EngineFact {
                name: n.clone(),
                package: None,
                version: None,
                framework: ImFramework::Ibus,
                is_vietnamese: is_vietnamese_engine(n),
                is_registered: true,
            })
            .collect();

        // Also mirror the bare name list into `ibus.registered_engines`
        // so the framework facts remain the single source of truth for
        // "what does IBus list". Daemon/version fields are left default —
        // they come from `IbusDaemonDetector` and are preserved by
        // `PartialFacts::merge_from`'s elementwise merge.
        let ibus_facts = IbusFacts {
            version: None,
            daemon_running: false,
            daemon_pid: None,
            config_dir: None,
            registered_engines: names.clone(),
        };

        Ok(DetectorOutput {
            partial: PartialFacts { engines, ibus: Some(ibus_facts), ..PartialFacts::default() },
            notes: vec![format!("ibus list-engine: {} engines", names.len())],
        })
    }
}

/// Extract engine IDs from `ibus list-engine` stdout.
///
/// IBus prints one engine per line, with fields separated by ` - `
/// (space-hyphen-space). The engine ID is the last non-empty segment
/// after splitting on that exact separator — a bare `-` inside any
/// field (e.g. `xkb:us::eng` doesn't have one, but `mozc-jp` does) must
/// NOT be a split point, which is why we can't just `rsplit('-')`.
/// Duplicate ids within the same run are deduped because IBus can print
/// the same engine under multiple language groups.
pub(crate) fn parse_engine_ids(stdout: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Language-group headers are single words with a trailing colon
        // in newer IBus; skip them.
        if !trimmed.contains(" - ") && trimmed.ends_with(':') {
            continue;
        }
        // Prefer the ` - ` split. Fall back to the whole line when the
        // output has no separators at all (some `ibus list-engine`
        // variants print bare engine ids).
        let id = trimmed.rsplit(" - ").next().map_or(trimmed, str::trim);
        if id.is_empty() {
            continue;
        }
        let id = id.to_owned();
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::process::tests::FakeCommandRunner;

    fn runner(stdout: &str) -> Arc<FakeCommandRunner> {
        let mut r = FakeCommandRunner::default();
        r.ok.insert(("ibus".to_owned(), "list-engine".to_owned()), stdout.to_owned());
        Arc::new(r)
    }

    fn runner_err() -> Arc<FakeCommandRunner> {
        let mut r = FakeCommandRunner::default();
        r.err.insert(("ibus".to_owned(), "list-engine".to_owned()), std::io::ErrorKind::NotFound);
        Arc::new(r)
    }

    #[tokio::test]
    async fn parses_mixed_vietnamese_and_xkb_engines() {
        let out = "\
Vietnamese - Bamboo - bamboo
Vietnamese - Unikey - unikey
English (US) - xkb:us::eng
Japanese - Mozc - mozc-jp
";
        let det = IbusEnginesDetector::with_runner(runner(out));
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert_eq!(res.partial.engines.len(), 4);
        let names: Vec<_> = res.partial.engines.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["bamboo", "unikey", "xkb:us::eng", "mozc-jp"]);

        // Vietnamese flag set only for bamboo/unikey.
        assert!(res.partial.engines[0].is_vietnamese);
        assert!(res.partial.engines[1].is_vietnamese);
        assert!(!res.partial.engines[2].is_vietnamese);
        assert!(!res.partial.engines[3].is_vietnamese);

        // All ibus-reported engines land in registered_engines too.
        let ibus = res.partial.ibus.expect("ibus slot set");
        assert_eq!(ibus.registered_engines, vec!["bamboo", "unikey", "xkb:us::eng", "mozc-jp"]);
    }

    #[tokio::test]
    async fn missing_ibus_binary_returns_empty_partial() {
        let det = IbusEnginesDetector::with_runner(runner_err());
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert!(res.partial.engines.is_empty());
        assert!(res.partial.ibus.is_none());
    }

    #[tokio::test]
    async fn empty_stdout_returns_empty_partial() {
        let det = IbusEnginesDetector::with_runner(runner(""));
        let res = det.run(&DetectorContext::default()).await.expect("ok");
        assert!(res.partial.engines.is_empty());
    }

    #[tokio::test]
    async fn id_is_im_ibus_engines() {
        let d = IbusEnginesDetector::new();
        assert_eq!(d.id(), "im.ibus.engines");
    }
}
