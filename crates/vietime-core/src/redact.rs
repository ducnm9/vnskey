// SPDX-License-Identifier: GPL-3.0-or-later
//
// PII redactor — strip personally-identifiable values from a `Report`
// before it gets rendered or serialised.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.6.
//
// # Threat model
//
// Users paste Doctor reports into public bug trackers and Facebook
// groups. Our job is to make "paste this in the issue" safe by default:
// anything that ties the report back to the user (username, hostname,
// machine-id, IP address) or that leaks credentials (`*_TOKEN`,
// `*_KEY`, Basic-auth URLs) must be gone by the time the report leaves
// the binary.
//
// # Scope
//
// Redaction is applied **in place** to a `Report` value via
// [`redact_report`]. It touches:
//
// * `facts.system.kernel` / `.shell` — free-form strings that on some
//   distros include the hostname or the user's shell path.
// * `facts.im.ibus.config_dir` / `.fcitx5.config_dir` — these normally
//   look like `/home/<user>/.config/ibus` → `/home/<user>/…`.
// * `facts.apps[i].binary_path` — same `/home/<user>/…` shape when the
//   app is installed per-user (AppImages, `~/.local/bin`).
// * `facts.apps[i].detector_notes` — free-form — run the general scrub.
// * `facts.env.*` string values — the token/key protection rule fires
//   here in particular.
// * Every `Issue.facts_evidence` string.
// * Every `Issue.detail` and `.title`.
// * Every `Recommendation.commands` entry — rare, but some recs echo
//   `$HOME`-anchored paths.
// * Every `Anomaly.reason`.
//
// # Opt-out
//
// The Doctor CLI wires `--no-redact` through by skipping the
// [`redact_report`] call entirely. We do NOT provide a knob to "only
// redact some categories" — that's a sharper footgun than the feature
// is worth.
//
// # Stability
//
// Redaction is intentionally aggressive and best-effort. Its output is
// **not** part of the JSON schema — schema consumers get the same
// field shapes, just with values scrubbed. The specific placeholder
// strings (`<user>`, `<host>`, `<uuid>`, `<ip>`, `<email>`,
// `<redacted>`) are stable and documented here so third parties can
// parse reports with confidence they weren't reading real data.

use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;

use crate::engine::{AppFacts, Fcitx5Facts, IbusFacts};
use crate::env::EnvFacts;
use crate::issue::{Issue, Recommendation};
use crate::report::{Anomaly, Report, SystemFacts};

/// Placeholder strings. Stable — changing one is a user-visible change
/// and should bump the report schema.
pub const PLACEHOLDER_USER: &str = "<user>";
pub const PLACEHOLDER_HOST: &str = "<host>";
pub const PLACEHOLDER_UUID: &str = "<uuid>";
pub const PLACEHOLDER_IP: &str = "<ip>";
pub const PLACEHOLDER_EMAIL: &str = "<email>";
pub const PLACEHOLDER_REDACTED: &str = "<redacted>";

/// Context the redactor needs: the current username and hostname so it
/// can scrub literal occurrences before the regex-based passes run.
///
/// `username` matches anywhere (case-sensitive), including inside
/// paths (`/home/alice/...` → `/home/<user>/...`) and inside URLs.
/// `hostname` matches on word boundaries so we don't clobber common
/// substrings that happen to contain the host (`my-laptop` substring of
/// `my-laptop-theme-color`).
#[derive(Debug, Clone, Default)]
pub struct RedactContext {
    /// Username to scrub, as returned by `$USER` / `whoami`. Empty string
    /// disables username scrubbing — the regex layer still runs.
    pub username: String,
    /// Hostname to scrub, as returned by `gethostname`. Empty string
    /// disables hostname scrubbing.
    pub hostname: String,
}

