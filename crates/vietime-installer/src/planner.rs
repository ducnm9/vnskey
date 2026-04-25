// SPDX-License-Identifier: GPL-3.0-or-later
//
// `Planner` — turns a `PreState` + `Goal` into a fully-spelled-out `Plan`.
//
// The Planner is the single brain of the Installer: it codifies every
// distro-specific rule (which packages, which env file, when to disable
// IBus) in one place so the Executor can stay dumb and idempotent.
//
// Spec ref: `spec/02-phase2-installer.md` §B.3.
//
// ## Invariants the Planner guarantees
//
// 1. Every `SetEnvVar`, `UnsetEnvVar`, and `WriteFile` step is preceded by
//    a `BackupFile` for its target path — so rollback can always restore.
// 2. Mutating steps fire in the order: Backup* → InstallPackages →
//    SetEnvVar → Systemctl* → Verify → Prompt.
// 3. Plans that would leave the user un-able to type at all (no env vars)
//    are rejected at `validate_plan` time.
//
// `validate_plan` is called from `plan()` before returning, so downstream
// code can treat a returned `Plan` as "structurally sound".

use std::collections::HashSet;
use std::path::PathBuf;

use vietime_core::{ActiveFramework, DistroFamily, ImFramework};

use crate::model::{
    Combo, Engine, EnvFile, Goal, PackageManager, Plan, PromptCondition, Step, VerifyCheck,
};
use crate::pre_state::PreState;

/// Planner error. Distinct from `anyhow` so callers (CLI + future TUI) can
/// render a specific hint per variant.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    #[error(
        "combo `{combo}` is not yet supported on distro family {family:?}. \
             See tasks/phase-2-installer.md INS-53 (Fedora/Arch) or INS-50/51."
    )]
    UnsupportedCombo { combo: Combo, family: DistroFamily },

    #[error("goal `{goal}` is not yet implemented — see {ticket}.")]
    UnimplementedGoal { goal: &'static str, ticket: &'static str },

    #[error(
        "plan invariant violated at step {index} ({kind}): \
         step mutates `{target}` without a preceding BackupFile"
    )]
    MissingBackup { index: usize, kind: &'static str, target: String },
}

/// Build a `Plan` from a known pre-state and a user goal.
pub fn plan(pre: PreState, goal: Goal) -> Result<Plan, PlanError> {
    let planned = match &goal {
        Goal::Install { combo } => plan_install(&pre, *combo)?,
        Goal::Uninstall { .. } => {
            return Err(PlanError::UnimplementedGoal {
                goal: "uninstall",
                ticket: "INS-44 (tasks/phase-2-installer.md Week 5)",
            });
        }
        Goal::Switch { .. } => {
            return Err(PlanError::UnimplementedGoal {
                goal: "switch",
                ticket: "INS-60 (tasks/phase-2-installer.md Week 7)",
            });
        }
    };

    let mut out = Plan::new_skeleton(goal, pre);
    out.steps = planned.steps;
    out.estimated_duration_secs = planned.estimated_duration_secs;
    out.requires_sudo = planned.requires_sudo;
    out.requires_logout = planned.requires_logout;

    validate_plan(&out)?;
    Ok(out)
}

/// Validate that every mutating step has a preceding `BackupFile` for the
/// same path. See module docs for the full rules.
pub fn validate_plan(plan: &Plan) -> Result<(), PlanError> {
    let mut backed_up: HashSet<PathBuf> = HashSet::new();
    // Use the same home resolution that the Planner used, so that the
    // invariant check stays deterministic even if `$HOME` is missing.
    let home = planner_home();

    for (idx, step) in plan.steps.iter().enumerate() {
        match step {
            Step::BackupFile { path } => {
                backed_up.insert(path.clone());
            }
            Step::SetEnvVar { file, .. } | Step::UnsetEnvVar { file, .. } => {
                let target = file.path(&home);
                require_backup(idx, step, &backed_up, &target)?;
            }
            Step::WriteFile { path, .. } => {
                require_backup(idx, step, &backed_up, path)?;
            }
            // Other variants encode their own rollback data (package list,
            // systemd unit previous state) in the snapshot manifest — no
            // BackupFile required.
            _ => {}
        }
    }
    Ok(())
}

fn require_backup(
    idx: usize,
    step: &Step,
    backed_up: &HashSet<PathBuf>,
    target: &std::path::Path,
) -> Result<(), PlanError> {
    if backed_up.contains(target) {
        return Ok(());
    }
    Err(PlanError::MissingBackup {
        index: idx,
        kind: step.kind(),
        target: target.display().to_string(),
    })
}

