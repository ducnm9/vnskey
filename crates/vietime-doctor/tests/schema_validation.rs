// SPDX-License-Identifier: GPL-3.0-or-later
//
// Validates that every JSON report we can produce matches the published
// `schemas/report.v1.json` file.
//
// We don't pull an external schema-validator crate (would add a license /
// bans footprint for one test binary). Instead we assert the structural
// invariants that the JSON Schema encodes, by inspecting both the Schema
// document itself and a real rendered report:
//
// * the schema file parses as JSON and has `$id` + `schema_version` const;
// * every top-level required field in the schema is present in a real report;
// * `schema_version` in output matches the `const` in the schema;
// * every `issues[].id` matches `^VD[0-9]{3}$` and every
//   `recommendations[].id` matches `^VR[0-9]{3}$`;
// * every `issues[].severity` is one of the published enum values;
// * every `EnvFacts.sources[*]` value is a published `EnvSource` enum value.
//
// CI also runs `scripts/check-report-schema.sh` on the host (which uses
// `check-jsonschema` when available) — that script is the "real" validator;
// the Rust test is the last-line defense that survives a missing Python.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.14.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/vietime-doctor. Walk up two levels.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn schema_path() -> PathBuf {
    repo_root().join("schemas").join("report.v1.json")
}

fn load_schema() -> Value {
    let raw = std::fs::read_to_string(schema_path()).expect("schemas/report.v1.json must exist");
    serde_json::from_str(&raw).expect("schema file must be valid JSON")
}

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vietime-doctor"))
}

fn report_json() -> Value {
    let out = Command::new(binary_path())
        .args(["report", "--json"])
        .output()
        .expect("spawn vietime-doctor report --json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&stdout).expect("report --json must emit valid JSON")
}

#[test]
fn schema_file_is_well_formed() {
    let schema = load_schema();
    assert_eq!(schema["$schema"], "https://json-schema.org/draft/2020-12/schema");
    assert_eq!(schema["$id"], "https://vietime.io/schemas/report.v1.json");
    // The outer schema pins the version to 1 via `const`.
    assert_eq!(
        schema["properties"]["schema_version"]["const"], 1,
        "schema_version must be pinned to 1 in v1 schema",
    );
}

#[test]
fn schema_declares_every_public_def() {
    // Sanity check that the obvious types are all defined. If we add a new
    // field to `Report` / `Facts`, this test reminds the author to bump the
    // schema.
    let schema = load_schema();
    let defs = schema["$defs"].as_object().expect("schema needs $defs");
    for expected in [
        "Severity",
        "ImFramework",
        "SessionType",
        "ActiveFramework",
        "DistroFamily",
        "Distro",
        "DesktopEnv",
        "EnvSource",
        "EnvFacts",
        "SystemFacts",
        "EngineFact",
        "IbusFacts",
        "Fcitx5Facts",
        "ImFacts",
        "AppKind",
        "AppFacts",
        "Facts",
        "Issue",
        "Recommendation",
        "Anomaly",
    ] {
        assert!(defs.contains_key(expected), "schema $defs is missing `{expected}`");
    }
}

#[test]
fn report_contains_every_top_level_required_field() {
    let schema = load_schema();
    let required: Vec<String> = schema["required"]
        .as_array()
        .expect("top-level required[]")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();

    let report = report_json();
    let obj = report.as_object().expect("report is a JSON object");
    for key in &required {
        assert!(
            obj.contains_key(key),
            "report is missing required top-level key `{key}`\nreport: {report:#}"
        );
    }
}

#[test]
fn report_schema_version_matches_schema_const() {
    let report = report_json();
    assert_eq!(report["schema_version"], 1, "report.schema_version must match the schema's const");
}

#[test]
fn every_issue_id_matches_vd_pattern() {
    let report = report_json();
    let empty = vec![];
    let issues = report["issues"].as_array().unwrap_or(&empty);
    for issue in issues {
        let id = issue["id"].as_str().expect("issue.id is string");
        assert!(
            id.starts_with("VD") && id.len() == 5 && id[2..].chars().all(|c| c.is_ascii_digit()),
            "issue id `{id}` doesn't match ^VD[0-9]{{3}}$"
        );
        let sev = issue["severity"].as_str().expect("severity");
        assert!(
            ["info", "warn", "error", "critical"].contains(&sev),
            "unknown severity `{sev}` in issue {id}"
        );
        if let Some(vr) = issue.get("recommendation").and_then(Value::as_str) {
            assert!(
                vr.starts_with("VR")
                    && vr.len() == 5
                    && vr[2..].chars().all(|c| c.is_ascii_digit()),
                "recommendation id `{vr}` on {id} doesn't match ^VR[0-9]{{3}}$"
            );
        }
    }
}

#[test]
fn every_recommendation_id_matches_vr_pattern() {
    let report = report_json();
    let empty = vec![];
    let recs = report["recommendations"].as_array().unwrap_or(&empty);
    for rec in recs {
        let id = rec["id"].as_str().expect("rec.id is string");
        assert!(
            id.starts_with("VR") && id.len() == 5 && id[2..].chars().all(|c| c.is_ascii_digit()),
            "recommendation id `{id}` doesn't match ^VR[0-9]{{3}}$"
        );
        assert!(rec["commands"].is_array(), "recommendation.commands must be array");
        assert!(
            rec["safe_to_run_unattended"].is_boolean(),
            "recommendation.safe_to_run_unattended must be bool"
        );
    }
}

#[test]
fn env_sources_only_use_published_enum_values() {
    let report = report_json();
    let sources = &report["facts"]["env"]["sources"];
    let Some(map) = sources.as_object() else {
        // Empty facts.env — nothing to check, still a valid report shape.
        return;
    };
    let allowed = [
        "process",
        "etc_environment",
        "etc_profile_d",
        "home_profile",
        "systemd_user_env",
        "pam",
        "unknown",
    ];
    for (key, val) in map {
        let source = val.as_str().expect("env source is string");
        assert!(
            allowed.contains(&source),
            "env.sources[{key}] = `{source}` not in published EnvSource enum"
        );
    }
}

#[test]
fn active_framework_is_one_of_published_values() {
    let report = report_json();
    let active = report["facts"]["im"]["active_framework"].as_str().expect("active_framework");
    assert!(
        ["none", "ibus", "fcitx5", "conflict"].contains(&active),
        "active_framework `{active}` not one of published values"
    );
}

#[test]
fn session_if_present_is_one_of_published_values() {
    let report = report_json();
    if let Some(sess) = report["facts"]["system"]["session"].as_str() {
        assert!(
            ["x11", "wayland", "tty", "unknown"].contains(&sess),
            "session `{sess}` not one of published SessionType values"
        );
    }
}
