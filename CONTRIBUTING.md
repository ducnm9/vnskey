# Contributing to VietIME Suite

Chào bạn! Cám ơn đã quan tâm đóng góp cho VietIME Suite.

Trước khi bắt đầu, **đọc 2 file này**:
- [Code of Conduct](CODE_OF_CONDUCT.md) — hành vi cộng đồng.
- [docs/dev/process.md](docs/dev/process.md) — quy trình dev, quality gate, release.

## First PR in 30 minutes

```bash
# 1. Fork + clone
git clone https://github.com/<your-user>/vietime.git
cd vietime

# 2. Build + test
cargo build --workspace
cargo test --workspace

# 3. Pick a "good-first-issue"
# https://github.com/<org>/vietime/labels/good-first-issue

# 4. Branch
git checkout -b doc/DOC-NN-short-desc

# 5. Code + test

# 6. Pre-flight
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 7. Commit (Conventional Commits)
git commit -m "feat(doctor): detect ibus-daemon"

# 8. Push + open PR
```

## Commit message format

Theo [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

**type**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `build`, `ci`.
**scope**: `core`, `doctor`, `installer`, `bench`, `spec`, `docs`, `ci`, `release`.

Ví dụ:
- `feat(doctor): add IBus daemon detector`
- `fix(installer): restore env file on SIGINT`
- `docs(spec): clarify rollback invariants`

## PR checklist

- [ ] Title theo Conventional Commits.
- [ ] Mô tả: **What changed? Why? How tested?**
- [ ] Mỗi acceptance criterion trong task tương ứng ✓.
- [ ] Ít nhất 1 test mới (hoặc ghi lý do không).
- [ ] `cargo fmt --check` + `clippy -D warnings` + `test` local xanh.
- [ ] Nếu chạm public API → docs cập nhật.

## SLA

Maintainer phản hồi PR ready-for-review trong 72h (nghỉ 1–2 ngày cuối tuần). Nếu
quá 72h, bạn được tự ping.

## Quality gates (CI tự chạy)

PR phải pass:
- `cargo fmt --check`
- `cargo clippy -D warnings`
- `cargo build --workspace`
- `cargo test --workspace`
- `cargo deny check`
- `cargo audit`

Chi tiết: [docs/dev/process.md §2](docs/dev/process.md).

## Issue templates

- **Bug**: dùng template `bug.yml`. Yêu cầu kèm `vietime-doctor report --json`.
- **Feature request**: template `feature.yml`.
- **Compat matrix entry**: template `compat-matrix-entry.yml`.

## Task tracking

Đọc `tasks/README.md`. Task có ID (DOC-##, INS-##, BEN-##). Contributor nên:
1. Comment vào issue liên quan xin assign.
2. Maintainer confirm → bạn bắt đầu.
3. Mở draft PR từ commit đầu.

Không tự assign quá 2 task cùng lúc.

## Cảm ơn 🙏

Đóng góp của bạn giúp 1 triệu dev Việt gõ tiếng Việt trên Linux không đau đầu nữa.
