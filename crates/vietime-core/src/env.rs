// SPDX-License-Identifier: GPL-3.0-or-later
//
// Environment file parsing.
//
// Reads IM-relevant env variables from `/etc/environment`-style files and
// builds an `EnvFacts` summary used by Doctor and Installer.
//
// `/etc/environment` is NOT a shell script — it's a simple KEY=value file
// consumed by PAM. Comments start with `#`, blank lines allowed. Values
// may be quoted with double quotes; escapes are NOT processed. This is the
// format Ubuntu, Debian, and Fedora all use. We keep the parser compatible
// with `environment.d/*.conf` files as well.
//
// Spec ref: `spec/02-phase2-installer.md` §B.8.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::im_framework::{parse_im_module_value, ImFramework};

/// Summary of the IM-relevant environment variables we care about.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvFacts {
    pub gtk_im_module: Option<String>,
    pub qt_im_module: Option<String>,
    pub xmodifiers: Option<String>,
    pub input_method: Option<String>,
    pub sdl_im_module: Option<String>,
    pub glfw_im_module: Option<String>,
    pub clutter_im_module: Option<String>,
}

impl EnvFacts {
    /// Build facts from a flat `HashMap` of env vars (typically the output of
    /// reading `/etc/environment` or the process environment).
    #[must_use]
    pub fn from_env(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| env.get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Self {
            gtk_im_module: get("GTK_IM_MODULE"),
            qt_im_module: get("QT_IM_MODULE"),
            xmodifiers: get("XMODIFIERS"),
            input_method: get("INPUT_METHOD"),
            sdl_im_module: get("SDL_IM_MODULE"),
            glfw_im_module: get("GLFW_IM_MODULE"),
            clutter_im_module: get("CLUTTER_IM_MODULE"),
        }
    }

    /// Most authoritative framework signal: compare the three major vars
    /// (`GTK_IM_MODULE`, `QT_IM_MODULE`, `XMODIFIERS`) and return the unique
    /// framework if they all agree, or `ImFramework::None` otherwise.
    ///
    /// Disagreement is a known pain point — Doctor's VD003 checker flags it.
    #[must_use]
    pub fn unified_framework(&self) -> ImFramework {
        let gtk = self.gtk_im_module.as_deref().map(parse_im_module_value);
        let qt = self.qt_im_module.as_deref().map(parse_im_module_value);
        let xmod = self.xmodifiers.as_deref().map(parse_im_module_value);

        let present: Vec<ImFramework> =
            [gtk, qt, xmod].into_iter().flatten().filter(|f| *f != ImFramework::None).collect();

        match present.as_slice() {
            [first, rest @ ..] if rest.iter().all(|f| f == first) => *first,
            // Either nothing is set, or they disagree.
            _ => ImFramework::None,
        }
    }

    /// True if at least one IM var is set.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.gtk_im_module.is_some()
            || self.qt_im_module.is_some()
            || self.xmodifiers.is_some()
            || self.input_method.is_some()
    }

    /// True if the three major vars (GTK/QT/X) disagree on the framework.
    #[must_use]
    pub fn has_disagreement(&self) -> bool {
        let set: std::collections::HashSet<ImFramework> = [
            self.gtk_im_module.as_deref().map(parse_im_module_value),
            self.qt_im_module.as_deref().map(parse_im_module_value),
            self.xmodifiers.as_deref().map(parse_im_module_value),
        ]
        .into_iter()
        .flatten()
        .filter(|f| *f != ImFramework::None)
        .collect();
        set.len() > 1
    }
}

