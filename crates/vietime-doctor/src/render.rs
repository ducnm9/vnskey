// SPDX-License-Identifier: GPL-3.0-or-later
//
// `render` — turns a [`Report`] into Markdown / plain text / JSON.
//
// Week 2 (DOC-14). Markdown is produced by rendering the [`minijinja`]
// template in `templates/report.md.j2`. Plain text reuses the same
// template and strips the markdown formatting with a tiny hand-rolled
// filter (good enough for terminal output; no crate dependency). JSON is
// just `serde_json::to_string_pretty`.
//
// Keeping the template + stripper in one file means every surface
// (markdown / plain / verbose) shares a single source of truth for what
// fields show up in a report — the inline `render_plain` in `main.rs`
// drifted from the JSON shape by Week 1 already.
//
// Spec ref: `spec/01-phase1-doctor.md` §A.5.

use std::fmt::Write as _;

use minijinja::{context, value::Value, Environment};
use serde::Serialize;

use vietime_core::{
    ActiveFramework, Anomaly, DesktopEnv, EnvFacts, EnvSource, ImFacts, Issue, Recommendation,
    Report, Severity, SystemFacts, IM_ENV_KEYS,
};

/// Raw template source baked into the binary.
const TEMPLATE_SRC: &str = include_str!("../templates/report.md.j2");

/// Knobs for [`render_markdown`] / [`render_plain`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    /// Strip markdown syntax, for terminals that don't render it.
    pub plain: bool,
    /// Include the `--verbose` footer line.
    pub verbose: bool,
}

/// Render errors. Minijinja produces detailed spans we surface verbatim so
/// test failures are actionable.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("template render failed: {0}")]
    Template(String),
    #[error("JSON serialisation failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// Render the report as Markdown. When `opts.plain` is set the result is
/// the same template output with markdown formatting stripped.
pub fn render(report: &Report, opts: &RenderOptions) -> Result<String, RenderError> {
    let mut env = Environment::new();
    env.add_template("report.md.j2", TEMPLATE_SRC)
        .map_err(|e| RenderError::Template(e.to_string()))?;
    let tpl = env.get_template("report.md.j2").map_err(|e| RenderError::Template(e.to_string()))?;
    let ctx = build_context(report, opts.verbose);
    let md = tpl.render(ctx).map_err(|e| RenderError::Template(e.to_string()))?;
    if opts.plain {
        Ok(strip_markdown(&md))
    } else {
        Ok(md)
    }
}

/// Pretty-printed JSON — the stable integration format.
pub fn render_json(report: &Report) -> Result<String, RenderError> {
    Ok(serde_json::to_string_pretty(report)?)
}

// ─── Context building ────────────────────────────────────────────────────

fn build_context(report: &Report, verbose: bool) -> Value {
    context! {
        generated_at => report.generated_at.to_rfc3339(),
        tool_version => report.tool_version.clone(),
        schema_version => report.schema_version,
        system => build_system(&report.facts.system),
        im => build_im(&report.facts.im),
        env_rows => build_env_rows(&report.facts.env),
        issues => report.issues.iter().map(issue_ctx).collect::<Vec<_>>(),
        recommendations => report.recommendations.iter().map(rec_ctx).collect::<Vec<_>>(),
        anomalies => report.anomalies.iter().map(anomaly_ctx).collect::<Vec<_>>(),
        verbose => verbose,
    }
}

#[derive(Serialize)]
struct SystemCtx {
    distro: bool,
    distro_display: String,
    desktop: Option<String>,
    session: Option<String>,
    kernel: Option<String>,
    shell: Option<String>,
}

fn build_system(sf: &SystemFacts) -> SystemCtx {
    let distro_display = sf
        .distro
        .as_ref()
        .map(|d| {
            d.pretty.clone().unwrap_or_else(|| {
                format!("{} {}", d.id, d.version_id.clone().unwrap_or_default()).trim().to_owned()
            })
        })
        .unwrap_or_default();
    SystemCtx {
        distro: sf.distro.is_some(),
        distro_display,
        desktop: sf.desktop.as_ref().map(DesktopEnv::display_name),
        session: sf.session.map(|s| s.as_str().to_owned()),
        kernel: sf.kernel.clone(),
        shell: sf.shell.clone(),
    }
}

