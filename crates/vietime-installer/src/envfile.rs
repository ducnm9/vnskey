// SPDX-License-Identifier: GPL-3.0-or-later
//
// Env-file parser + writer for the Installer.
//
// The installer mutates three different flavours of env file:
//
//   1. `/etc/environment`      — one `KEY=VALUE` per line, no export.
//   2. `~/.profile`             — POSIX shell, lines of the form `export K=V`.
//   3. `~/.config/environment.d/90-vietime.conf` — systemd-user-env format,
//      same `KEY=VALUE` as /etc/environment but honoured only by systemd.
//
// All three get the same safety rail: we only touch lines bracketed by
// `# >>> VietIME managed start >>>` / `# <<< VietIME managed end <<<` so a
// user's hand-written customisation is never clobbered. A file with no
// markers at all gets the block appended. A file whose markers are
// mangled is refused at parse time so we never corrupt further.
//
// The parser is deliberately lenient on user content — comments,
// whitespace, and unrelated assignments above/below the block are
// preserved byte-for-byte via the `prefix`/`suffix` pair. Only the
// managed block is canonicalised.
//
// Spec ref: `spec/02-phase2-installer.md` §B.8.

use std::collections::BTreeMap;

/// Marker that starts the Installer-managed block. Kept stable across
/// releases — old users' files expect this exact string.
pub const MARKER_START: &str = "# >>> VietIME managed start >>>";
pub const MARKER_END: &str = "# <<< VietIME managed end <<<";

/// Hint rendered inside the managed block as a safety notice. Not parsed.
const MANAGED_HINT: &str = "# This block is managed by `vietime install`. \
Do not edit by hand — run `vietime rollback` or re-run install to change it.";

/// The three flavours of env file the installer understands. Determines
/// how lines are serialised when we rewrite the managed block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `KEY=VALUE` per line — used by `/etc/environment` and systemd.
    KeyValue,
    /// `export KEY=VALUE` per line — used by `~/.profile`.
    PosixShellExport,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvFileError {
    #[error("env file has an opening marker but no closing marker (or vice versa)")]
    UnbalancedMarkers,
    #[error("env file has more than one VietIME managed block — refuse to edit")]
    MultipleManagedBlocks,
    #[error("invalid assignment on line {line}: `{raw}`")]
    InvalidAssignment { line: usize, raw: String },
}

/// Parsed view of an env file: the raw text above the managed block, the
/// parsed key/value pairs inside the managed block, and the raw text
/// below. `update` mutates the map in-memory; `render` spits out the
/// re-serialised file.
#[derive(Copy, Clone, PartialEq, Eq)]
enum ParseState {
    Prefix,
    InBlock,
    Suffix,
}

#[derive(Debug, Clone)]
pub struct EnvFileDoc {
    prefix: String,
    suffix: String,
    /// Keys inside the managed block. `BTreeMap` gives deterministic
    /// output without relying on insertion order.
    managed: BTreeMap<String, String>,
    /// Whether the source had a managed block at all. Used on `render`
    /// to decide whether to emit an empty block (if someone explicitly
    /// unset every key) or drop it entirely.
    had_block: bool,
    format: Format,
}

impl EnvFileDoc {
    /// Parse `source` into a doc. `format` controls how `parse_line` reads
    /// each assignment and how `render` writes them back.
    pub fn parse(source: &str, format: Format) -> Result<Self, EnvFileError> {
        let mut prefix_lines = Vec::new();
        let mut suffix_lines = Vec::new();
        let mut block_lines = Vec::new();

        let mut state = ParseState::Prefix;
        let mut seen_block = false;

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed == MARKER_START {
                if state != ParseState::Prefix {
                    return Err(EnvFileError::MultipleManagedBlocks);
                }
                state = ParseState::InBlock;
                seen_block = true;
                continue;
            }
            if trimmed == MARKER_END {
                if state != ParseState::InBlock {
                    return Err(EnvFileError::UnbalancedMarkers);
                }
                state = ParseState::Suffix;
                continue;
            }
            match state {
                ParseState::Prefix => prefix_lines.push(line),
                ParseState::InBlock => block_lines.push(line),
                ParseState::Suffix => suffix_lines.push(line),
            }
        }

        if state == ParseState::InBlock {
            return Err(EnvFileError::UnbalancedMarkers);
        }

