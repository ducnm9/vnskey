// SPDX-License-Identifier: GPL-3.0-or-later
//
// Stable-Rust soak tests for the two hand-rolled parsers
// (`parse_etc_environment`, `detect_from_os_release`). These run on every
// `cargo test` pass so obvious regressions (panics, malformed keys) fail
// fast without needing the nightly `cargo-fuzz` pipeline.
//
// The real fuzz campaign lives in `/fuzz/fuzz_targets/` and uses
// `libfuzzer-sys` for coverage-guided exploration. This file is the
// smoke-test counterpart: deterministic, runs in milliseconds, and shares
// the invariants the fuzz harness asserts.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::single_char_add_str
)]

use vietime_core::{detect_from_os_release, parse_etc_environment};

/// Deterministic pseudo-random byte generator seeded from a u64. We avoid
/// pulling in `rand` as a dev-dep — a tiny LCG is plenty for "feed the
/// parser 2000 garbage strings".
fn lcg_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        out.push((state >> 33) as u8);
    }
    out
}

fn assert_env_invariants(map: &std::collections::HashMap<String, String>) {
    for key in map.keys() {
        assert!(!key.is_empty(), "empty key escaped parser");
        let mut chars = key.chars();
        let first = chars.next().unwrap();
        assert!(first == '_' || first.is_ascii_alphabetic(), "invalid first char in key: {key:?}");
        for c in chars {
            assert!(c == '_' || c.is_ascii_alphanumeric(), "invalid char {c:?} in key {key:?}");
        }
    }
}

#[test]
fn parse_etc_environment_never_panics_on_random_bytes() {
    for seed in 0..2000_u64 {
        let bytes = lcg_bytes(seed, 256);
        let s = String::from_utf8_lossy(&bytes);
        let parsed = parse_etc_environment(&s);
        assert_env_invariants(&parsed);
    }
}

#[test]
fn parse_etc_environment_handles_pathological_line_lengths() {
    // A single mega-line with embedded NULs, quotes, and equals signs.
    let mut input = String::new();
    input.push_str("\u{feff}");
    input.push_str("FOO=\"");
    for i in 0..10_000 {
        input.push(char::from((i & 0x7F) as u8));
    }
    input.push('"');
    input.push('\n');
    let parsed = parse_etc_environment(&input);
    assert_env_invariants(&parsed);
}

#[test]
fn parse_etc_environment_accepts_utf8_values_but_rejects_nonascii_keys() {
    // Keys must stay ASCII; non-ASCII chars in the key position drop
    // the line silently per the POSIX env-var rule.
    let input = "ĐẠM=vietnamese\nFOO=vietnamese\nВАСЯ=russian\n";
    let parsed = parse_etc_environment(input);
    assert_env_invariants(&parsed);
    assert!(parsed.contains_key("FOO"));
    assert!(!parsed.contains_key("ĐẠM"));
    assert!(!parsed.contains_key("ВАСЯ"));
}

#[test]
fn os_release_never_panics_on_random_bytes() {
    for seed in 0..2000_u64 {
        let bytes = lcg_bytes(seed, 512);
        let s = String::from_utf8_lossy(&bytes);
        let d = detect_from_os_release(&s);
        assert!(!d.id.is_empty(), "distro id should never be empty");
    }
}

#[test]
fn os_release_accepts_mixed_quoting() {
    // A field mixing single quotes on one line and double quotes on
    // another has been seen on real systems where configuration
    // management tools touch the file. We don't promise anything about
    // the parsed value beyond "doesn't panic, id is non-empty."
    let input = "ID='ubuntu'\nPRETTY_NAME=\"Ubuntu 24.04\"\nEXTRA=weird'thing\"still\n";
    let d = detect_from_os_release(input);
    assert_eq!(d.id, "ubuntu");
}