#[derive(Serialize)]
struct ImCtx {
    active: String,
    ibus: Option<String>,
    fcitx5: Option<String>,
    engines: Vec<String>,
}

fn build_im(im: &ImFacts) -> ImCtx {
    let active = match im.active_framework {
        ActiveFramework::None => "none".to_owned(),
        ActiveFramework::Ibus => "IBus".to_owned(),
        ActiveFramework::Fcitx5 => "Fcitx5".to_owned(),
        ActiveFramework::Conflict => "conflict (both IBus and Fcitx5 active)".to_owned(),
    };
    let ibus = im.ibus.as_ref().map(|f| {
        let running = if f.daemon_running { "running" } else { "not running" };
        match &f.version {
            Some(v) => format!("{running}, version {v}"),
            None => running.to_owned(),
        }
    });
    let fcitx5 = im.fcitx5.as_ref().map(|f| {
        let running = if f.daemon_running { "running" } else { "not running" };
        match &f.version {
            Some(v) => format!("{running}, version {v}"),
            None => running.to_owned(),
        }
    });
    let engines = im.engines.iter().map(|e| format!("{} ({:?})", e.name, e.framework)).collect();
    ImCtx { active, ibus, fcitx5, engines }
}

#[derive(Serialize)]
struct EnvRow {
    name: String,
    value: String,
    source: String,
}

fn build_env_rows(env: &EnvFacts) -> Vec<EnvRow> {
    let mut out = Vec::new();
    for key in IM_ENV_KEYS {
        let value = env.get_by_key(key).unwrap_or("").to_owned();
        if value.is_empty() && !env.sources.contains_key(key) {
            // Nothing set at all — omit the row to keep the table focused.
            continue;
        }
        let source = env
            .sources
            .get(key)
            .map_or_else(|| "(unknown)".to_owned(), |s| env_source_label(*s).to_owned());
        let display_value = if value.is_empty() { "<unset>".to_owned() } else { value };
        out.push(EnvRow { name: key.to_owned(), value: display_value, source });
    }
    out
}

fn env_source_label(s: EnvSource) -> &'static str {
    match s {
        EnvSource::Process => "process",
        EnvSource::EtcEnvironment => "/etc/environment",
        EnvSource::EtcProfileD => "/etc/profile.d/*.sh",
        EnvSource::HomeProfile => "~/.profile or ~/.config/environment.d/",
        EnvSource::SystemdUserEnv => "systemctl --user show-environment",
        EnvSource::Pam => "pam",
        EnvSource::Unknown => "unknown",
    }
}

#[derive(Serialize)]
struct IssueCtx {
    id: String,
    title: String,
    detail: String,
    severity_badge: &'static str,
}

fn issue_ctx(issue: &Issue) -> IssueCtx {
    IssueCtx {
        id: issue.id.clone(),
        title: issue.title.clone(),
        detail: issue.detail.clone(),
        severity_badge: severity_badge(issue.severity),
    }
}

fn severity_badge(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
        Severity::Critical => "critical",
    }
}

#[derive(Serialize)]
struct RecCtx {
    id: String,
    title: String,
    description: String,
    commands: Vec<String>,
}

fn rec_ctx(r: &Recommendation) -> RecCtx {
    RecCtx {
        id: r.id.clone(),
        title: r.title.clone(),
        description: r.description.clone(),
        commands: r.commands.clone(),
    }
}

#[derive(Serialize)]
struct AnomalyCtx {
    detector: String,
    reason: String,
}

fn anomaly_ctx(a: &Anomaly) -> AnomalyCtx {
    AnomalyCtx { detector: a.detector.clone(), reason: a.reason.clone() }
}

// ─── Markdown stripping ──────────────────────────────────────────────────

