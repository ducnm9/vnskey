// SPDX-License-Identifier: GPL-3.0-or-later
//
// Distro detection from `/etc/os-release`.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.2 (Facts data model).
//
// The parser is intentionally defensive: real `/etc/os-release` files
// in the wild contain inconsistent quoting, comments, blank lines, and
// occasional BOM bytes. We accept all of these and extract only the keys
// we care about.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// High-level family used for package manager and env-file decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistroFamily {
    Debian,
    Redhat,
    Arch,
    Suse,
    Alpine,
    Nix,
    Unknown,
}

/// Concrete distro identification.
///
/// `name` and `version` come from `ID` / `VERSION_ID` in
/// `/etc/os-release`. `pretty` is `PRETTY_NAME` when available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Distro {
    pub id: String,
    pub version_id: Option<String>,
    pub pretty: Option<String>,
    pub family: DistroFamily,
    /// The `ID_LIKE` field, normalized to lowercase tokens. Helps catch
    /// derivatives (e.g. Pop!_OS declares `ID_LIKE=ubuntu debian`).
    pub id_like: Vec<String>,
}

impl Distro {
    /// Build an "unknown" marker when parsing fails or the file is missing.
    /// We never panic or error here — the Doctor should still produce a
    /// report even on exotic systems.
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            id: "unknown".to_string(),
            version_id: None,
            pretty: None,
            family: DistroFamily::Unknown,
            id_like: Vec::new(),
        }
    }

    /// True if the distro belongs to a given family (directly or via
    /// `ID_LIKE`).
    #[must_use]
    pub fn is_family(&self, family: DistroFamily) -> bool {
        if self.family == family {
            return true;
        }
        let expected_ids: &[&str] = match family {
            DistroFamily::Debian => &["debian", "ubuntu"],
            DistroFamily::Redhat => &["rhel", "fedora", "centos"],
            DistroFamily::Arch => &["arch"],
            DistroFamily::Suse => &["suse", "opensuse"],
            DistroFamily::Alpine => &["alpine"],
            DistroFamily::Nix => &["nixos"],
            DistroFamily::Unknown => return false,
        };
        self.id_like.iter().any(|id| expected_ids.contains(&id.as_str()))
    }
}

/// Parse the contents of `/etc/os-release`.
///
/// Follows the [systemd `os-release(5)`][os-release] specification: shell-style
/// KEY=value, values can be quoted with `"` or `'`. Blank lines and comments
/// starting with `#` are ignored. Unknown keys are ignored silently — we only
/// read `ID`, `VERSION_ID`, `PRETTY_NAME`, and `ID_LIKE`.
///
/// Returns [`Distro::unknown`] when the input is empty or contains no `ID=`.
///
/// [os-release]: https://www.freedesktop.org/software/systemd/man/os-release.html
#[must_use]
pub fn detect_from_os_release(contents: &str) -> Distro {
    let kv = parse_os_release_kv(contents);

    let Some(id) = kv.get("ID").cloned() else {
        return Distro::unknown();
    };

    let version_id = kv.get("VERSION_ID").cloned();
    let pretty = kv.get("PRETTY_NAME").cloned();
    let id_like = kv
        .get("ID_LIKE")
        .map(|v| v.split_whitespace().map(str::to_ascii_lowercase).collect::<Vec<_>>())
        .unwrap_or_default();

    let family = classify_family(&id, &id_like);

    Distro { id, version_id, pretty, family, id_like }
}

fn parse_os_release_kv(contents: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw_line in contents.lines() {
        // Strip BOM on the first line if present.
        let line = raw_line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let value = unquote(value.trim());
        out.insert(key, value);
    }
    out
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 {
        let first = value.chars().next().unwrap_or(' ');
        let last = value.chars().last().unwrap_or(' ');
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            // Trim the matching wrapper characters.
            return value[first.len_utf8()..value.len() - last.len_utf8()].to_string();
        }
    }
    value.to_string()
}