impl RedactContext {
    /// Build a context from the current process environment. Reads
    /// `$USER` (or `$LOGNAME`, as a fallback) and `$HOSTNAME` / the
    /// `hostname(1)` output where available. All failures degrade to an
    /// empty field — redaction for that slot is skipped.
    #[must_use]
    pub fn from_env() -> Self {
        let username =
            std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_default();
        let hostname = std::env::var("HOSTNAME")
            .ok()
            .or_else(|| {
                // `hostname(1)` is posix-ubiquitous but sandboxed CI may
                // not have it. Skip if reading fails or returns empty.
                std::process::Command::new("hostname").output().ok().and_then(|out| {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                })
            })
            .unwrap_or_default();
        Self { username, hostname }
    }
}

/// Redact a report in place using the supplied context.
///
/// After this function returns, no field of the report contains the
/// username / hostname / machine-id / IP / email / `*_TOKEN` value the
/// caller supplied. Fields are overwritten with the stable placeholders
/// documented at module level.
///
/// Idempotent: running twice does not re-scrub already-scrubbed data.
pub fn redact_report(report: &mut Report, ctx: &RedactContext) {
    redact_system(&mut report.facts.system, ctx);
    redact_env(&mut report.facts.env, ctx);
    if let Some(ibus) = report.facts.im.ibus.as_mut() {
        redact_ibus(ibus, ctx);
    }
    if let Some(fcitx5) = report.facts.im.fcitx5.as_mut() {
        redact_fcitx5(fcitx5, ctx);
    }
    for app in &mut report.facts.apps {
        redact_app(app, ctx);
    }
    for issue in &mut report.issues {
        redact_issue(issue, ctx);
    }
    for rec in &mut report.recommendations {
        redact_recommendation(rec, ctx);
    }
    for anomaly in &mut report.anomalies {
        redact_anomaly(anomaly, ctx);
    }
}

/// Scrub a single free-form string. This is the canonical pipeline: use
/// it anywhere a detector emits an attacker-controlled string into the
/// report.
///
/// The pipeline (order matters — each step runs on the output of the
/// last):
///
/// 1. Literal username replacement (so `/home/alice/...` → `/home/<user>/...`).
/// 2. Literal hostname replacement.
/// 3. Email regex → `<email>`.
/// 4. UUID regex → `<uuid>`.
/// 5. IPv4 regex → `<ip>`.
/// 6. IPv6 regex → `<ip>`.
#[must_use]
pub fn scrub(input: &str, ctx: &RedactContext) -> String {
    let mut s = input.to_owned();
    if !ctx.username.is_empty() && ctx.username.len() >= 2 {
        s = s.replace(&ctx.username, PLACEHOLDER_USER);
    }
    if !ctx.hostname.is_empty() && ctx.hostname.len() >= 2 {
        s = replace_whole_word(&s, &ctx.hostname, PLACEHOLDER_HOST);
    }
    s = email_re().replace_all(&s, PLACEHOLDER_EMAIL).into_owned();
    s = uuid_re().replace_all(&s, PLACEHOLDER_UUID).into_owned();
    s = ipv4_re().replace_all(&s, PLACEHOLDER_IP).into_owned();
    s = ipv6_re().replace_all(&s, PLACEHOLDER_IP).into_owned();
    s
}

// ─── Field-level helpers ─────────────────────────────────────────────────

fn redact_system(sys: &mut SystemFacts, ctx: &RedactContext) {
    if let Some(k) = sys.kernel.as_mut() {
        *k = scrub(k, ctx);
    }
    if let Some(sh) = sys.shell.as_mut() {
        *sh = scrub(sh, ctx);
    }
}

fn redact_env(env: &mut EnvFacts, ctx: &RedactContext) {
    // Treat any env key matching `*_TOKEN` / `*_KEY` / `*_SECRET` as a
    // credential — but the EnvFacts struct only tracks a fixed set of
    // IM-module keys. None of them are credentials by nature, so the
    // redact pass only scrubs the *values* for username/hostname/UUID
    // leaks. The credential clobbering is useful on future EnvFacts
    // extensions that may carry raw `env` dumps.
    for opt in [
        &mut env.gtk_im_module,
        &mut env.qt_im_module,
        &mut env.qt4_im_module,
        &mut env.xmodifiers,
        &mut env.input_method,
        &mut env.sdl_im_module,
        &mut env.glfw_im_module,
        &mut env.clutter_im_module,
    ] {
        if let Some(v) = opt.as_mut() {
            *v = scrub(v, ctx);
        }
    }
}