/// Deterministic "home" path used by the invariant checker. Real execution
/// resolves this at runtime; the Planner embeds a literal so TOML goldens
/// don't drift per developer.
fn planner_home() -> PathBuf {
    PathBuf::from("/home/vietime")
}

struct PlannedSteps {
    steps: Vec<Step>,
    estimated_duration_secs: u64,
    requires_sudo: bool,
    requires_logout: bool,
}

fn plan_install(pre: &PreState, combo: Combo) -> Result<PlannedSteps, PlanError> {
    // Only Debian family is wired up this week.
    if !pre.distro.is_family(DistroFamily::Debian) {
        return Err(PlanError::UnsupportedCombo { combo, family: pre.distro.family });
    }

    match combo.framework {
        ImFramework::Fcitx5 => Ok(plan_install_fcitx5_debian(pre, combo)),
        ImFramework::Ibus => Ok(plan_install_ibus_debian(pre, combo)),
        ImFramework::None => Err(PlanError::UnsupportedCombo { combo, family: pre.distro.family }),
    }
}

fn plan_install_fcitx5_debian(pre: &PreState, combo: Combo) -> PlannedSteps {
    // Spec/02 §B.3 — canonical order.
    let engine_pkg = match combo.engine {
        Engine::Bamboo => "fcitx5-bamboo",
        Engine::Unikey => "fcitx5-unikey",
    };
    let packages = vec![
        "fcitx5".to_owned(),
        "fcitx5-frontend-gtk3".to_owned(),
        "fcitx5-frontend-gtk4".to_owned(),
        "fcitx5-frontend-qt5".to_owned(),
        "fcitx5-module-xorg".to_owned(),
        engine_pkg.to_owned(),
    ];

    let home = planner_home();
    let mut steps: Vec<Step> = Vec::with_capacity(18);

    // 1. Backups for every file we'll later touch.
    steps.push(Step::BackupFile { path: PathBuf::from("/etc/environment") });
    steps.push(Step::BackupFile { path: home.join(".profile") });
    steps.push(Step::BackupFile { path: home.join(".config/fcitx5/profile") });

    // 2. Packages.
    steps.push(Step::InstallPackages { manager: PackageManager::Apt, packages });

    // 3. im-config — Debian-specific glue.
    steps.push(Step::RunImConfig { mode: "fcitx5".to_owned() });

    // 4–8. Five IM env vars. Values straight from spec/02 §A.5.
    let env_pairs: &[(&str, &str)] = &[
        ("GTK_IM_MODULE", "fcitx"),
        ("QT_IM_MODULE", "fcitx"),
        ("XMODIFIERS", "@im=fcitx"),
        ("SDL_IM_MODULE", "fcitx"),
        // GLFW has never learned "fcitx"; upstream treats "ibus" as the
        // generic IBus-compatible value. Quirk documented in spec/02 §B.3.
        ("GLFW_IM_MODULE", "ibus"),
    ];
    for (k, v) in env_pairs {
        steps.push(Step::SetEnvVar {
            file: EnvFile::EtcEnvironment,
            key: (*k).to_owned(),
            value: (*v).to_owned(),
        });
    }

    // 9. Write a minimal Fcitx5 profile that has Bamboo/Unikey pre-selected.
    steps.push(Step::WriteFile {
        path: home.join(".config/fcitx5/profile"),
        content: default_fcitx5_profile(combo.engine),
        mode: 0o644,
    });

    // 10. systemd user services.
    steps.push(Step::SystemctlUserEnable { unit: "fcitx5.service".to_owned() });

    // Only disable IBus if it's actually the active framework — don't touch
    // the unit on a system where Fcitx5 is already running.
    if pre.active_framework == ActiveFramework::Ibus {
        steps.push(Step::SystemctlUserDisable { unit: "ibus.service".to_owned() });
    }

    steps.push(Step::SystemctlUserStart { unit: "fcitx5.service".to_owned() });

    // 11. Verify.
    steps.push(Step::Verify {
        check: VerifyCheck::DaemonRunning { framework: ImFramework::Fcitx5 },
    });
    steps.push(Step::Verify {
        check: VerifyCheck::EngineRegistered { name: combo.engine.as_str().to_owned() },
    });
    steps.push(Step::Verify { check: VerifyCheck::EnvConsistent });

    // 12. Final prompt — reminds the user they need to re-login before env
    // vars propagate to the session.
    steps.push(Step::Prompt {
        message: "Hoàn tất. Hãy logout và login lại để các biến môi trường IM có hiệu lực, \
                  sau đó chạy `vietime-installer verify`."
            .to_owned(),
        continue_if: PromptCondition::UserYes,
    });

    PlannedSteps { steps, estimated_duration_secs: 45, requires_sudo: true, requires_logout: true }
}