fn classify_family(id: &str, id_like: &[String]) -> DistroFamily {
    let id_lower = id.to_ascii_lowercase();
    let family_of = |key: &str| -> Option<DistroFamily> {
        match key {
            "debian" | "ubuntu" | "pop" | "linuxmint" | "elementary" | "kali" => {
                Some(DistroFamily::Debian)
            }
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => Some(DistroFamily::Redhat),
            "arch" | "manjaro" | "endeavouros" => Some(DistroFamily::Arch),
            "opensuse-leap" | "opensuse-tumbleweed" | "opensuse" | "suse" | "sles" => {
                Some(DistroFamily::Suse)
            }
            "alpine" => Some(DistroFamily::Alpine),
            "nixos" => Some(DistroFamily::Nix),
            _ => None,
        }
    };

    if let Some(f) = family_of(&id_lower) {
        return f;
    }
    for like in id_like {
        if let Some(f) = family_of(like) {
            return f;
        }
    }
    DistroFamily::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    const UBUNTU_22_04: &str = r#"NAME="Ubuntu"
VERSION="22.04.4 LTS (Jammy Jellyfish)"
ID=ubuntu
ID_LIKE=debian
PRETTY_NAME="Ubuntu 22.04.4 LTS"
VERSION_ID="22.04"
"#;

    const UBUNTU_24_04: &str = r#"PRETTY_NAME="Ubuntu 24.04 LTS"
NAME="Ubuntu"
VERSION_ID="24.04"
VERSION="24.04 LTS (Noble Numbat)"
ID=ubuntu
ID_LIKE=debian
"#;

    const DEBIAN_12: &str = r#"PRETTY_NAME="Debian GNU/Linux 12 (bookworm)"
NAME="Debian GNU/Linux"
VERSION_ID="12"
VERSION="12 (bookworm)"
ID=debian
HOME_URL="https://www.debian.org/"
"#;

    const FEDORA_40: &str = r#"NAME="Fedora Linux"
VERSION="40 (Workstation Edition)"
ID=fedora
VERSION_ID=40
PRETTY_NAME="Fedora Linux 40 (Workstation Edition)"
"#;

    const ARCH: &str = r#"NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
BUILD_ID=rolling
"#;

    const POP_OS: &str = r#"NAME="Pop!_OS"
VERSION="22.04 LTS"
ID=pop
ID_LIKE="ubuntu debian"
PRETTY_NAME="Pop!_OS 22.04 LTS"
VERSION_ID="22.04"
"#;

    #[test]
    fn ubuntu_22_04_is_debian_family() {
        let d = detect_from_os_release(UBUNTU_22_04);
        assert_eq!(d.id, "ubuntu");
        assert_eq!(d.version_id.as_deref(), Some("22.04"));
        assert_eq!(d.pretty.as_deref(), Some("Ubuntu 22.04.4 LTS"));
        assert_eq!(d.family, DistroFamily::Debian);
        assert!(d.is_family(DistroFamily::Debian));
    }

    #[test]
    fn ubuntu_24_04_parses() {
        let d = detect_from_os_release(UBUNTU_24_04);
        assert_eq!(d.id, "ubuntu");
        assert_eq!(d.version_id.as_deref(), Some("24.04"));
        assert_eq!(d.family, DistroFamily::Debian);
    }

    #[test]
    fn debian_12_parses() {
        let d = detect_from_os_release(DEBIAN_12);
        assert_eq!(d.id, "debian");
        assert_eq!(d.family, DistroFamily::Debian);
        assert!(d.id_like.is_empty());
    }

    #[test]
    fn fedora_40_is_redhat_family() {
        let d = detect_from_os_release(FEDORA_40);
        assert_eq!(d.id, "fedora");
        assert_eq!(d.version_id.as_deref(), Some("40"));
        assert_eq!(d.family, DistroFamily::Redhat);
    }

    #[test]
    fn arch_without_version_id_parses() {
        let d = detect_from_os_release(ARCH);
        assert_eq!(d.id, "arch");
        assert_eq!(d.family, DistroFamily::Arch);
        assert!(d.version_id.is_none());
    }

    #[test]
    fn pop_os_classified_via_id_like() {
        let d = detect_from_os_release(POP_OS);
        assert_eq!(d.id, "pop");
        assert_eq!(d.family, DistroFamily::Debian);
        assert_eq!(d.id_like, vec!["ubuntu", "debian"]);
        assert!(d.is_family(DistroFamily::Debian));
    }

    #[test]
    fn empty_returns_unknown() {
        assert_eq!(detect_from_os_release("").family, DistroFamily::Unknown);
    }

    #[test]
    fn garbage_returns_unknown() {
        let junk = "this is not kv data\njust prose\n";
        assert_eq!(detect_from_os_release(junk).id, "unknown");
    }

    #[test]
    fn handles_comments_and_blank_lines() {
        let s = "\n# comment\n\nID=debian\n# another comment\nVERSION_ID=\"12\"\n";
        let d = detect_from_os_release(s);
        assert_eq!(d.id, "debian");
        assert_eq!(d.version_id.as_deref(), Some("12"));
    }

    #[test]
    fn handles_bom_prefix() {
        let s = "\u{feff}ID=ubuntu\nVERSION_ID=22.04\n";
        let d = detect_from_os_release(s);
        assert_eq!(d.id, "ubuntu");
    }

    #[test]
    fn handles_single_quoted_values() {
        let s = "ID='arch'\nPRETTY_NAME='Arch Linux'\n";
        let d = detect_from_os_release(s);
        assert_eq!(d.id, "arch");
        assert_eq!(d.pretty.as_deref(), Some("Arch Linux"));
    }

    #[test]
    fn is_family_matches_direct_and_id_like() {
        let pop = detect_from_os_release(POP_OS);
        assert!(pop.is_family(DistroFamily::Debian));
        assert!(!pop.is_family(DistroFamily::Redhat));
    }
}
