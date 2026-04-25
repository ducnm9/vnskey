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

/// Where Doctor picked up an environment variable.
///
/// Tracked per-variable on `EnvFacts::sources` so checkers can tell
/// "user exported it in `~/.profile`" apart from "nothing sets it"
/// from "systemd --user exported it for this session". The ordering
/// is also what `EnvFacts::merge_by_priority` uses to resolve conflicts
/// between detectors: `Process` always wins over static config files.
///
/// Spec ref: `spec/01-phase1-doctor.md` §B.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvSource {
    /// `/proc/self/environ` — the environment Doctor itself inherited.
    Process,
    /// `/etc/environment` (PAM-parsed KEY=value file).
    EtcEnvironment,
    /// `/etc/profile.d/*.sh` shell snippets.
    EtcProfileD,
    /// `~/.profile`, `~/.bashrc`, `~/.zshrc`,
    /// `~/.config/environment.d/*.conf`.
    HomeProfile,
    /// `systemctl --user show-environment` output.
    SystemdUserEnv,
    /// PAM environment files. Reserved for a later detector; not used
    /// in Week 2 but kept in the enum so a future detector slotting in
    /// doesn't force a schema bump.
    Pam,
    /// The detector couldn't determine where the value came from — or
    /// it predates source tagging (deserialised from an older payload).
    Unknown,
}

impl EnvSource {
    /// Ascending priority: `Process` (6) wins, `Unknown` (0) loses.
    ///
    /// The concrete numbers aren't part of the public API; only the
    /// *relative* ordering is promised. Kept `const` so the compiler
    /// can inline the comparison in `merge_by_priority`.
    const fn priority(self) -> u8 {
        match self {
            Self::Process => 6,
            Self::SystemdUserEnv => 5,
            Self::HomeProfile => 4,
            Self::EtcEnvironment => 3,
            Self::EtcProfileD => 2,
            Self::Pam => 1,
            Self::Unknown => 0,
        }
    }
}

/// Summary of the IM-relevant environment variables we care about.
///
/// `sources` is populated by detectors via `EnvFacts::from_env_with_source`
/// or `EnvFacts::merge_by_priority` — it's the Week-2 addition that lets
/// checkers cite which file a stale value came from (`VD009`).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvFacts {
    pub gtk_im_module: Option<String>,
    pub qt_im_module: Option<String>,
    pub qt4_im_module: Option<String>,
    pub xmodifiers: Option<String>,
    pub input_method: Option<String>,
    pub sdl_im_module: Option<String>,
    pub glfw_im_module: Option<String>,
    pub clutter_im_module: Option<String>,
    /// Per-variable provenance. Keys are the env-var names
    /// (`"GTK_IM_MODULE"`, …). `#[serde(default)]` keeps
    /// deserialisation compatible with older reports that lack the field.
    #[serde(default)]
    pub sources: HashMap<String, EnvSource>,
}

/// The 8 IM-relevant env var keys — kept as a const slice so every caller
/// iterates them in the same order (tests, `from_env_with_source`,
/// `merge_by_priority`, future checkers).
pub const IM_ENV_KEYS: [&str; 8] = [
    "GTK_IM_MODULE",
    "QT_IM_MODULE",
    "QT4_IM_MODULE",
    "XMODIFIERS",
    "INPUT_METHOD",
    "SDL_IM_MODULE",
    "GLFW_IM_MODULE",
    "CLUTTER_IM_MODULE",
];

impl EnvFacts {
    /// Build facts from a flat `HashMap` of env vars (typically the output of
    /// reading `/etc/environment` or the process environment).
    ///
    /// The `sources` map is left empty — callers that know where the data
    /// came from should prefer `from_env_with_source` so checkers can cite
    /// the origin.
    #[must_use]
    pub fn from_env(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| env.get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        Self {
            gtk_im_module: get("GTK_IM_MODULE"),
            qt_im_module: get("QT_IM_MODULE"),
            qt4_im_module: get("QT4_IM_MODULE"),
            xmodifiers: get("XMODIFIERS"),
            input_method: get("INPUT_METHOD"),
            sdl_im_module: get("SDL_IM_MODULE"),
            glfw_im_module: get("GLFW_IM_MODULE"),
            clutter_im_module: get("CLUTTER_IM_MODULE"),
            sources: HashMap::new(),
        }
    }

    /// Same as [`Self::from_env`] but also tags every field that ended up
    /// `Some` with `source` in the `sources` map. Detectors should always
    /// use this so `merge_by_priority` can do something meaningful.
    #[must_use]
    pub fn from_env_with_source(env: &HashMap<String, String>, source: EnvSource) -> Self {
        let mut facts = Self::from_env(env);
        for key in IM_ENV_KEYS {
            if facts.get_by_key(key).is_some() {
                facts.sources.insert(key.to_owned(), source);
            }
        }
        facts
    }

