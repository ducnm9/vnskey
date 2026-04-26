// SPDX-License-Identifier: GPL-3.0-or-later
//
// Shared xdotool helpers used by all X11-based app runners.

use std::time::Duration;

use tokio::process::Command;

use super::{AppInstance, AppRunnerError};

pub async fn run_xdotool(display: &str, args: &[&str]) -> Result<(), AppRunnerError> {
    let mut cmd = Command::new("xdotool");
    cmd.args(args).env("DISPLAY", display);
    let output = cmd.output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("xdotool"),
        _ => AppRunnerError::Io(e),
    })?;
    if !output.status.success() {
        return Err(AppRunnerError::NonZeroExit {
            binary: "xdotool",
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

pub async fn search_window(display: &str, name: &str) -> Result<String, AppRunnerError> {
    let mut cmd = Command::new("xdotool");
    cmd.args(["search", "--name", name]).env("DISPLAY", display);
    let output = cmd.output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("xdotool"),
        _ => AppRunnerError::Io(e),
    })?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
            return Ok(first_line.to_owned());
        }
    }
    Err(AppRunnerError::CaptureFailure(format!(
        "no window matching '{name}' found"
    )))
}

pub async fn focus_window(display: &str, inst: &AppInstance) -> Result<(), AppRunnerError> {
    if let Some(wid) = &inst.window_id {
        run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
        run_xdotool(display, &["windowfocus", "--sync", wid]).await?;
    }
    Ok(())
}

pub async fn select_all_delete(display: &str, inst: &AppInstance) -> Result<(), AppRunnerError> {
    if let Some(wid) = &inst.window_id {
        run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
    }
    run_xdotool(display, &["key", "ctrl+a"]).await?;
    run_xdotool(display, &["key", "Delete"]).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}

pub async fn copy_and_read_clipboard(
    display: &str,
    inst: &AppInstance,
) -> Result<String, AppRunnerError> {
    if let Some(wid) = &inst.window_id {
        run_xdotool(display, &["windowactivate", "--sync", wid]).await?;
    }
    run_xdotool(display, &["key", "ctrl+a"]).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    run_xdotool(display, &["key", "ctrl+c"]).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut cmd = Command::new("xclip");
    cmd.args(["-selection", "clipboard", "-o"])
        .env("DISPLAY", display);
    let output = cmd.output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppRunnerError::BinaryMissing("xclip"),
        _ => AppRunnerError::Io(e),
    })?;

    if !output.status.success() {
        if output.stderr.is_empty()
            || String::from_utf8_lossy(&output.stderr).contains("target")
        {
            return Ok(String::new());
        }
        return Err(AppRunnerError::CaptureFailure(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
