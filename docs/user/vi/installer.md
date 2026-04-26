<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# VietIME Installer — Hướng dẫn sử dụng (Tiếng Việt)

`vietime-installer` là công cụ cài đặt một cú pháp cho gõ tiếng Việt
trên Linux. Chọn một combo (ví dụ Fcitx5 + Bamboo) và installer sẽ lo
phần cài gói, đặt biến môi trường, bật service và gọi `im-config` —
tất cả đều được ghi snapshot để có thể rollback bất cứ lúc nào.

> **Phạm vi:** Phase 2 cung cấp `install` / `uninstall` / `switch` cho
> 4 combo MVP trên họ Debian (Ubuntu, Pop!_OS, Debian). Fedora và Arch
> sẽ có trong v0.2 (INS-50 / INS-51).

## Bắt đầu nhanh

```bash
# Xem tool sẽ làm gì, chưa đụng vào hệ thống.
vietime-installer install fcitx5-bamboo --dry-run

# Cài tương tác (sẽ hỏi sudo một lần).
vietime-installer install fcitx5-bamboo

# Hoặc chọn từ wizard.
vietime-installer install

# Kiểm tra kết quả.
vietime-installer verify
```

Chuỗi happy-path kết thúc bằng lời nhắc 1 dòng yêu cầu bạn logout và
login lại — đây là bước thủ công duy nhất, vì GTK/Qt chỉ đọc env vars
khi khởi tạo phiên.

## Danh sách lệnh

| Lệnh | Ý nghĩa |
|------|---------|
| `install [combo]`          | Lập kế hoạch + thực thi; chạy wizard nếu bỏ trống `combo` |
| `uninstall`                | Rollback snapshot gần nhất (tương đương `rollback`) |
| `switch <combo>`           | Gỡ combo hiện tại, cài combo mới |
| `rollback [--to ID] [--force]` | Undo một snapshot cụ thể hoặc snapshot mới nhất; `--force` vượt qua manifest `incomplete=true` |
| `snapshots`                | Liệt kê mọi snapshot, mới nhất trước |
| `status`                   | Tóm tắt 1 dòng về snapshot đang áp dụng |
| `verify`                   | Gọi `vietime-doctor check` (exit 0/1/2) |
| `list`                     | Liệt kê combo hỗ trợ |
| `doctor [args…]`           | Chuyển tham số thẳng sang `vietime-doctor` |
| `version`                  | In phiên bản installer |
| `hello`                    | Smoke check (in phiên bản + banner `vietime-core`) |

### Cờ toàn cục

* `--dry-run` — lên kế hoạch nhưng không thay đổi gì. Mỗi bước in
  hành động dự kiến; không ghi snapshot.
* `-y`, `--yes` — bỏ qua mọi prompt xác nhận. Kết hợp với sudo đã cache
  (chạy `sudo -v` trước) để chạy hoàn toàn không tương tác trong CI.
* `-v`, `--verbose` — tracing thêm ra stderr.
* `--log-file PATH` — ghi log vào `PATH` thay vì stderr.

## Mã thoát

| Mã | Ý nghĩa |
|----|---------|
| 0  | Thành công |
| 64 | Sai cú pháp (cờ sai, combo không tồn tại) |
| 70 | Lỗi nội bộ (I/O snapshot, package manager, Ctrl+C) |

## Các combo hỗ trợ

Bốn combo MVP:

| Slug               | Framework | Engine  | Gói (Ubuntu/Debian) |
|--------------------|-----------|---------|---------------------|
| `fcitx5-bamboo`    | Fcitx5    | Bamboo  | `fcitx5`, `fcitx5-bamboo` |
| `fcitx5-unikey`    | Fcitx5    | Unikey  | `fcitx5`, `fcitx5-unikey` |
| `ibus-bamboo`      | IBus      | Bamboo  | `ibus`, `ibus-bamboo`     |
| `ibus-unikey`      | IBus      | Unikey  | `ibus`, `ibus-unikey`     |