        let mut managed = BTreeMap::new();
        for (idx, raw) in block_lines.iter().enumerate() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (k, v) = parse_line(trimmed, format)
                .ok_or(EnvFileError::InvalidAssignment { line: idx + 1, raw: (*raw).to_owned() })?;
            managed.insert(k, v);
        }

        Ok(Self {
            prefix: join_lines_preserving_trailing_newline(&prefix_lines),
            suffix: join_lines_preserving_trailing_newline(&suffix_lines),
            managed,
            had_block: seen_block,
            format,
        })
    }

    /// Insert / overwrite `key = value` in the managed block.
    pub fn set(&mut self, key: &str, value: &str) {
        self.had_block = true;
        self.managed.insert(key.to_owned(), value.to_owned());
    }

    /// Remove a key from the managed block. Returns `true` if the key was
    /// present.
    pub fn unset(&mut self, key: &str) -> bool {
        self.managed.remove(key).is_some()
    }

    /// Managed-block snapshot — used by unit tests and by `SetEnvVar`'s
    /// idempotency check.
    #[must_use]
    pub fn managed(&self) -> &BTreeMap<String, String> {
        &self.managed
    }

    /// Render the doc back to a single `String` suitable for writing over
    /// the original file. Always ends in exactly one trailing newline.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.prefix);
        ensure_trailing_newline(&mut out);

        // We always emit the block if either (a) the original file had
        // one or (b) there's at least one key to write. A previously-
        // managed file with every key cleared will keep an empty block so
        // subsequent `unset` → `set` round-trips are symmetric.
        let emit_block = self.had_block || !self.managed.is_empty();
        if emit_block {
            out.push_str(MARKER_START);
            out.push('\n');
            out.push_str(MANAGED_HINT);
            out.push('\n');
            for (k, v) in &self.managed {
                out.push_str(&format_line(k, v, self.format));
                out.push('\n');
            }
            out.push_str(MARKER_END);
            out.push('\n');
        }

        out.push_str(&self.suffix);
        ensure_trailing_newline(&mut out);
        // Collapse multiple trailing newlines to exactly one.
        while out.ends_with("\n\n") {
            out.pop();
        }
        out
    }
}

fn parse_line(trimmed: &str, format: Format) -> Option<(String, String)> {
    let body = match format {
        Format::KeyValue => trimmed,
        Format::PosixShellExport => trimmed.strip_prefix("export ")?,
    };
    let (k, v) = body.split_once('=')?;
    let k = k.trim();
    if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let v = v.trim().trim_matches('"').trim_matches('\'');
    Some((k.to_owned(), v.to_owned()))
}

fn format_line(key: &str, value: &str, format: Format) -> String {
    match format {
        Format::KeyValue => format!("{key}={value}"),
        Format::PosixShellExport => format!("export {key}={value}"),
    }
}

fn join_lines_preserving_trailing_newline(lines: &[&str]) -> String {
    let mut s = String::new();
    for line in lines {
        s.push_str(line);
        s.push('\n');
    }
    s
}