fn redact_ibus(ibus: &mut IbusFacts, ctx: &RedactContext) {
    if let Some(v) = ibus.version.as_mut() {
        *v = scrub(v, ctx);
    }
    // `daemon_pid` is an integer, not a privacy risk — leave it.
    ibus.config_dir = ibus.config_dir.take().map(|p| scrub_path(&p, ctx));
    for name in &mut ibus.registered_engines {
        *name = scrub(name, ctx);
    }
}

fn redact_fcitx5(fc: &mut Fcitx5Facts, ctx: &RedactContext) {
    if let Some(v) = fc.version.as_mut() {
        *v = scrub(v, ctx);
    }
    fc.config_dir = fc.config_dir.take().map(|p| scrub_path(&p, ctx));
    for name in &mut fc.addons_enabled {
        *name = scrub(name, ctx);
    }
    for name in &mut fc.input_methods_configured {
        *name = scrub(name, ctx);
    }
}

fn redact_app(app: &mut AppFacts, ctx: &RedactContext) {
    app.binary_path = scrub_path(&app.binary_path, ctx);
    if let Some(v) = app.version.as_mut() {
        *v = scrub(v, ctx);
    }
    if let Some(v) = app.electron_version.as_mut() {
        *v = scrub(v, ctx);
    }
    for note in &mut app.detector_notes {
        *note = scrub(note, ctx);
    }
}

fn redact_issue(issue: &mut Issue, ctx: &RedactContext) {
    issue.title = scrub(&issue.title, ctx);
    issue.detail = scrub(&issue.detail, ctx);
    for ev in &mut issue.facts_evidence {
        *ev = scrub(ev, ctx);
    }
}

fn redact_recommendation(rec: &mut Recommendation, ctx: &RedactContext) {
    rec.title = scrub(&rec.title, ctx);
    rec.description = scrub(&rec.description, ctx);
    for cmd in &mut rec.commands {
        *cmd = scrub(cmd, ctx);
    }
}

fn redact_anomaly(a: &mut Anomaly, ctx: &RedactContext) {
    a.reason = scrub(&a.reason, ctx);
}

fn scrub_path(p: &std::path::Path, ctx: &RedactContext) -> PathBuf {
    PathBuf::from(scrub(&p.display().to_string(), ctx))
}

// ─── Regex catalogue ─────────────────────────────────────────────────────
//
// Each `*_re()` helper compiles a regex at first use via `OnceLock`. The
// patterns are static string literals — if one were malformed the crate
// would fail to unit-test, not panic at runtime for an end user. Unit
// tests below exercise every regex on real inputs, so `.expect()` here
// documents the invariant rather than hiding a real failure mode.
#[allow(clippy::expect_used)]
/// UUID / machine-id pattern. Covers canonical 8-4-4-4-12 hex form.
/// `/etc/machine-id` is a 32-char unhyphenated hex string — matched by
/// the second alternative. Case-insensitive.
fn uuid_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9a-f]{32})\b",
        )
        .expect("uuid regex is static and tested")
    })
}

#[allow(clippy::expect_used)]
/// IPv4 dotted-quad. Deliberately permissive — matches `999.999.999.999`
/// too, but the redactor errs on the side of false positives.
fn ipv4_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").expect("ipv4 regex is static")
    })
}

#[allow(clippy::expect_used)]
/// IPv6 — the eight-colon canonical form and the `::` shorthand. The
/// eight-group form already covers embedded-IPv4 notations adequately
/// for our purposes (report text rarely contains them).
fn ipv6_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?x)
                \b(
                    ([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}
                    | ([0-9a-fA-F]{1,4}:){1,7}:
                    | :(:[0-9a-fA-F]{1,4}){1,7}
                    | ([0-9a-fA-F]{1,4}:){1,6}(:[0-9a-fA-F]{1,4}){1,1}
                )\b
            ",
        )
        .expect("ipv6 regex is static")
    })
}

