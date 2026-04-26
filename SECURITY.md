# Security Policy

## Supported Versions

VietIME Suite đang ở giai đoạn v0.1 development. Chỉ phiên bản latest `main` branch
nhận security update.

| Component | Supported |
|---|---|
| vietime-doctor | latest tag |
| vietime-installer | latest tag |
| vietime-bench | latest tag |

## Reporting a Vulnerability

Nếu bạn phát hiện lỗ hổng bảo mật, **đừng mở public GitHub issue**.

Thay vào đó, gửi email tới: `security@vietime.io` (placeholder — sẽ cập nhật khi
domain được setup) hoặc dùng GitHub Security Advisory private report.

### Thông tin cần cung cấp

- Component bị ảnh hưởng (doctor / installer / bench).
- Version (output của `<tool> --version`).
- Mô tả reproduce cụ thể.
- Impact estimate (RCE / privilege escalation / info disclosure / DoS).

### Disclosure timeline

- **Trong 72h**: acknowledge đã nhận.
- **Trong 14 ngày**: triage + preliminary fix plan.
- **Trong 90 ngày**: public advisory + patch release.

Reporter được credit trong advisory trừ khi yêu cầu ẩn danh.

## Scope

**In scope**:
- Installer privilege escalation (sudo handling, config file tampering).
- Path traversal trong snapshot/rollback.
- Arbitrary code execution qua config parsing.
- PII leak trong Doctor report.
- Supply chain (compromised dependency).

**Out of scope**:
- Lỗi của upstream engine (ibus-bamboo, fcitx5-bamboo) — báo tới upstream.
- Lỗi của IBus/Fcitx5 framework.
- Self-inflicted bugs do user chạy `sudo rm -rf /` dạng vậy.
- DoS qua chạy vòng lặp vô hạn tạo snapshot.

## Security Practices

- `cargo-audit` chạy mỗi PR.
- `cargo-deny` chặn unapproved license + banned crate.
- Installer không cache sudo password, không dùng `pexpect`.
- Snapshot dir `0700` permission, chỉ user owner đọc/ghi.
- PII redaction rule trong spec/01 §5.4.
- Workspace lint `unsafe_code = "forbid"` — no unsafe blocks allowed.

## Audit Notes (POL-43)

### Installer privilege escalation paths

- `sudo` invoked only via `std::process::Command` with explicit argument list.
  No shell expansion, no string interpolation into sudo commands.
- Config file writes go through the snapshot/rollback system. Rollback
  restores exact file contents from snapshot, not user-supplied data.
- Dry-run mode (`--dry-run`) executes all plan steps without mutations.

### Shell injection review

- All external commands (`ibus-daemon`, `fcitx5`, `xdotool`, `ydotool`,
  `wtype`, `gsettings`, `ibus engine`, `fcitx5-remote`) use
  `std::process::Command` with explicit `.arg()` calls — no shell
  interpolation.
- Test vector `input_keys` are passed to `xdotool type` via `--` argument
  terminator to prevent flag injection.
- Profile and vector TOML files are parsed by `toml` crate (pure Rust,
  no eval).

### PII handling

- Doctor report redacts `$USER`, hostname, and `$HOME` by default.
- `--no-redact` flag prints a stderr warning before output.
- Bench run results contain no user-identifiable information — only
  engine/app/session/mode combos and accuracy scores.

### Supply chain

- `cargo-deny` enforces license allowlist (MIT, Apache-2.0, BSD, MPL-2.0, GPL-3.0).
- `cargo-audit` checks for known CVEs on every PR.
- Lock file (`Cargo.lock`) committed to repo for reproducible builds.