fn ensure_trailing_newline(s: &mut String) {
    if !s.ends_with('\n') {
        s.push('\n');
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrips_key_value_file_without_block() {
        let src = "FOO=bar\nBAZ=qux\n";
        let doc = EnvFileDoc::parse(src, Format::KeyValue).unwrap();
        assert!(doc.managed().is_empty(), "no managed block → no managed keys");
        assert_eq!(doc.render(), "FOO=bar\nBAZ=qux\n", "idempotent on untouched file");
    }

    #[test]
    fn set_appends_managed_block_if_none_exists() {
        let src = "FOO=bar\n";
        let mut doc = EnvFileDoc::parse(src, Format::KeyValue).unwrap();
        doc.set("GTK_IM_MODULE", "fcitx");
        let out = doc.render();
        assert!(out.contains("FOO=bar"), "preserves user content");
        assert!(out.contains(MARKER_START), "adds opening marker");
        assert!(out.contains(MARKER_END), "adds closing marker");
        assert!(out.contains("GTK_IM_MODULE=fcitx"), "writes new key");
    }

    #[test]
    fn set_is_idempotent() {
        let src = "FOO=bar\n";
        let mut doc = EnvFileDoc::parse(src, Format::KeyValue).unwrap();
        doc.set("GTK_IM_MODULE", "fcitx");
        let first = doc.render();
        let mut doc2 = EnvFileDoc::parse(&first, Format::KeyValue).unwrap();
        doc2.set("GTK_IM_MODULE", "fcitx");
        assert_eq!(doc2.render(), first, "re-setting the same value is a no-op");
    }

    #[test]
    fn set_overwrites_existing_key_without_duplicating() {
        let src = "FOO=bar\n";
        let mut doc = EnvFileDoc::parse(src, Format::KeyValue).unwrap();
        doc.set("GTK_IM_MODULE", "ibus");
        doc.set("GTK_IM_MODULE", "fcitx");
        let out = doc.render();
        assert_eq!(out.matches("GTK_IM_MODULE=").count(), 1, "one key per block");
        assert!(out.contains("GTK_IM_MODULE=fcitx"));
    }

    #[test]
    fn unset_removes_key_from_managed_block() {
        let src = format!(
            "{MARKER_START}\n# managed\nGTK_IM_MODULE=fcitx\nQT_IM_MODULE=fcitx\n{MARKER_END}\n"
        );
        let mut doc = EnvFileDoc::parse(&src, Format::KeyValue).unwrap();
        assert!(doc.unset("GTK_IM_MODULE"));
        let out = doc.render();
        assert!(!out.contains("GTK_IM_MODULE"));
        assert!(out.contains("QT_IM_MODULE=fcitx"));
    }

    #[test]
    fn unset_returns_false_for_missing_key() {
        let mut doc = EnvFileDoc::parse("", Format::KeyValue).unwrap();
        assert!(!doc.unset("NOTHING"), "returns false when key absent");
    }

    #[test]
    fn unbalanced_marker_errors() {
        let src = format!("FOO=bar\n{MARKER_START}\nGTK_IM_MODULE=fcitx\n");
        let err = EnvFileDoc::parse(&src, Format::KeyValue).unwrap_err();
        assert!(matches!(err, EnvFileError::UnbalancedMarkers));
    }

    #[test]
    fn multiple_managed_blocks_error() {
        let src = format!("{MARKER_START}\nA=1\n{MARKER_END}\n{MARKER_START}\nB=2\n{MARKER_END}\n");
        let err = EnvFileDoc::parse(&src, Format::KeyValue).unwrap_err();
        assert!(matches!(err, EnvFileError::MultipleManagedBlocks));
    }

    #[test]
    fn posix_shell_export_format_roundtrips() {
        let src = "# user comment\nexport PATH=/usr/bin\n";
        let mut doc = EnvFileDoc::parse(src, Format::PosixShellExport).unwrap();
        doc.set("GTK_IM_MODULE", "fcitx");
        let out = doc.render();
        assert!(out.contains("# user comment"));
        assert!(out.contains("export PATH=/usr/bin"));
        assert!(out.contains("export GTK_IM_MODULE=fcitx"));
    }

    #[test]
    fn prefix_and_suffix_preserved_byte_for_byte() {
        let src = format!(
            "# header\nUSER_VAR=1\n{MARKER_START}\nGTK_IM_MODULE=fcitx\n{MARKER_END}\n# footer\n"
        );
        let doc = EnvFileDoc::parse(&src, Format::KeyValue).unwrap();
        let out = doc.render();
        assert!(out.starts_with("# header\nUSER_VAR=1\n"));
        assert!(out.trim_end().ends_with("# footer"));
    }

    #[test]
    fn invalid_assignment_in_block_errors() {
        let src = format!("{MARKER_START}\nthis is not a kv line\n{MARKER_END}\n");
        let err = EnvFileDoc::parse(&src, Format::KeyValue).unwrap_err();
        assert!(matches!(err, EnvFileError::InvalidAssignment { .. }));
    }

    #[test]
    fn quoted_values_are_unquoted_on_parse() {
        let src = format!("{MARKER_START}\nGTK_IM_MODULE=\"fcitx\"\n{MARKER_END}\n");
        let doc = EnvFileDoc::parse(&src, Format::KeyValue).unwrap();
        assert_eq!(doc.managed().get("GTK_IM_MODULE"), Some(&"fcitx".to_owned()));
    }

    #[test]
    fn empty_input_and_empty_set_produces_empty_output() {
        let doc = EnvFileDoc::parse("", Format::KeyValue).unwrap();
        assert_eq!(doc.render(), "\n", "empty file → single newline");
    }
}