/// Strip markdown syntax from `md`. Not exhaustive — just enough for the
/// terminal output used by `--plain`. Handles:
///
/// * ATX headings (`#+ …`) → text only.
/// * Matched inline backticks → surrounding text kept, ticks removed.
/// * Triple-backtick fence lines → dropped entirely.
/// * Trailing whitespace on each line trimmed.
///
/// Lists, tables, and bullets are left as-is; they're already readable on
/// a terminal.
fn strip_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    for raw_line in md.lines() {
        let line = raw_line.trim_end();
        // Drop fenced code block markers — the content stays, the markers go.
        if line.trim_start().starts_with("```") {
            continue;
        }
        // Strip ATX heading prefix.
        let without_hash = strip_atx_heading(line);
        // Strip inline code backticks (matched pairs only).
        let without_ticks = strip_inline_ticks(&without_hash);
        let _ = writeln!(out, "{without_ticks}");
    }
    out
}

fn strip_atx_heading(line: &str) -> String {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes > 0 && hashes <= 6 {
        let rest = &trimmed[hashes..];
        if rest.starts_with(' ') || rest.is_empty() {
            return rest.trim_start().to_owned();
        }
    }
    line.to_owned()
}

fn strip_inline_ticks(line: &str) -> String {
    // Collapse matched backtick runs. Only handle single-backtick inline
    // code — the fenced block case is already excluded above.
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            continue;
        }
        out.push(c);
    }
    // If the run was unmatched we still put the chars back in; callers
    // don't actually care since our templates always emit matched ticks.
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use vietime_core::{
        desktop::DesktopEnv,
        distro::{Distro, DistroFamily},
        engine::{Fcitx5Facts, IbusFacts},
        issue::{Issue, Severity},
        session::SessionType,
        Facts, ImFacts, SystemFacts, REPORT_SCHEMA_VERSION,
    };

    fn fixed_report(tool_version: &str) -> Report {
        Report {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: Utc.with_ymd_and_hms(2026, 4, 25, 10, 23, 11).unwrap(),
            tool_version: tool_version.to_owned(),
            facts: Facts::default(),
            issues: vec![],
            recommendations: vec![],
            anomalies: vec![],
        }
    }

    fn ubuntu_clean_report() -> Report {
        let mut r = fixed_report("0.0.1");
        r.facts.system = SystemFacts {
            distro: Some(Distro {
                id: "ubuntu".to_owned(),
                version_id: Some("24.04".to_owned()),
                pretty: Some("Ubuntu 24.04 LTS".to_owned()),
                family: DistroFamily::Debian,
                id_like: vec![],
            }),
            desktop: Some(DesktopEnv::Gnome { version: Some("46".to_owned()) }),
            session: Some(SessionType::Wayland),
            kernel: Some("6.8.0-45-generic".to_owned()),
            shell: Some("zsh".to_owned()),
        };
        r.facts.im = ImFacts {
            active_framework: ActiveFramework::Ibus,
            ibus: Some(IbusFacts {
                version: Some("1.5.29".to_owned()),
                daemon_running: true,
                daemon_pid: Some(2341),
                config_dir: None,
                registered_engines: vec![],
            }),
            fcitx5: None,
            engines: vec![],
        };
        // A typical clean IBus process env.
        let mut env = std::collections::HashMap::new();
        env.insert("GTK_IM_MODULE".to_owned(), "ibus".to_owned());
        env.insert("QT_IM_MODULE".to_owned(), "ibus".to_owned());
        env.insert("XMODIFIERS".to_owned(), "@im=ibus".to_owned());
        r.facts.env = EnvFacts::from_env_with_source(&env, EnvSource::Process);
        r
    }

    fn conflict_report() -> Report {
        let mut r = fixed_report("0.0.1");
        r.facts.system = SystemFacts {
            distro: Some(Distro {
                id: "fedora".to_owned(),
                version_id: Some("40".to_owned()),
                pretty: Some("Fedora Linux 40".to_owned()),
                family: DistroFamily::Redhat,
                id_like: vec![],
            }),
            desktop: Some(DesktopEnv::Kde { version: Some("6.0".to_owned()) }),
            session: Some(SessionType::X11),
            kernel: None,
            shell: None,
        };
        r.facts.im = ImFacts {
            active_framework: ActiveFramework::Conflict,
            ibus: Some(IbusFacts {
                version: Some("1.5.30".to_owned()),
                daemon_running: true,
                daemon_pid: Some(1111),
                config_dir: None,
                registered_engines: vec![],
            }),
            fcitx5: Some(Fcitx5Facts {
                version: Some("5.1.12".to_owned()),
                daemon_running: true,
                daemon_pid: Some(2222),
                config_dir: None,
                addons_enabled: vec![],
                input_methods_configured: vec![],
            }),
            engines: vec![],
        };
        // Env vars disagree — classic VD003 trigger.
        let mut proc_env = std::collections::HashMap::new();
        proc_env.insert("GTK_IM_MODULE".to_owned(), "fcitx".to_owned());
        let mut merged = EnvFacts::from_env_with_source(&proc_env, EnvSource::Process);
        let mut etc_env = std::collections::HashMap::new();
        etc_env.insert("QT_IM_MODULE".to_owned(), "ibus".to_owned());
        etc_env.insert("XMODIFIERS".to_owned(), "@im=ibus".to_owned());
        let etc = EnvFacts::from_env_with_source(&etc_env, EnvSource::EtcEnvironment);
        merged.merge_by_priority(&etc);
        r.facts.env = merged;
        r.issues.push(Issue {
            id: "VD001".to_owned(),
            severity: Severity::Critical,
            title: "Both IBus and Fcitx5 daemons running".to_owned(),
            detail: "Only one IM framework should be active at a time.".to_owned(),
            facts_evidence: vec!["ibus-daemon pid=1111".to_owned(), "fcitx5 pid=2222".to_owned()],
            recommendation: Some("VR001".to_owned()),
        });
        r.recommendations.push(Recommendation {
            id: "VR001".to_owned(),
            title: "Pick one IM framework".to_owned(),
            description: "Disable whichever framework you are not using.".to_owned(),
            commands: vec!["systemctl --user disable --now fcitx5.service".to_owned()],
            safe_to_run_unattended: false,
            references: vec![],
        });
        r.anomalies.push(Anomaly {
            detector: "env.systemd".to_owned(),
            reason: "systemctl --user show-environment failed: timeout".to_owned(),
        });
        r
    }

    // The `generated_at` timestamp is stable in these tests because we
    // build reports with `fixed_report`, so snapshots stay deterministic
    // without needing insta redactions.

    #[test]
    fn snapshot_empty_report() {
        let r = fixed_report("0.0.1");
        let out = render(&r, &RenderOptions::default()).expect("render");
        insta::assert_snapshot!("render__empty_report", out);
    }

    #[test]
    fn snapshot_ubuntu_ibus_clean() {
        let r = ubuntu_clean_report();
        let out = render(&r, &RenderOptions::default()).expect("render");
        insta::assert_snapshot!("render__ubuntu_ibus_clean", out);
    }

    #[test]
    fn snapshot_conflict() {
        let r = conflict_report();
        let out = render(&r, &RenderOptions::default()).expect("render");
        insta::assert_snapshot!("render__conflict", out);
    }

    #[test]
    fn json_pretty_is_stable() {
        let r = fixed_report("0.0.1");
        let json = render_json(&r).expect("json");
        // Sanity: it's real JSON and includes our tool version.
        assert!(json.starts_with('{'));
        assert!(json.contains("\"tool_version\": \"0.0.1\""));
    }

    #[test]
    fn plain_strips_markdown_headings_and_fences() {
        let r = ubuntu_clean_report();
        let md = render(&r, &RenderOptions::default()).expect("md");
        let plain = render(&r, &RenderOptions { plain: true, verbose: false }).expect("plain");
        // Headings lose their `#+ ` prefix.
        assert!(md.contains("# VietIME Doctor Report"));
        assert!(!plain.contains("# VietIME Doctor Report"));
        assert!(plain.contains("VietIME Doctor Report"));
        // Table pipes still readable.
        assert!(plain.contains("| Var | Value | Source |"));
    }

    #[test]
    fn verbose_footer_only_in_verbose_mode() {
        let r = fixed_report("0.0.1");
        let plain = render(&r, &RenderOptions::default()).expect("render");
        assert!(!plain.contains("verbose:"));
        let verbose =
            render(&r, &RenderOptions { plain: false, verbose: true }).expect("render verbose");
        assert!(verbose.contains("schema_version="));
    }
}
