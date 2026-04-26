// SPDX-License-Identifier: GPL-3.0-or-later
//
// `sys.locale` — reads the effective locale from POSIX env-var precedence.
//
// Rule per POSIX / `locale(7)`: LC_ALL wins if set, else the specific
// category (we care about LC_CTYPE because it governs character encoding —
// which is the signal VD014 gates on), else LANG. Everything beyond that is
// locale-category fallback that doesn't change the UTF-8 verdict.
//
// We do NOT shell out to `locale` here: the binary is slow, its output is
// distro-variable, and all we need is the string itself. The Week-6
// `UnicodeLocaleMissing` checker (VD014) will parse the string to decide
// whether the active locale is a UTF-8 one.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3, §B.4 VD014.

use std::time::Duration;

use async_trait::async_trait;

use crate::detector::{Detector, DetectorContext, DetectorOutput, DetectorResult, PartialFacts};

#[derive(Debug, Default)]
pub struct LocaleDetector;

impl LocaleDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Detector for LocaleDetector {
    fn id(&self) -> &'static str {
        "sys.locale"
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(50)
    }

    async fn run(&self, ctx: &DetectorContext) -> DetectorResult {
        // Treat an empty-string env var as unset — that's how libc's
        // `setlocale(3)` behaves (LC_ALL="" falls through to LC_CTYPE
        // / LANG). Otherwise a stray `export LC_ALL=` in a shell
        // profile would silently nullify the detector.
        let pick = |key: &str| ctx.env.get(key).filter(|v| !v.is_empty()).cloned();
        let locale = pick("LC_ALL").or_else(|| pick("LC_CTYPE")).or_else(|| pick("LANG"));
        let mut notes = Vec::new();
        if let Some(ref l) = locale {
            notes.push(format!("effective locale: {l}"));
        } else {
            notes.push("no LC_ALL / LC_CTYPE / LANG set".to_owned());
        }
        Ok(DetectorOutput { partial: PartialFacts { locale, ..PartialFacts::default() }, notes })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_ctx(env: &[(&str, &str)]) -> DetectorContext {
        let env: HashMap<String, String> =
            env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        DetectorContext { env, sysroot: None, target_app: None }
    }

    #[tokio::test]
    async fn lc_all_wins_over_lc_ctype_and_lang() {
        let ctx =
            make_ctx(&[("LC_ALL", "en_US.UTF-8"), ("LC_CTYPE", "C"), ("LANG", "vi_VN.UTF-8")]);
        let out = LocaleDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.locale.as_deref(), Some("en_US.UTF-8"));
    }

    #[tokio::test]
    async fn falls_back_to_lc_ctype_when_lc_all_absent() {
        let ctx = make_ctx(&[("LC_CTYPE", "en_US.UTF-8"), ("LANG", "vi_VN.UTF-8")]);
        let out = LocaleDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.locale.as_deref(), Some("en_US.UTF-8"));
    }

    #[tokio::test]
    async fn falls_back_to_lang_when_others_absent() {
        let ctx = make_ctx(&[("LANG", "vi_VN.UTF-8")]);
        let out = LocaleDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.locale.as_deref(), Some("vi_VN.UTF-8"));
    }

    #[tokio::test]
    async fn empty_string_is_treated_as_unset() {
        let ctx = make_ctx(&[("LC_ALL", ""), ("LANG", "en_US.UTF-8")]);
        let out = LocaleDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.locale.as_deref(), Some("en_US.UTF-8"));
    }

    #[tokio::test]
    async fn none_when_no_vars_set() {
        let ctx = DetectorContext::default();
        let out = LocaleDetector::new().run(&ctx).await.expect("detector ok");
        assert_eq!(out.partial.locale, None);
        assert!(out.notes.iter().any(|n| n.contains("no LC_ALL")));
    }
}
