// SPDX-License-Identifier: GPL-3.0-or-later
//
// Static app profile registry and binary-resolution helpers.
//
// The registry is the small hardcoded list of apps the Doctor knows how to
// diagnose when the user passes `--app <X>`. It lives entirely in-crate,
// behind read-only `&'static` tables — no user override, no config file —
// that's a Week 5+ feature (`~/.config/vietime/apps.toml`). See spec/01
// §B.5.
//
// Two submodules:
//
//   * `registry` — `AppProfile` + `PROFILES` + `resolve_app()`.
//   * `resolve`  — subprocess-using helpers shared by DOC-31 and DOC-32
//                  (binary-path lookup, `--version` parsing).

pub mod registry;
pub mod resolve;

pub use registry::{resolve_app, AppProfile, PROFILES};
pub use resolve::{parse_version_token, resolve_binary};