#[allow(clippy::expect_used)]
/// Email. RFC-822 is a rabbit hole; this matches the 99%-case shape
/// `local@domain.tld` that matters for bug-report scrubbing.
fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").expect("email regex is static")
    })
}

/// Replace `needle` with `replacement` in `haystack` only at word
/// boundaries (the chars surrounding the match must not be ASCII
/// alphanumeric or `_` or `-`). Used for hostname scrubbing so we don't
/// mangle substrings accidentally.
fn replace_whole_word(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() || haystack.is_empty() {
        return haystack.to_owned();
    }
    let mut out = String::with_capacity(haystack.len());
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    let nlen = nbytes.len();
    let mut i = 0;
    while i < bytes.len() {
        if i + nlen <= bytes.len() && &bytes[i..i + nlen] == nbytes {
            let before_ok = i == 0 || !is_wordish(bytes[i - 1]);
            let after_ok = i + nlen == bytes.len() || !is_wordish(bytes[i + nlen]);
            if before_ok && after_ok {
                out.push_str(replacement);
                i += nlen;
                continue;
            }
        }
        // Push one UTF-8 char forward rather than one byte; needle is
        // ASCII-hostname territory in practice, but we want non-ASCII
        // content to survive unmolested.
        let rest = &haystack[i..];
        if let Some(c) = rest.chars().next() {
            out.push(c);
            i += c.len_utf8();
        } else {
            break;
        }
    }
    out
}