Fcitx5 được khuyến nghị mạnh cho phiên Wayland. Combo IBus dành cho
người dùng X11/GNOME muốn giữ stack mặc định.

## Snapshot & rollback

Mọi lần chạy thay đổi hệ thống đều ghi manifest vào
`~/.config/vietime/snapshots/<timestamp>/` kèm theo các bản sao lưu
cần thiết. Bố cục:

```
~/.config/vietime/snapshots/
├── 2026-04-26T10-15-00Z/
│   ├── manifest.toml            # plan + artifact + cờ incomplete
│   └── files/
│       ├── etc_environment.bak
│       └── etc_environment.bak.sha256
└── latest -> 2026-04-26T10-15-00Z
```

* `manifest.toml` ghi lại plan đã chạy, danh sách artifact (backup,
  gói đã cài, thay đổi service) và cờ `incomplete = true` chỉ được
  set thành `false` khi kết thúc sạch. Nếu bị SIGKILL giữa các bước,
  manifest sẽ ở trạng thái `incomplete` — `rollback` sẽ từ chối nếu
  không kèm `--force`.
* `latest` trỏ về snapshot mới nhất, để `uninstall` / `rollback` chạy
  không cần tham số.
* File sidecar SHA-256 phòng tránh bit-rot khi restore backup.

### Luồng rollback phổ biến

```bash
# Undo lần cài gần nhất.
vietime-installer uninstall

# Liệt kê snapshot, rollback về snapshot cụ thể.
vietime-installer snapshots
vietime-installer rollback --to 2026-04-24T09-31-00Z

# Ép rollback một lần chạy bị đứt.
vietime-installer rollback --force
```

## Xử lý sudo

Installer **không** cache mật khẩu và **không** chạy sudo với stdin
được pipe. Tool dựa vào credential cache riêng của `sudo`: lần chạy
mutating đầu tiên, `sudo -v` sẽ hỏi trên TTY một lần; các lệnh package
manager tiếp theo dùng lại cache đó.

* `--yes` chuyển sang `sudo -n` (không tương tác). Nếu sudo phải hỏi,
  installer báo lỗi và yêu cầu bạn chạy `sudo -v` trước.
* Plan chỉ đụng vào home (`~/.profile`, `systemctl --user`) không hề
  gọi sudo.

## Tình huống thường gặp

**"Mới cài Ubuntu, cài giúp Fcitx5-Bamboo"**

```bash
vietime-installer install fcitx5-bamboo
# … một prompt sudo, ~30 giây …
# Logout, login lại, gõ thôi.
```

**"Chuyển từ IBus sang Fcitx5"**

```bash
vietime-installer switch fcitx5-bamboo
```

Lệnh này rollback snapshot hiện tại (trả env IBus về) rồi cài combo mới
một cách atomic.

**"CI kiểm tra image headless"**

```bash
sudo -v                                 # prime credential cache
vietime-installer install fcitx5-bamboo --yes
vietime-installer verify                # exit 0 nếu pass
```

**"Có lỗi, trả máy về trạng thái cũ"**

```bash
vietime-installer uninstall
```

## Xử lý sự cố

* **"no snapshots found"** — chưa có lần cài nào qua VietIME. `status`
  và `uninstall` đều báo rõ.
* **"snapshot `…` is flagged incomplete"** — lần chạy trước bị giết
  giữa chừng. Xem `~/.config/vietime/snapshots/<id>/manifest.toml`
  rồi chạy lại với `--force` để rollback.
* **Lỗi package manager** — installer trả lại stderr gốc từ `apt-get`.
  Thường gặp: mất mạng, `/etc/apt/sources.list` hỏng, gói bị hold. Sửa
  xong chạy lại `install`.
* **Vẫn không gõ được tiếng Việt** — chạy `vietime-doctor` để xem check
  nào lỗi. VD001 / VD002 / VD003 thường là thủ phạm.

## Nhận trợ giúp

* Báo bug: <https://github.com/vietime/vietime/issues>
* Thảo luận: <https://github.com/vietime/vietime/discussions>
* Hướng dẫn Doctor: [README.md](README.md)