    /// Read a field by its env-var name. Returns `None` both when the var
    /// is unknown and when it is simply unset.
    #[must_use]
    pub fn get_by_key(&self, key: &str) -> Option<&str> {
        match key {
            "GTK_IM_MODULE" => self.gtk_im_module.as_deref(),
            "QT_IM_MODULE" => self.qt_im_module.as_deref(),
            "QT4_IM_MODULE" => self.qt4_im_module.as_deref(),
            "XMODIFIERS" => self.xmodifiers.as_deref(),
            "INPUT_METHOD" => self.input_method.as_deref(),
            "SDL_IM_MODULE" => self.sdl_im_module.as_deref(),
            "GLFW_IM_MODULE" => self.glfw_im_module.as_deref(),
            "CLUTTER_IM_MODULE" => self.clutter_im_module.as_deref(),
            _ => None,
        }
    }

    /// Write a field by its env-var name. Unknown keys are silently ignored
    /// — detectors only ever pass keys from [`IM_ENV_KEYS`].
    fn set_by_key(&mut self, key: &str, value: Option<String>) {
        match key {
            "GTK_IM_MODULE" => self.gtk_im_module = value,
            "QT_IM_MODULE" => self.qt_im_module = value,
            "QT4_IM_MODULE" => self.qt4_im_module = value,
            "XMODIFIERS" => self.xmodifiers = value,
            "INPUT_METHOD" => self.input_method = value,
            "SDL_IM_MODULE" => self.sdl_im_module = value,
            "GLFW_IM_MODULE" => self.glfw_im_module = value,
            "CLUTTER_IM_MODULE" => self.clutter_im_module = value,
            _ => {}
        }
    }