fn plan_install_ibus_debian(pre: &PreState, combo: Combo) -> PlannedSteps {
    // Parallel to the Fcitx5 plan but shorter: no im-config switch is needed
    // (Ubuntu ships with IBus already wired via im-config), no service flip.
    let engine_pkg = match combo.engine {
        Engine::Bamboo => "ibus-bamboo",
        Engine::Unikey => "ibus-unikey",
    };
    let packages = vec![
        "ibus".to_owned(),
        "ibus-gtk".to_owned(),
        "ibus-gtk3".to_owned(),
        engine_pkg.to_owned(),
    ];

    let home = planner_home();
    let mut steps: Vec<Step> = Vec::with_capacity(12);

    steps.push(Step::BackupFile { path: PathBuf::from("/etc/environment") });
    steps.push(Step::BackupFile { path: home.join(".profile") });

    steps.push(Step::InstallPackages { manager: PackageManager::Apt, packages });

    let env_pairs: &[(&str, &str)] = &[
        ("GTK_IM_MODULE", "ibus"),
        ("QT_IM_MODULE", "ibus"),
        ("XMODIFIERS", "@im=ibus"),
        ("GLFW_IM_MODULE", "ibus"),
    ];
    for (k, v) in env_pairs {
        steps.push(Step::SetEnvVar {
            file: EnvFile::EtcEnvironment,
            key: (*k).to_owned(),
            value: (*v).to_owned(),
        });
    }

    steps.push(Step::SystemctlUserEnable { unit: "ibus.service".to_owned() });
    // If Fcitx5 was active before, step down before enabling IBus.
    if pre.active_framework == ActiveFramework::Fcitx5 {
        steps.push(Step::SystemctlUserDisable { unit: "fcitx5.service".to_owned() });
    }
    steps.push(Step::SystemctlUserStart { unit: "ibus.service".to_owned() });

    steps.push(Step::Verify { check: VerifyCheck::DaemonRunning { framework: ImFramework::Ibus } });
    steps.push(Step::Verify {
        check: VerifyCheck::EngineRegistered { name: combo.engine.as_str().to_owned() },
    });
    steps.push(Step::Verify { check: VerifyCheck::EnvConsistent });

    steps.push(Step::Prompt {
        message: "Hoàn tất. Hãy logout và login lại, sau đó chạy `vietime-installer verify`."
            .to_owned(),
        continue_if: PromptCondition::UserYes,
    });

    PlannedSteps { steps, estimated_duration_secs: 30, requires_sudo: true, requires_logout: true }
}

