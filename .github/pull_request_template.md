## Task

Closes #NNN hoặc tham chiếu task ID (`DOC-##` / `INS-##` / `BEN-##` / `P0-##` / `POL-##`).

## What & Why

<1–2 câu mô tả thay đổi chính và lý do.>

## How tested

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Thêm test mới: `<test_name>` tại `<path>`
- [ ] Manual test:
  - <steps>

## Acceptance criteria (from task file)

Copy acceptance criteria từ `tasks/phase-X.md` và check off:

- [ ] ...
- [ ] ...

## Checklist

- [ ] Title theo Conventional Commits.
- [ ] Không có commit > 400 LOC; nếu có, đã split.
- [ ] Không `unwrap()` hoặc `expect()` trong code path production (test OK).
- [ ] Error message mới có actionable hint.
- [ ] Public API thay đổi → docs cập nhật.
- [ ] Config file format thay đổi → schema version bump.
- [ ] Không tăng binary size > 10% (check `cargo bloat`).

## Screenshots / asciinema (nếu UI/UX change)

<paste hoặc link>