    /// Merge `other` into `self` per-field, preferring the value whose
    /// `EnvSource` has higher priority. Callers don't have to control
    /// detector completion order — `Process` always wins over
    /// `EtcEnvironment` regardless of who finished first.
    ///
    /// A field missing from `self.sources` is treated as `EnvSource::Unknown`
    /// (priority 0), so the first detector to contribute a value wins against
    /// the default-constructed struct.
    ///
    /// `other.sources` entries that reference unknown keys are preserved in
    /// `self.sources` unmodified — this is defensive only; detectors never
    /// produce such entries.
    pub fn merge_by_priority(&mut self, other: &EnvFacts) {
        for key in IM_ENV_KEYS {
            let incoming_source = other.sources.get(key).copied();
            let Some(incoming_source) = incoming_source else {
                // Other didn't claim this field — nothing to do. We don't
                // touch self.value even if other.value is None, matching
                // the existing "don't erase known data" invariant in
                // `PartialFacts::merge_from`.
                continue;
            };
            let current_source = self.sources.get(key).copied().unwrap_or(EnvSource::Unknown);
            if incoming_source.priority() > current_source.priority() {
                // Incoming wins: take the value even if it's None (the
                // higher-priority source authoritatively says "unset").
                self.set_by_key(key, other.get_by_key(key).map(str::to_owned));
                self.sources.insert(key.to_owned(), incoming_source);
            }
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

    // ──────────────────────────────────────────────────────────────
    // DOC-11 additional parser fixtures — bringing the edge-case
    // coverage to ≥20 as the acceptance checklist demands.
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn keeps_inline_hash_in_value_as_literal() {
        // `#` is only a comment marker at the *start* of a trimmed line.
        // Mid-value it's a legitimate character (e.g. Emacs init files).
        let s = "GTK_IM_MODULE=ibus#trailing\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("ibus#trailing"));
    }

    #[test]
    fn backtick_wrapped_value_is_not_unquoted() {
        // Only matched `"` and `'` are stripped. Shell-style backticks
        // stay literal so we don't eat text that's almost-but-not-quite
        // a command substitution.
        let s = "GTK_IM_MODULE=`ibus`\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("`ibus`"));
    }

    #[test]
    fn tab_indented_assignment_parses() {
        // Real /etc/profile.d snippets occasionally use hard tabs for
        // readability. The line-level `trim()` handles them transparently.
        let s = "\tGTK_IM_MODULE=fcitx\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("fcitx"));
    }

    #[test]
    fn crlf_line_endings_parse() {
        // Users editing on Windows or copying from docs will hit CRLF.
        // `str::lines()` handles \r\n but leaves the stray \r — the
        // per-line `trim()` then cleans it up.
        let s = "GTK_IM_MODULE=ibus\r\nQT_IM_MODULE=ibus\r\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("ibus"));
        assert_eq!(kv.get("QT_IM_MODULE").map(String::as_str), Some("ibus"));
    }

    #[test]
    fn matched_empty_double_quotes_produce_empty_string() {
        let s = "FOO=\"\"\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("FOO").map(String::as_str), Some(""));
    }

    #[test]
    fn missing_value_is_empty_string() {
        let s = "FOO=\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("FOO").map(String::as_str), Some(""));
    }

    #[test]
    fn double_quoted_value_preserves_internal_spaces() {
        // Unlike shell-words semantics we don't split on whitespace —
        // `/etc/environment` values are passed verbatim to PAM.
        let s = "FOO=\"one two three\"\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("FOO").map(String::as_str), Some("one two three"));
    }

    #[test]
    fn single_quoted_value_leaves_dollar_literal() {
        // /etc/environment isn't a shell script: variable expansion doesn't
        // happen whether quoted or not. We just strip the quotes.
        let s = "FOO='value with $SHELL'\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("FOO").map(String::as_str), Some("value with $SHELL"));
    }

    #[test]
    fn duplicate_key_last_wins() {
        // systemd's behaviour: later assignments overwrite earlier ones.
        let s = "GTK_IM_MODULE=ibus\nGTK_IM_MODULE=fcitx\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("fcitx"));
    }

    #[test]
    fn key_with_trailing_whitespace_before_equals_still_parses() {
        // `KEY = value` isn't valid per /etc/environment spec, but we
        // see it in real world files. We trim aggressively so the key
        // lookup still works.
        let s = "GTK_IM_MODULE   =ibus\n";
        let kv = parse_etc_environment(s);
        assert_eq!(kv.get("GTK_IM_MODULE").map(String::as_str), Some("ibus"));
    }

    // ──────────────────────────────────────────────────────────────
    // EnvSource / merge_by_priority / from_env_with_source — Week 2.
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn qt4_im_module_round_trips_through_from_env() {
        let e = env(&[("QT4_IM_MODULE", "fcitx")]);
        let f = EnvFacts::from_env(&e);
        assert_eq!(f.qt4_im_module.as_deref(), Some("fcitx"));
    }

    #[test]
    fn from_env_with_source_populates_sources_map() {
        let e =
            env(&[("GTK_IM_MODULE", "ibus"), ("QT_IM_MODULE", "ibus"), ("XMODIFIERS", "@im=ibus")]);
        let f = EnvFacts::from_env_with_source(&e, EnvSource::Process);
        assert_eq!(f.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
        assert_eq!(f.sources.get("QT_IM_MODULE"), Some(&EnvSource::Process));
        assert_eq!(f.sources.get("XMODIFIERS"), Some(&EnvSource::Process));
        // Variables that aren't set get no entry — keeps the map small.
        assert!(!f.sources.contains_key("SDL_IM_MODULE"));
    }

    #[test]
    fn merge_by_priority_lower_source_does_not_overwrite_process() {
        let e = env(&[("GTK_IM_MODULE", "ibus")]);
        let mut facts = EnvFacts::from_env_with_source(&e, EnvSource::Process);

        let e2 = env(&[("GTK_IM_MODULE", "fcitx")]);
        let etc = EnvFacts::from_env_with_source(&e2, EnvSource::EtcEnvironment);

        facts.merge_by_priority(&etc);
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
    }

    #[test]
    fn merge_by_priority_higher_source_overwrites_lower() {
        let e = env(&[("GTK_IM_MODULE", "fcitx")]);
        let mut facts = EnvFacts::from_env_with_source(&e, EnvSource::EtcEnvironment);

        let e2 = env(&[("GTK_IM_MODULE", "ibus")]);
        let proc_facts = EnvFacts::from_env_with_source(&e2, EnvSource::Process);

        facts.merge_by_priority(&proc_facts);
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
    }

    #[test]
    fn merge_by_priority_fills_empty_fields_regardless_of_source() {
        // Default-constructed struct has no sources at all — every incoming
        // field must land, even from a low-priority source.
        let mut facts = EnvFacts::default();
        let e = env(&[("GTK_IM_MODULE", "fcitx")]);
        let etc = EnvFacts::from_env_with_source(&e, EnvSource::EtcProfileD);

        facts.merge_by_priority(&etc);
        assert_eq!(facts.gtk_im_module.as_deref(), Some("fcitx"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::EtcProfileD));
    }

    #[test]
    fn merge_by_priority_unknown_source_loses_to_everything() {
        // A detector that somehow tagged its value `Unknown` must not
        // clobber a real source. This is a defensive guard — no detector
        // is supposed to emit `Unknown`.
        let e = env(&[("GTK_IM_MODULE", "ibus")]);
        let mut facts = EnvFacts::from_env_with_source(&e, EnvSource::HomeProfile);

        let e2 = env(&[("GTK_IM_MODULE", "fcitx")]);
        let shady = EnvFacts::from_env_with_source(&e2, EnvSource::Unknown);

        facts.merge_by_priority(&shady);
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::HomeProfile));
    }

    #[test]
    fn merge_by_priority_independent_fields_both_land() {
        // Different fields from different sources should all end up in
        // the merged struct — this is the normal "each detector
        // contributes something different" case.
        let e_proc = env(&[("GTK_IM_MODULE", "ibus")]);
        let mut facts = EnvFacts::from_env_with_source(&e_proc, EnvSource::Process);

        let e_etc = env(&[("SDL_IM_MODULE", "ibus")]);
        let etc = EnvFacts::from_env_with_source(&e_etc, EnvSource::EtcEnvironment);

        facts.merge_by_priority(&etc);
        assert_eq!(facts.gtk_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sdl_im_module.as_deref(), Some("ibus"));
        assert_eq!(facts.sources.get("GTK_IM_MODULE"), Some(&EnvSource::Process));
        assert_eq!(facts.sources.get("SDL_IM_MODULE"), Some(&EnvSource::EtcEnvironment));
    }
}
