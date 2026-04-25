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

---

File này sẽ expand khi có security issue thật hoặc hướng tới v1.0.
