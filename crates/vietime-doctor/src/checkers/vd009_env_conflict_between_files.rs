// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD009 `EnvConflictBetweenFiles` — Warn.
//
// Fires when the three major IM env vars (GTK_IM_MODULE, QT_IM_MODULE,
// XMODIFIERS) are sourced from **two or more** distinct `EnvSource`
// categories AND disagree on the framework they point at. A user whose
// /etc/environment says `GTK_IM_MODULE=ibus` while ~/.profile exports
// `QT_IM_MODULE=fcitx` will see this fire — the net effect is half-broken
// Vietnamese input and a confusing maintenance story.
//
// Complements VD003 (`EnvVarMismatch`) which is process-env-vs-active-
// framework. VD009 is specifically the "split-brain config file" case:
// the values disagree, AND the disagreement crosses file boundaries.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD009).

use std::collections::HashSet;

use vietime_core::{EnvSource, Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd009;

impl Checker for Vd009 {
    fn id(&self) -> &'static str {
        "VD009"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        let env = &facts.env;
        // Cheap exit: only fire when disagreement *exists* on the major
        // three keys. VD003 handles the process-env path; we only escalate
        // when multiple config files are involved.
        if !env.has_disagreement() {
            return vec![];
        }
        let major_keys = ["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"];
        let mut involved: HashSet<EnvSource> = HashSet::new();
        let mut evidence: Vec<String> = Vec::new();
        for key in major_keys {
            let Some(value) = env.get_by_key(key) else { continue };
            let Some(source) = env.sources.get(key).copied() else { continue };
            if source == EnvSource::Unknown {
                continue;
            }
            involved.insert(source);
            evidence.push(format!("{key}={value} (from {})", source_label(source)));
        }
        if involved.len() < 2 {
            return vec![];
        }
        vec![Issue {
            id: "VD009".to_owned(),
            severity: Severity::Warn,
            title: "IM env vars set across multiple config files with conflicting values"
                .to_owned(),
            detail: "Two or more config files (e.g. /etc/environment and ~/.profile) \
                 each set one of GTK_IM_MODULE / QT_IM_MODULE / XMODIFIERS, \
                 and the values disagree on the input-method framework. \
                 This is fragile: future package updates can silently flip \
                 the resolved framework. Consolidate all three variables \
                 into a single file and point them at the same framework."
                .to_owned(),
            facts_evidence: evidence,
            recommendation: Some("VR009".to_owned()),
        }]
    }
}

fn source_label(s: EnvSource) -> &'static str {
    match s {
        EnvSource::Process => "process env",
        EnvSource::EtcEnvironment => "/etc/environment",
        EnvSource::EtcProfileD => "/etc/profile.d/*.sh",
        EnvSource::HomeProfile => "~/.profile",
        EnvSource::SystemdUserEnv => "systemd --user",
        EnvSource::Pam => "pam",
        EnvSource::Unknown => "unknown",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vietime_core::EnvFacts;

    fn merged(process: &[(&str, &str)], etc: &[(&str, &str)]) -> EnvFacts {
        let p: HashMap<String, String> =
            process.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let e: HashMap<String, String> =
            etc.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let mut out = EnvFacts::from_env_with_source(&p, EnvSource::Process);
        let other = EnvFacts::from_env_with_source(&e, EnvSource::EtcEnvironment);
        out.merge_by_priority(&other);
        out
    }

    #[test]
    fn fires_on_two_sources_with_disagreement() {
        // GTK via /etc/environment → fcitx, QT via process → ibus.
        // Process is higher priority so GTK stays from etc, QT is process.
        let env = merged(&[("QT_IM_MODULE", "ibus")], &[("GTK_IM_MODULE", "fcitx")]);
        let facts = Facts { env, ..Facts::default() };
        let out = Vd009.check(&facts);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warn);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR009"));
    }

    #[test]
    fn silent_when_single_source() {
        // Both vars from /etc/environment — same file, same source.
        let env = merged(&[], &[("GTK_IM_MODULE", "ibus"), ("QT_IM_MODULE", "fcitx")]);
        let facts = Facts { env, ..Facts::default() };
        assert!(Vd009.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_aligned_even_across_sources() {
        // Both point at ibus — no disagreement, no fire regardless of sources.
        let env = merged(&[("GTK_IM_MODULE", "ibus")], &[("QT_IM_MODULE", "ibus")]);
        let facts = Facts { env, ..Facts::default() };
        assert!(Vd009.check(&facts).is_empty());
    }

    #[test]
    fn silent_when_no_im_vars() {
        let facts = Facts::default();
        assert!(Vd009.check(&facts).is_empty());
    }
}