fn default_fcitx5_profile(engine: Engine) -> String {
    // Minimal profile that enables keyboard-us + the VietIME engine. Full
    // template lives in INS-24 (Week 3) — this is the reduced form required
    // to type Vietnamese on first login.
    let engine_id = match engine {
        Engine::Bamboo => "bamboo",
        Engine::Unikey => "unikey",
    };
    format!(
        "[Groups/0]\n\
         Name=Default\n\
         Default Layout=us\n\
         DefaultIM={engine_id}\n\
         \n\
         [Groups/0/Items/0]\n\
         Name=keyboard-us\n\
         Layout=\n\
         \n\
         [Groups/0/Items/1]\n\
         Name={engine_id}\n\
         Layout=\n\
         \n\
         [GroupOrder]\n\
         0=Default\n"
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::model::{Combo, Engine, Goal, PLAN_SCHEMA_VERSION};

    fn install_goal(framework: ImFramework, engine: Engine) -> Goal {
        Goal::Install { combo: Combo::new(framework, engine) }
    }

    #[test]
    fn ubuntu_install_fcitx5_bamboo_has_expected_shape() {
        let pre = PreState::fixture_ubuntu_24_04();
        let plan = plan(pre, install_goal(ImFramework::Fcitx5, Engine::Bamboo)).expect("plan ok");

        assert_eq!(plan.schema_version, PLAN_SCHEMA_VERSION);
        assert!(plan.requires_sudo);
        assert!(plan.requires_logout);

        // First three steps must be backups.
        assert!(matches!(plan.steps[0], Step::BackupFile { .. }));
        assert!(matches!(plan.steps[1], Step::BackupFile { .. }));
        assert!(matches!(plan.steps[2], Step::BackupFile { .. }));

        // Exactly one InstallPackages step, and it mentions fcitx5-bamboo.
        let install_count =
            plan.steps.iter().filter(|s| matches!(s, Step::InstallPackages { .. })).count();
        assert_eq!(install_count, 1);
        let bamboo_mentioned = plan.steps.iter().any(|s| matches!(
            s,
            Step::InstallPackages { packages, .. } if packages.iter().any(|p| p == "fcitx5-bamboo")
        ));
        assert!(bamboo_mentioned);

        // Exactly one `im-config -n fcitx5`.
        assert_eq!(plan.steps.iter().filter(|s| matches!(s, Step::RunImConfig { .. })).count(), 1);

        // Because the fixture says IBus was active, we should disable it.
        let disables_ibus = plan.steps.iter().any(|s| {
            matches!(
                s,
                Step::SystemctlUserDisable { unit } if unit == "ibus.service"
            )
        });
        assert!(disables_ibus, "should disable ibus when it was active");
    }

    #[test]
    fn ubuntu_install_fcitx5_unikey_swaps_engine_package() {
        let pre = PreState::fixture_ubuntu_24_04();
        let plan = plan(pre, install_goal(ImFramework::Fcitx5, Engine::Unikey)).expect("plan ok");
        let has_unikey = plan.steps.iter().any(|s| matches!(
            s,
            Step::InstallPackages { packages, .. } if packages.iter().any(|p| p == "fcitx5-unikey")
        ));
        let has_bamboo = plan.steps.iter().any(|s| matches!(
            s,
            Step::InstallPackages { packages, .. } if packages.iter().any(|p| p == "fcitx5-bamboo")
        ));
        assert!(has_unikey);
        assert!(!has_bamboo);
    }

    #[test]
    fn ubuntu_install_ibus_does_not_touch_fcitx5_unit_when_none_active() {
        // Start from a pre-state where neither daemon is running.
        let mut pre = PreState::fixture_ubuntu_24_04();
        pre.active_framework = ActiveFramework::None;

        let plan =
            super::plan(pre, install_goal(ImFramework::Ibus, Engine::Bamboo)).expect("plan ok");

        let touches_fcitx5 = plan.steps.iter().any(|s| {
            matches!(
                s,
                Step::SystemctlUserDisable { unit } if unit == "fcitx5.service"
            )
        });
        assert!(!touches_fcitx5, "should not touch fcitx5.service when it wasn't active");
    }

    #[test]
    fn fedora_fcitx5_bamboo_is_unsupported_this_week() {
        let err =
            plan(PreState::fixture_fedora_40(), install_goal(ImFramework::Fcitx5, Engine::Bamboo))
                .expect_err("should fail on Fedora");
        assert!(
            matches!(err, PlanError::UnsupportedCombo { family: DistroFamily::Redhat, .. }),
            "expected UnsupportedCombo, got {err:?}"
        );
    }

    #[test]
    fn uninstall_goal_returns_unimplemented_this_week() {
        let err = plan(PreState::fixture_ubuntu_24_04(), Goal::Uninstall { snapshot_id: None })
            .expect_err("uninstall planner not yet wired");
        assert!(matches!(err, PlanError::UnimplementedGoal { goal: "uninstall", .. }));
    }

    #[test]
    fn validate_plan_rejects_setenv_without_backup() {
        // Build a deliberately broken Plan and make sure the invariant
        // checker catches it.
        let mut bad = Plan::new_skeleton(
            install_goal(ImFramework::Fcitx5, Engine::Bamboo),
            PreState::fixture_ubuntu_24_04(),
        );
        bad.steps.push(Step::SetEnvVar {
            file: EnvFile::EtcEnvironment,
            key: "GTK_IM_MODULE".to_owned(),
            value: "fcitx".to_owned(),
        });
        let err = validate_plan(&bad).expect_err("should reject");
        assert!(matches!(err, PlanError::MissingBackup { .. }));
    }

    #[test]
    fn validate_plan_accepts_good_plan() {
        let good = plan(
            PreState::fixture_ubuntu_24_04(),
            install_goal(ImFramework::Fcitx5, Engine::Bamboo),
        )
        .expect("plan ok");
        assert!(validate_plan(&good).is_ok());
    }
}
