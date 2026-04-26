// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fuzz target for `vietime_core::distro::detect_from_os_release`.
//
// Property under test: `/etc/os-release` parsing is total — any input must
// resolve to a `Distro` (possibly `unknown`) without panicking. Real
// os-release files are shell-ish with enough quirks (BOM, mixed quoting,
// trailing junk) to deserve a fuzz soak before 0.1.0 ships.

#![no_main]
#![allow(clippy::unwrap_used)]

use libfuzzer_sys::fuzz_target;
use vietime_core::detect_from_os_release;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    let d = detect_from_os_release(&s);
    // Invariant: `id` is always populated (`"unknown"` when no ID= line).
    assert!(!d.id.is_empty(), "distro id should never be empty");
});
