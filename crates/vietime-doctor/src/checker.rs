// SPDX-License-Identifier: GPL-3.0-or-later
//
// Checker trait — pure synchronous `(&Facts) -> Vec<Issue>` pass.
//
// Checkers run after every Detector has finished and the Orchestrator has
// assembled a complete `Facts` tree. Unlike Detectors they have no I/O, no
// timeouts, and no panic-to-anomaly handling: the input is already in
// memory, the work is pure, and any panic is a Doctor bug we want to crash
// on during testing.
//
// Week 5 ships 9 checkers (VD001-VD008 + VD012); VD009-VD015 land with the
// Week 6 polish batch.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4.

use std::sync::Arc;

use vietime_core::{Facts, Issue};

/// A single check run against the fully-assembled `Facts` tree.
///
/// Implementations are zero-sized structs (`pub struct Vd001;`) — there's
/// no per-run state. The `&Facts` input gives each checker the whole report
/// (system + IM + env + apps) so cross-section triggers (e.g. "Wayland
/// session AND IBus active") are a single pattern-match.
pub trait Checker: Send + Sync + std::fmt::Debug {
    /// Stable id matching the `VD###` family (e.g. `"VD001"`). Used by
    /// `list` subcommand output and debug tracing. The wire-visible
    /// identifier on an emitted `Issue` is `Issue::id` — kept in sync by
    /// convention, but the trait doesn't enforce it.
    fn id(&self) -> &'static str;

    /// The checker's verdict. Returns an empty `Vec` when the trigger
    /// doesn't fire. Most checkers return 0 or 1 issues; VD005 / VD007 /
    /// VD008 may emit one issue per matching row (per-engine, per-app).
    fn check(&self, facts: &Facts) -> Vec<Issue>;
}

/// Run every registered checker against `facts`, flatten the resulting
/// issues, and sort them by `(severity descending, id ascending)` so the
/// highest-severity row surfaces first and ordering is deterministic.
///
/// Empty input (no checkers registered) is a valid call — returns `vec![]`.
#[must_use]
pub fn run_checkers(checkers: &[Arc<dyn Checker>], facts: &Facts) -> Vec<Issue> {
    let mut issues: Vec<Issue> = checkers.iter().flat_map(|c| c.check(facts)).collect();
    // Descending severity first (Critical > Error > Warn > Info), then by
    // id ascending so equal-severity rows stay in registration order-ish.
    issues.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
    issues
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::{Issue, Severity};

    fn mk_issue(id: &str, sev: Severity) -> Issue {
        Issue {
            id: id.to_owned(),
            severity: sev,
            title: id.to_owned(),
            detail: String::new(),
            facts_evidence: vec![],
            recommendation: None,
        }
    }

    #[derive(Debug)]
    struct FakeChecker {
        id: &'static str,
        out: Vec<Issue>,
    }

    impl Checker for FakeChecker {
        fn id(&self) -> &'static str {
            self.id
        }
        fn check(&self, _facts: &Facts) -> Vec<Issue> {
            self.out.clone()
        }
    }

    #[test]
    fn empty_checker_list_produces_no_issues() {
        let facts = Facts::default();
        assert!(run_checkers(&[], &facts).is_empty());
    }

    #[test]
    fn issues_sort_by_severity_desc_then_id_asc() {
        let a: Arc<dyn Checker> =
            Arc::new(FakeChecker { id: "fake.a", out: vec![mk_issue("VD004", Severity::Warn)] });
        let b: Arc<dyn Checker> = Arc::new(FakeChecker {
            id: "fake.b",
            out: vec![mk_issue("VD001", Severity::Critical)],
        });
        let c: Arc<dyn Checker> =
            Arc::new(FakeChecker { id: "fake.c", out: vec![mk_issue("VD012", Severity::Info)] });
        let facts = Facts::default();
        let out = run_checkers(&[a, b, c], &facts);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, "VD001"); // Critical first
        assert_eq!(out[1].id, "VD004"); // Warn next
        assert_eq!(out[2].id, "VD012"); // Info last
    }

    #[test]
    fn checker_with_empty_output_contributes_nothing() {
        let a: Arc<dyn Checker> = Arc::new(FakeChecker {
            id: "fake.a",
            out: vec![mk_issue("VD001", Severity::Critical)],
        });
        let b: Arc<dyn Checker> = Arc::new(FakeChecker { id: "fake.b", out: vec![] });
        let facts = Facts::default();
        let out = run_checkers(&[a, b], &facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "VD001");
    }

    #[test]
    fn multi_issue_checker_yields_all_issues() {
        // VD005 fires once per installed-not-registered engine; simulate
        // two such issues from a single checker.
        let ck: Arc<dyn Checker> = Arc::new(FakeChecker {
            id: "VD005",
            out: vec![mk_issue("VD005", Severity::Warn), mk_issue("VD005", Severity::Warn)],
        });
        let facts = Facts::default();
        let out = run_checkers(&[ck], &facts);
        assert_eq!(out.len(), 2);
    }
}
