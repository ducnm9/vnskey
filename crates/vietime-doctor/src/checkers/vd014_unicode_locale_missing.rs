// SPDX-License-Identifier: GPL-3.0-or-later
//
// VD014 `UnicodeLocaleMissing` — Warn.
//
// Fires when the effective locale is set to a non-UTF-8 encoding (e.g.
// `en_US.ISO-8859-1` or `C` / `POSIX`). Vietnamese input is defined in the
// IMs as UTF-8 output; a non-UTF-8 locale causes commit strings to be
// mangled or silently dropped at the libc boundary.
//
// The heuristic is deliberately loose: we accept any string that contains
// `utf-8` or `utf8` (case-insensitive) as UTF-8. Everything else,
// including the very common "C"/"POSIX" fallback, trips the warning.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.4 (VD014).

use vietime_core::{Facts, Issue, Severity};

use crate::checker::Checker;

/// See module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Vd014;

impl Checker for Vd014 {
    fn id(&self) -> &'static str {
        "VD014"
    }

    fn check(&self, facts: &Facts) -> Vec<Issue> {
        let Some(locale) = facts.system.locale.as_deref() else {
            // LocaleDetector said "nothing set". That's a separate
            // problem and libc falls back to C — still non-UTF-8, so fire.
            return vec![emit_issue("(unset)", "no LC_ALL / LC_CTYPE / LANG set")];
        };
        if is_utf8(locale) {
            return vec![];
        }
        vec![emit_issue(locale, "effective locale is not a UTF-8 one")]
    }
}

fn emit_issue(locale: &str, why: &str) -> Issue {
    Issue {
        id: "VD014".to_owned(),
        severity: Severity::Warn,
        title: "Active locale is not UTF-8".to_owned(),
        detail: "Vietnamese engines emit UTF-8 commit strings; a non-UTF-8 \
             locale (e.g. `C`, `POSIX`, or an ISO-8859 variant) mangles the \
             output at the libc boundary. Set LANG to a `*.UTF-8` variant \
             in `/etc/locale.conf` or your shell profile."
            .to_owned(),
        facts_evidence: vec![format!("locale: {locale}"), why.to_owned()],
        recommendation: Some("VR014".to_owned()),
    }
}

fn is_utf8(locale: &str) -> bool {
    let lower = locale.to_ascii_lowercase();
    lower.contains("utf-8") || lower.contains("utf8")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vietime_core::SystemFacts;

    fn facts_with_locale(locale: Option<&str>) -> Facts {
        Facts {
            system: SystemFacts { locale: locale.map(str::to_owned), ..SystemFacts::default() },
            ..Facts::default()
        }
    }

    #[test]
    fn silent_on_en_us_utf_8() {
        assert!(Vd014.check(&facts_with_locale(Some("en_US.UTF-8"))).is_empty());
    }

    #[test]
    fn silent_on_vi_vn_utf8_no_dash() {
        assert!(Vd014.check(&facts_with_locale(Some("vi_VN.utf8"))).is_empty());
    }

    #[test]
    fn fires_on_c_locale() {
        let out = Vd014.check(&facts_with_locale(Some("C")));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].recommendation.as_deref(), Some("VR014"));
    }

    #[test]
    fn fires_on_posix_locale() {
        assert_eq!(Vd014.check(&facts_with_locale(Some("POSIX"))).len(), 1);
    }

    #[test]
    fn fires_on_iso_latin1_locale() {
        assert_eq!(Vd014.check(&facts_with_locale(Some("en_US.ISO-8859-1"))).len(), 1);
    }

    #[test]
    fn fires_on_unset_locale() {
        let out = Vd014.check(&facts_with_locale(None));
        assert_eq!(out.len(), 1);
        assert!(out[0].facts_evidence.iter().any(|e| e.contains("unset")));
    }
}
