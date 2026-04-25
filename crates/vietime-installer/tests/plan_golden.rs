// SPDX-License-Identifier: GPL-3.0-or-later
//
// Golden plan test (INS-04).
//
// Locks down the exact shape of `plan(Install(fcitx5-bamboo))` on the
// Ubuntu 24.04 fixture so any future refactor that changes the step list
// surfaces as an insta snapshot diff. The Plan's `id` (random UUID) and
// `generated_at` (wall clock) are zeroed out before snapshotting so the
// snapshot is reproducible across machines and runs.
//
// Spec ref: `spec/02-phase2-installer.md` §B.3.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::TimeZone;
use uuid::Uuid;
use vietime_core::ImFramework;
use vietime_installer::{plan, Combo, Engine, Goal, Plan, PreState};

/// Strip non-deterministic fields (`id`, `generated_at`) so the golden can
/// survive across runs and CI machines.
fn canonicalise(mut plan: Plan) -> Plan {
    plan.id = Uuid::nil();
    plan.generated_at = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    plan
}

#[test]
fn install_fcitx5_bamboo_on_ubuntu_24_04_matches_golden() {
    let pre = PreState::fixture_ubuntu_24_04();
    let goal = Goal::Install { combo: Combo::new(ImFramework::Fcitx5, Engine::Bamboo) };
    let built = plan(pre, goal).expect("planner must succeed for a supported combo");
    let frozen = canonicalise(built);

    // Use TOML here (rather than JSON) because TOML is the wire format the
    // Executor will eventually write to `manifest.toml`, so the golden
    // doubles as a human-readable reference for that file.
    let toml_text = toml::to_string(&frozen).expect("plan must serialise to TOML");
    insta::assert_snapshot!("install_fcitx5_bamboo_ubuntu_24_04", toml_text);
}