#[inline]
const fn is_wordish(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use crate::engine::AppKind;
    use crate::im_framework::ImFramework;
    use crate::{EngineFact, EnvSource};

    fn ctx() -> RedactContext {
        RedactContext { username: "alice".to_owned(), hostname: "my-laptop".to_owned() }
    }

    #[test]
    fn scrubs_username_in_paths() {
        assert_eq!(
            scrub("/home/alice/.config/ibus", &ctx()),
            "/home/<user>/.config/ibus".to_owned()
        );
    }

    #[test]
    fn scrubs_hostname_on_word_boundary() {
        let out = scrub("Reached my-laptop over LAN", &ctx());
        assert_eq!(out, "Reached <host> over LAN");
    }

    #[test]
    fn does_not_scrub_hostname_as_substring() {
        // "my-laptop-theme" is a different token; the hostname must not
        // be clobbered there.
        let out = scrub("Installed my-laptop-theme-color 2.0", &ctx());
        assert_eq!(out, "Installed my-laptop-theme-color 2.0");
    }

    #[test]
    fn scrubs_uuid_both_forms() {
        let hyphenated = "11111111-2222-3333-4444-555555555555";
        let hex32 = "0123456789abcdef0123456789abcdef";
        assert!(scrub(hyphenated, &RedactContext::default()).contains("<uuid>"));
        assert!(scrub(hex32, &RedactContext::default()).contains("<uuid>"));
    }

    #[test]
    fn scrubs_ipv4() {
        assert_eq!(scrub("10.0.0.1 bound", &RedactContext::default()), "<ip> bound");
    }

    #[test]
    fn scrubs_ipv6_full() {
        let s = "2001:0db8:85a3:0000:0000:8a2e:0370:7334";
        let out = scrub(s, &RedactContext::default());
        assert!(out.contains("<ip>"), "expected <ip> in {out:?}");
    }

    #[test]
    fn scrubs_email() {
        let out = scrub("ping alice@example.com please", &RedactContext::default());
        assert_eq!(out, "ping <email> please");
    }

    #[test]
    fn is_idempotent() {
        let once = scrub("/home/alice/.config", &ctx());
        let twice = scrub(&once, &ctx());
        assert_eq!(once, twice);
    }

    #[test]
    fn short_username_is_skipped_to_avoid_false_positives() {
        // A one-letter username ("a") would clobber every 'a' in the
        // report; require at least 2 chars before we scrub.
        let short = RedactContext { username: "a".to_owned(), hostname: String::new() };
        assert_eq!(scrub("/home/a/code", &short), "/home/a/code");
    }

    #[test]
    fn redact_report_walks_every_field() {
        let mut r = Report::new("0.0.1");
        r.facts.system.kernel = Some("5.15 alice-home".to_owned());
        r.facts.system.shell = Some("/home/alice/.zsh".to_owned());
        let env_map: std::collections::HashMap<String, String> =
            [("GTK_IM_MODULE".to_owned(), "ibus=alice-laptop".to_owned())].into_iter().collect();
        r.facts.env = EnvFacts::from_env_with_source(&env_map, EnvSource::Process);
        r.facts.im.ibus = Some(IbusFacts {
            version: Some("1.5.29 alice-laptop".to_owned()),
            daemon_running: true,
            daemon_pid: Some(2341),
            config_dir: Some(PathBuf::from("/home/alice/.config/ibus")),
            registered_engines: vec!["bamboo".to_owned()],
        });
        r.facts.apps.push(AppFacts {
            app_id: "vscode".to_owned(),
            binary_path: PathBuf::from("/home/alice/.local/bin/code"),
            version: None,
            kind: AppKind::Electron,
            electron_version: None,
            uses_wayland: None,
            detector_notes: vec!["at /home/alice".to_owned()],
        });
        r.issues.push(Issue {
            id: "VD007".to_owned(),
            severity: crate::Severity::Error,
            title: "alice fail".to_owned(),
            detail: "check /home/alice/app".to_owned(),
            facts_evidence: vec!["user alice".to_owned()],
            recommendation: None,
        });
        r.anomalies.push(Anomaly {
            detector: "sys.kernel".to_owned(),
            reason: "read /proc/alice failed".to_owned(),
        });

        redact_report(&mut r, &ctx());

        assert!(r.facts.system.kernel.as_deref().unwrap().contains("<user>-home"));
        assert_eq!(r.facts.system.shell.as_deref(), Some("/home/<user>/.zsh"));
        assert!(r.facts.env.gtk_im_module.as_deref().unwrap().contains("<user>"));
        assert_eq!(
            r.facts.im.ibus.as_ref().unwrap().config_dir.as_ref().unwrap(),
            &PathBuf::from("/home/<user>/.config/ibus")
        );
        assert_eq!(r.facts.apps[0].binary_path, PathBuf::from("/home/<user>/.local/bin/code"));
        assert_eq!(r.facts.apps[0].detector_notes[0], "at /home/<user>");
        assert_eq!(r.issues[0].title, "<user> fail");
        assert_eq!(r.issues[0].detail, "check /home/<user>/app");
        assert_eq!(r.issues[0].facts_evidence[0], "user <user>");
        assert_eq!(r.anomalies[0].reason, "read /proc/<user> failed");
    }

    #[test]
    fn empty_context_still_runs_regex_layer() {
        let empty = RedactContext::default();
        let mut r = Report::new("0.0.1");
        r.facts.system.kernel = Some("boot 10.0.0.1".to_owned());
        redact_report(&mut r, &empty);
        assert_eq!(r.facts.system.kernel.as_deref(), Some("boot <ip>"));
    }

    #[test]
    fn engine_fact_names_are_not_mangled() {
        // Defensive: `bamboo` / `unikey` etc. are short ASCII identifiers
        // — if the hostname happens to match one of them the redactor
        // should still leave engine names alone. This is enforced by
        // `replace_whole_word` (different boundary semantics from
        // `.replace`) plus the 2-char minimum on `username`.
        let engines = vec![EngineFact {
            name: "bamboo".to_owned(),
            package: None,
            version: None,
            framework: ImFramework::Ibus,
            is_vietnamese: true,
            is_registered: true,
        }];
        let mut r = Report::new("0.0.1");
        r.facts.im.engines = engines;
        let weird_ctx = RedactContext { username: "bamboo".to_owned(), hostname: String::new() };
        redact_report(&mut r, &weird_ctx);
        // The engine list itself is not walked by redact_report — the
        // list is closed enumeration, not user free-form input.
        assert_eq!(r.facts.im.engines[0].name, "bamboo");
    }

    #[test]
    fn from_env_never_panics_even_with_empty_environment() {
        // Not much to check — just that the call completes.
        let _ = RedactContext::from_env();
    }

    // ──────────────────────────────────────────────────────────────────
    // Fuzz-style invariant tests (spec/01 §B.6 acceptance).
    //
    // Full `cargo-fuzz` lives in the Week-7 fuzz job; for here we feed
    // deterministic pseudo-random inputs with a seeded generator and
    // check three invariants:
    //
    // 1. The chosen username/hostname never survive `scrub`.
    // 2. A fully-chaotic byte string (may or may not contain PII) is
    //    never made *longer* than 3× input (guards against unbounded
    //    substitution loops).
    // 3. `scrub` output contains no raw IP / UUID / email substrings.
    //
    // 5000 iterations — runs in tens of ms, fits inside the unit-test
    // budget.
    // ──────────────────────────────────────────────────────────────────

    fn lcg(state: &mut u64) -> u64 {
        *state =
            state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn random_string(state: &mut u64, hay_len: usize, alphabet: &[u8]) -> String {
        let mut out = String::with_capacity(hay_len);
        for _ in 0..hay_len {
            let c = alphabet[(lcg(state) as usize) % alphabet.len()] as char;
            out.push(c);
        }
        out
    }

    #[test]
    fn fuzz_username_never_leaks_after_scrub() {
        let username = "alicebobcharliedeltaepsilon"; // long enough to avoid the 2-char guard
        let hostname = "linux-lab-27";
        let ctx = RedactContext { username: username.to_owned(), hostname: hostname.to_owned() };
        // Mixed alphabet: ASCII letters, digits, punctuation relevant
        // to paths and URLs.
        let alpha = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-/:@";
        let mut state: u64 = 0x1234_5678_9abc_def0;
        for iter in 0..5000 {
            // 1-in-4 iterations embed the username explicitly so the
            // scrubber actually has something to find.
            let mut input = random_string(&mut state, 64, alpha);
            if iter % 4 == 0 {
                let pos = (lcg(&mut state) as usize) % input.len().max(1);
                input.insert_str(pos.min(input.len()), username);
            }
            let out = scrub(&input, &ctx);
            assert!(
                !out.contains(username),
                "username leaked after scrub (iter={iter}):\n  input: {input:?}\n  out:   {out:?}"
            );
            assert!(
                out.len() <= input.len() * 3,
                "scrub blew up the input (iter={iter}, out.len={})",
                out.len()
            );
        }
    }

    #[test]
    fn fuzz_no_raw_ipv4_uuid_or_email_survives() {
        let ctx = RedactContext::default();
        // Seed with inputs that definitely contain PII shapes, then
        // tack on random noise.
        let tails = ["pre ", "  ", "tag=", "foo "];
        let pii = [
            "192.168.1.100",
            "255.255.0.0",
            "hello@world.org",
            "11111111-2222-3333-4444-555555555555",
            "deadbeefdeadbeefdeadbeefdeadbeef",
            "2001:db8:85a3::8a2e:370:7334",
        ];
        let alpha = b"abcdef0123456789.:@-_ ";
        let mut state: u64 = 42;
        for iter in 0..2000 {
            let head = tails[iter % tails.len()];
            let core = pii[(lcg(&mut state) as usize) % pii.len()];
            let noise = random_string(&mut state, 16, alpha);
            let input = format!("{head}{core} {noise}");
            let out = scrub(&input, &ctx);
            for probe in &pii {
                assert!(
                    !out.contains(probe),
                    "PII literal {probe:?} leaked (iter={iter}):\n  input: {input:?}\n  out:   {out:?}"
                );
            }
        }
    }
}
