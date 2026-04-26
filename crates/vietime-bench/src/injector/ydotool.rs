// SPDX-License-Identifier: GPL-3.0-or-later
//
// `YdotoolInjector` — Wayland keystroke injector via ydotool(1).
// BEN-31. Spec ref: `spec/03-phase3-test-suite.md` §B.5.
//
// Requires `/dev/uinput` access. In Docker: `--device /dev/uinput`.
// In CI: `sudo modprobe uinput && sudo chmod 666 /dev/uinput`.

use async_trait::async_trait;
use tokio::process::Command;

use super::{InjectorError, KeystrokeInjector};

const MS_PER_KEY_MAX: u32 = 2_000;

#[derive(Debug, Clone)]
pub struct YdotoolInjector {
    wayland_display: String,
}

impl YdotoolInjector {
    #[must_use]
    pub fn new(wayland_display: impl Into<String>) -> Self {
        Self {
            wayland_display: wayland_display.into(),
        }
    }

    #[must_use]
    pub fn display(&self) -> &str {
        &self.wayland_display
    }
}

#[async_trait]
impl KeystrokeInjector for YdotoolInjector {
    fn id(&self) -> &'static str {
        "ydotool"
    }

    async fn type_raw(&self, keys: &str, ms_per_key: u32) -> Result<(), InjectorError> {
        let ms = ms_per_key.min(MS_PER_KEY_MAX);

        let mut cmd = Command::new("ydotool");
        cmd.args(["type", "--key-delay", &ms.to_string(), "--", keys]);
        cmd.env("WAYLAND_DISPLAY", &self.wayland_display);

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => InjectorError::BinaryMissing("ydotool"),
            _ => InjectorError::Io(e),
        })?;

        if !output.status.success() {
            // Fallback to wtype if ydotool fails (e.g. no uinput access).
            return self.try_wtype_fallback(keys, ms).await;
        }
        Ok(())
    }
}

impl YdotoolInjector {
    async fn try_wtype_fallback(&self, keys: &str, ms: u32) -> Result<(), InjectorError> {
        let mut cmd = Command::new("wtype");
        cmd.args(["-d", &ms.to_string(), "--", keys]);
        cmd.env("WAYLAND_DISPLAY", &self.wayland_display);

        let output = cmd.output().await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => InjectorError::BinaryMissing("ydotool (wtype fallback also missing)"),
            _ => InjectorError::Io(e),
        })?;

        if !output.status.success() {
            return Err(InjectorError::NonZeroExit {
                binary: "wtype",
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable() {
        assert_eq!(YdotoolInjector::new("wayland-0").id(), "ydotool");
    }

    #[test]
    fn display_readable() {
        let inj = YdotoolInjector::new("wayland-bench-0");
        assert_eq!(inj.display(), "wayland-bench-0");
    }

    #[tokio::test]
    #[ignore = "requires ydotool + /dev/uinput + a live Wayland session"]
    async fn ydotool_types_into_wayland() {
        let inj = YdotoolInjector::new("wayland-0");
        inj.type_raw("aa", 30)
            .await
            .expect("ydotool should type cleanly");
    }
}