/// Parse a `/etc/environment`-format string.
///
/// Returns a `HashMap<String, String>` of KEY → value with quotes stripped
/// and trailing whitespace trimmed. Silently ignores malformed lines — we
/// want to produce a usable report even when the file has been hand-edited.
#[must_use]
pub fn parse_etc_environment(contents: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Support Debian's PAM-parsed format which does NOT understand
        // `export`, but Fedora's `environment.d/` files often do — accept
        // the prefix for compatibility.
        let line = line.strip_prefix("export ").unwrap_or(line);

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !is_valid_env_key(key) {
            continue;
        }
        out.insert(key.to_string(), unquote(value.trim()));
    }
    out
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let first = bytes[0];
        let last = bytes[value.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }

    #[test]
    fn parses_default_ubuntu_etc_environment() {
        // Real-world Ubuntu /etc/environment plus IM vars appended.
        let s = r#"PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/games"
GTK_IM_MODULE=ibus
QT_IM_MODULE=ibus
XMODIFIERS=@im=ibus
"#;
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("ibus"));
        assert_eq!(kv.get("QT_IM_MODULE").map(String::as_str), Some("ibus"));
        assert_eq!(kv.get("XMODIFIERS").map(String::as_str), Some("@im=ibus"));
        assert!(kv.contains_key("PATH"));
    }

    #[test]
    fn strips_double_quotes() {
        let s = r#"GTK_IM_MODULE="fcitx""#;
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("fcitx"));
    }

    #[test]
    fn strips_single_quotes() {
        let s = "XMODIFIERS='@im=fcitx'\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("XMODIFIERS").map(String::as_str), Some("@im=fcitx"));
    }

    #[test]
    fn handles_export_prefix() {
        let s = "export GTK_IM_MODULE=fcitx\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("fcitx"));
    }

    #[test]
    fn skips_comments_and_blanks() {
        let s = "\n# GTK_IM_MODULE=should-be-ignored\n\nQT_IM_MODULE=fcitx\n";
        let kv = parse_etc_environment(s);
        assert!(!kv.contains_key("GTK_IM_MODULE"));
        assert_eq!(kv.get("QT_IM_MODULE").map(String::as_str), Some("fcitx"));
    }

    #[test]
    fn rejects_malformed_keys() {
        let s = "1INVALID=x\nvalid_key=y\n bad key =z\n";
        let kv = parse_etc_environment(s);
        assert!(!kv.contains_key("1INVALID"));
        assert_eq!(kv.get("valid_key").map(String::as_str), Some("y"));
        assert!(!kv.keys().any(|k| k.contains(' ')));
    }

    #[test]
    fn env_facts_from_env_picks_known_keys_only() {
        let e = env(&[
            ("GTK_IM_MODULE", "fcitx"),
            ("QT_IM_MODULE", "fcitx"),
            ("XMODIFIERS", "@im=fcitx"),
            ("PATH", "/usr/bin"),
            ("HOME", "/home/x"),
        ]);
        let f = EnvFacts::from_env(&e);
        assert_eq!(f.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(f.qt_im_module.as_deref(), Some("fcitx"));
        assert_eq!(f.xmodifiers.as_deref(), Some("@im=fcitx"));
        assert!(f.has_any());
        assert!(!f.has_disagreement());
        assert_eq!(f.unified_framework(), ImFramework::Fcitx5);
    }

    #[test]
    fn detects_disagreement_between_gtk_and_qt() {
        let e = env(&[
            ("GTK_IM_MODULE", "ibus"),
            ("QT_IM_MODULE", "fcitx"),
            ("XMODIFIERS", "@im=ibus"),
        ]);
        let f = EnvFacts::from_env(&e);
        assert!(f.has_disagreement());
        assert_eq!(f.unified_framework(), ImFramework::None);
    }

    #[test]
    fn empty_env_has_no_signals() {
        let e: HashMap<String, String> = HashMap::new();
        let f = EnvFacts::from_env(&e);
        assert!(!f.has_any());
        assert!(!f.has_disagreement());
        assert_eq!(f.unified_framework(), ImFramework::None);
    }

    #[test]
    fn empty_string_values_treated_as_unset() {
        let e = env(&[("GTK_IM_MODULE", ""), ("QT_IM_MODULE", "   ")]);
        let f = EnvFacts::from_env(&e);
        assert!(!f.has_any());
    }

    #[test]
    fn ignores_bom_prefix() {
        let s = "\u{feff}GTK_IM_MODULE=ibus\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("ibus"));
    }

    #[test]
    fn preserves_value_with_equals_sign() {
        let s = "FOO=a=b=c\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("FOO").map(String::as_str), Some("a=b=c"));
    }
}
