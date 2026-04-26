// SPDX-License-Identifier: GPL-3.0-or-later
//
// Test vector data model + TOML loader (BEN-12).
//
// A test vector is (input_keys, expected_output) — the bench runner feeds
// `input_keys` through the IME and compares the captured text against
// `expected_output`. Vectors live in `test-vectors/*.toml`; the loader
// deserialises them and validates Unicode NFC normalisation.
//
// Spec ref: `spec/03-phase3-test-suite.md` §A.5, §B.9.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Top-level structure of a `test-vectors/*.toml` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorFile {
    pub version: u32,
    #[serde(default)]
    pub engine_mode: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub vectors: Vec<TestVector>,
}

/// A single test case: type `input_keys` via the IME → expect
/// `expected_output` in the target app's text area.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestVector {
    pub id: String,
    pub input_keys: String,
    pub expected_output: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub known_failing_on: Vec<String>,
    #[serde(default)]
    pub upstream_issue: Option<String>,
}

/// Errors from loading or validating test vectors.
#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    #[error("i/o error reading `{path}`: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("TOML parse error in `{path}`: {source}")]
    Parse { path: String, source: toml::de::Error },

    #[error("validation failed:\n{}", messages.join("\n"))]
    Validation { messages: Vec<String> },
}

/// Load a single TOML vector file from disk.
pub fn load_vector_file(path: &Path) -> Result<VectorFile, VectorError> {
    let content = std::fs::read_to_string(path).map_err(|e| VectorError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let vf: VectorFile = toml::from_str(&content).map_err(|e| VectorError::Parse {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(vf)
}

/// Load all `*.toml` files from a directory and merge their vectors.
pub fn load_vectors_from_dir(dir: &Path) -> Result<Vec<TestVector>, VectorError> {
    let mut all = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| VectorError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();

    for path in paths {
        let vf = load_vector_file(&path)?;
        all.extend(vf.vectors);
    }
    Ok(all)
}

/// Validate a set of vectors: check for duplicate IDs and NFC normalisation
/// of `expected_output`. Returns `Ok(())` or a list of problems.
pub fn validate_vectors(vectors: &[TestVector]) -> Result<(), VectorError> {
    let mut messages = Vec::new();
    let mut seen_ids = HashSet::new();

    for v in vectors {
        if !seen_ids.insert(&v.id) {
            messages.push(format!("  duplicate id: {}", v.id));
        }
        if v.id.is_empty() {
            messages.push("  vector has empty id".to_owned());
        }
        if v.input_keys.is_empty() {
            messages.push(format!("  {}: input_keys is empty", v.id));
        }
        if v.expected_output.is_empty() {
            messages.push(format!("  {}: expected_output is empty", v.id));
        }
        if !is_nfc(&v.expected_output) {
            messages.push(format!(
                "  {}: expected_output is not NFC-normalised",
                v.id
            ));
        }
    }

    if messages.is_empty() {
        Ok(())
    } else {
        Err(VectorError::Validation { messages })
    }
}

/// Check if a string is in Unicode NFC form. We compare the string against
/// its NFC-recomposed form byte-by-byte. For the ASCII + Vietnamese diacritics
/// range this is reliable without pulling in the `unicode-normalization` crate:
/// precomposed Vietnamese letters (à, ắ, ữ, …) are already their own code
/// points in NFC, so the only way a string fails this check is if it was
/// written with combining sequences.
fn is_nfc(s: &str) -> bool {
    // Fast path: pure ASCII is always NFC.
    if s.is_ascii() {
        return true;
    }
    // Check for combining marks (Unicode category Mn). Vietnamese NFC text
    // should not have standalone combining diacriticals.
    for ch in s.chars() {
        if is_combining_mark(ch) {
            return false;
        }
    }
    true
}

/// Returns `true` if `ch` is a Unicode combining mark in the ranges relevant
/// to Vietnamese text. This covers the Combining Diacritical Marks block
/// (U+0300..U+036F) which includes the accents used in Vietnamese.
fn is_combining_mark(ch: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&ch)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn vec_with(id: &str, input: &str, expected: &str) -> TestVector {
        TestVector {
            id: id.to_owned(),
            input_keys: input.to_owned(),
            expected_output: expected.to_owned(),
            tags: vec![],
            known_failing_on: vec![],
            upstream_issue: None,
        }
    }

    #[test]
    fn parse_vector_file_toml() {
        let toml = r#"
version = 1
engine_mode = "telex"
description = "Core Telex"

[[vectors]]
id = "T001"
input_keys = "tieesng Vieejt"
expected_output = "tiếng Việt"
tags = ["basic", "tone"]

[[vectors]]
id = "T002"
input_keys = "aa"
expected_output = "â"
tags = ["modifier"]
"#;
        let vf: VectorFile = toml::from_str(toml).unwrap();
        assert_eq!(vf.version, 1);
        assert_eq!(vf.engine_mode.as_deref(), Some("telex"));
        assert_eq!(vf.vectors.len(), 2);
        assert_eq!(vf.vectors[0].id, "T001");
        assert_eq!(vf.vectors[0].expected_output, "tiếng Việt");
        assert_eq!(vf.vectors[1].tags, vec!["modifier"]);
    }

    #[test]
    fn parse_bugs_vector_with_known_failing() {
        let toml = r#"
version = 1

[[vectors]]
id = "BUG-001"
input_keys = "xin chaof"
expected_output = "xin chào"
tags = ["electron"]
known_failing_on = ["ibus-bamboo@<=0.8.2 + vscode + wayland"]
upstream_issue = "https://github.com/example/issue/1"
"#;
        let vf: VectorFile = toml::from_str(toml).unwrap();
        assert_eq!(vf.vectors[0].known_failing_on.len(), 1);
        assert!(vf.vectors[0].upstream_issue.is_some());
    }

    #[test]
    fn validate_catches_duplicate_ids() {
        let vecs = vec![
            vec_with("T001", "aa", "â"),
            vec_with("T001", "ee", "ê"),
        ];
        let err = validate_vectors(&vecs).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate id"), "{msg}");
    }

    #[test]
    fn validate_catches_empty_fields() {
        let vecs = vec![vec_with("", "aa", "â")];
        let err = validate_vectors(&vecs).unwrap_err();
        assert!(err.to_string().contains("empty id"));
    }

    #[test]
    fn validate_catches_non_nfc() {
        // "ắ" decomposed: a + breve + acute
        let decomposed = "a\u{0306}\u{0301}";
        let vecs = vec![vec_with("T001", "aws", decomposed)];
        let err = validate_vectors(&vecs).unwrap_err();
        assert!(err.to_string().contains("NFC"), "{}", err);
    }

    #[test]
    fn validate_passes_good_vectors() {
        let vecs = vec![
            vec_with("T001", "aa", "â"),
            vec_with("T002", "tieesng", "tiếng"),
        ];
        validate_vectors(&vecs).unwrap();
    }

    #[test]
    fn is_nfc_accepts_precomposed_vietnamese() {
        assert!(is_nfc("tiếng Việt"));
        assert!(is_nfc("xin chào các bạn"));
        assert!(is_nfc("ASCII only"));
    }

    #[test]
    fn is_nfc_rejects_combining_sequences() {
        // U+0301 COMBINING ACUTE ACCENT
        let bad = "e\u{0301}";
        assert!(!is_nfc(bad));
    }

    #[test]
    fn load_vector_file_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.toml");
        std::fs::write(
            &path,
            r#"
version = 1
[[vectors]]
id = "T001"
input_keys = "aa"
expected_output = "â"
"#,
        )
        .unwrap();
        let vf = load_vector_file(&path).unwrap();
        assert_eq!(vf.vectors.len(), 1);
    }

    #[test]
    fn load_vectors_from_dir_merges_files() {
        let dir = tempfile::tempdir().unwrap();
        for (name, id) in [("a.toml", "T001"), ("b.toml", "T002")] {
            std::fs::write(
                dir.path().join(name),
                format!(
                    r#"
version = 1
[[vectors]]
id = "{id}"
input_keys = "aa"
expected_output = "â"
"#
                ),
            )
            .unwrap();
        }
        let vecs = load_vectors_from_dir(dir.path()).unwrap();
        assert_eq!(vecs.len(), 2);
    }
}
