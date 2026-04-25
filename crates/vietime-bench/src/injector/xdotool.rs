// SPDX-License-Identifier: GPL-3.0-or-later
//
// `XdotoolInjector` — shell out to `xdotool(1)` to type into an X11 window.
//
// Week 1 lands the argv builder + ms clamp (both pure, both unit-tested) plus
// an end-to-end spawn path gated behind `#[ignore]` because the CI Linux box
// needs a real X server to exercise it. macOS devs can't run it locally.
//
// Design note: we never pass user-provided keys on the command line without
// a `--` terminator; xdotool otherwise interprets anything starting with `-`
// (e.g. test vectors like `-foo`) as a flag and errors out confusingly.
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.4.

use async_trait::async_trait;
use tokio::process::Command;

use super::{InjectorError, KeystrokeInjector};

/// Upper bound on `ms_per_key`. 2000 ms is absurdly slow (≈0.5 chars/sec) so
/// anything above that is almost certainly a caller mistaking seconds for
/// milliseconds. Clamping rather than erroring means the run doesn't hang
/// for minutes if someone passes `30_000`.
const MS_PER_KEY_MAX: u32 = 2_000;

/// X11 keystroke injector. Stores the `DISPLAY` we were asked to target so
/// the caller doesn't have to thread it through to every `type_raw` call.
#[derive(Debug, Clone)]
pub struct XdotoolInjector {
    display: String,
}

impl XdotoolInjector {
    /// Build an injector bound to `display` (e.g. `":99"`). The string is
    /// forwarded verbatim into the `DISPLAY` env var — no validation, because
    /// xdotool itself surfaces a clear error for bad displays.
    #[must_use]
    pub fn new(display: impl Into<String>) -> Self {
        Self { display: display.into() }
    }

    /// Display this injector is targeting. Exposed mostly for tests.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }
}

#[async_trait]
impl KeystrokeInjector for XdotoolInjector {
    fn id(&self) -> &'static str {
        "xdotool"
    }

    async fn type_raw(&self, keys: &str, ms_per_key: u32) -> Result<(), InjectorError> {
        let ms = clamp_ms(ms_per_key);
        let argv = build_argv(keys, ms);

        let mut cmd = Command::new("xdotool");
        cmd.args(&argv);
        cmd.env("DISPLAY", &self.display);

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => InjectorError::BinaryMissing("xdotool"),
            _ => InjectorError::Io(e),
        })?;

        if !output.status.success() {
            return Err(InjectorError::NonZeroExit {
                binary: "xdotool",
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(())
    }
}

/// Clamp `ms_per_key` into `[0, MS_PER_KEY_MAX]`. Pure — no side effects,
/// deterministic, cheap to unit-test.
#[must_use]
fn clamp_ms(ms: u32) -> u32 {
    ms.min(MS_PER_KEY_MAX)
}

/// Build the argv vector passed to `xdotool`. Pure — no process spawn, no
/// env lookup. Tested exhaustively so we don't regress the `--` guard by
/// accident and end up with test vectors swallowing leading hyphens.
#[must_use]
fn build_argv(keys: &str, ms: u32) -> Vec<String> {
    vec![
        "type".to_owned(),
        "--delay".to_owned(),
        ms.to_string(),
        // `--` forces xdotool to treat every subsequent arg as a positional
        // rather than a flag, so test vectors like "-foo" don't break.
        "--".to_owned(),
        keys.to_owned(),
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn clamp_leaves_small_values_alone() {
        assert_eq!(clamp_ms(0), 0);
        assert_eq!(clamp_ms(30), 30);
        assert_eq!(clamp_ms(MS_PER_KEY_MAX), MS_PER_KEY_MAX);
    }

    #[test]
    fn clamp_caps_runaway_values() {
        assert_eq!(clamp_ms(10_000), MS_PER_KEY_MAX);
        assert_eq!(clamp_ms(u32::MAX), MS_PER_KEY_MAX);
    }

    #[test]
    fn argv_for_typical_telex_input() {
        assert_eq!(build_argv("aa", 30), vec!["type", "--delay", "30", "--", "aa"],);
    }

    #[test]
    fn argv_preserves_leading_hyphen_via_terminator() {
        // Regression guard: without `--`, xdotool would treat `-foo` as a
        // short flag and error out.
        let argv = build_argv("-foo", 0);
        assert_eq!(argv[3], "--");
        assert_eq!(argv[4], "-foo");
    }

    #[test]
    fn argv_propagates_ms_as_decimal_string() {
        assert_eq!(build_argv("x", 0)[2], "0");
        assert_eq!(build_argv("x", 1234)[2], "1234");
    }

    #[test]
    fn id_is_stable() {
        assert_eq!(XdotoolInjector::new(":99").id(), "xdotool");
    }

    #[test]
    fn display_is_readable_back() {
        let inj = XdotoolInjector::new(":42");
        assert_eq!(inj.display(), ":42");
    }

    // Real-spawn integration test — needs an X server + xdotool binary.
    // macOS dev boxes opt out via `#[ignore]`; CI flips this on in Week 8.
    #[tokio::test]
    #[ignore = "requires xdotool + a live X server on the host"]
    async fn xdotool_types_into_live_display() {
        let inj = XdotoolInjector::new(":99");
        inj.type_raw("aa", 30).await.expect("xdotool should type cleanly");
    }
}
