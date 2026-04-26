// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fuzz target for `vietime_core::env::parse_etc_environment`.
//
// Property under test: the parser never panics and never returns
// malformed keys, regardless of input shape (bogus UTF-8, stray quotes,
// embedded NULs, mile-long lines).

#![no_main]
#![allow(clippy::unwrap_used)]

use libfuzzer_sys::fuzz_target;
use vietime_core::parse_etc_environment;

fuzz_target!(|data: &[u8]| {
    // The parser takes `&str`, so we feed it a lossy-converted copy. That
    // still exercises every code path (whitespace handling, quote matching,
    // key validation) because invalid UTF-8 bytes collapse into `U+FFFD`.
    let s = String::from_utf8_lossy(data);
    let parsed = parse_etc_environment(&s);

    // Invariant: every returned key conforms to the POSIX env-var shape.
    for key in parsed.keys() {
        assert!(!key.is_empty(), "empty key escaped the parser");
        let mut chars = key.chars();
        let first = chars.next().unwrap();
        assert!(
            first == '_' || first.is_ascii_alphabetic(),
            "invalid first char in key: {key:?}"
        );
        for c in chars {
            assert!(
                c == '_' || c.is_ascii_alphanumeric(),
                "invalid char in key: {key:?} ({c:?})"
            );
        }
    }
});
