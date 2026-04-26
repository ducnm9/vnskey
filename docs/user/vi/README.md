<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# VietIME Doctor — Hướng dẫn sử dụng (Tiếng Việt)

`vietime-doctor` kiểm tra hệ thống Linux của bạn để tìm các nguyên
nhân khiến việc gõ tiếng Việt không hoạt động: thiếu tiến trình nền
(daemon) của input method, biến môi trường bị lệch, ứng dụng Electron
không chuyển tiếp phím IME, v.v. Công cụ này **chỉ báo cáo** — không
tự chỉnh sửa hệ thống.

> **Phạm vi:** Phase 1 chỉ gồm Doctor (chẩn đoán). Phần Installer
> (tự động sửa lỗi) sẽ có trong Phase 2.

## Bắt đầu nhanh

```bash
vietime-doctor          # Báo cáo đầy đủ dạng Markdown ra stdout
vietime-doctor check    # Trạng thái 1 dòng + mã thoát (0/1/2)
vietime-doctor list     # Liệt kê mọi detector & checker có trong bản build
```

Mã thoát:

| Mã  | Ý nghĩa |
|-----|---------|
| 0   | Không có vấn đề (hoặc chỉ Info) |
| 1   | Có cảnh báo (Warn) |
| 2   | Có lỗi (Error / Critical) |
| 64  | Sai cú pháp dòng lệnh |
| 70  | Lỗi nội bộ Doctor |

## Định dạng đầu ra

* `--plain` — bỏ ký tự Markdown cho terminal đọc thô.
* `--json` — định dạng máy đọc, ổn định. Được kiểm tra bằng
  [`schemas/report.v1.json`](../../../schemas/report.v1.json); dùng
  được trong CI.
* `--verbose` — thêm chân trang tóm tắt phiên bản schema và số lượng
  vấn đề.

## Che dấu dữ liệu cá nhân

Mặc định, `vietime-doctor` xóa `$USER`, tên máy và đường dẫn home
trong báo cáo. Khi bạn chia sẻ báo cáo trên bug tracker, dữ liệu đã
được che. Truyền `--no-redact` để xem báo cáo gốc — Doctor sẽ in cảnh
báo lên stderr để bạn không quên.

## Danh mục kiểm tra

Phase 1 có 15 kiểm tra (VD001 – VD015). Mỗi kiểm tra có ID ổn định,
mức nghiêm trọng và (cho Warn/Error/Critical) một đề xuất `VR###`
kèm lệnh shell cụ thể.

| ID | Mức độ | Kích hoạt khi |
|----|--------|---------------|
| VD001 | Critical | Đã cài engine tiếng Việt nhưng không có daemon IM nào chạy |
| VD002 | Error    | IBus và Fcitx5 chạy cùng lúc |
| VD003 | Error    | Biến môi trường IM không khớp với framework đang dùng |
| VD004 | Warn     | Không đặt `SDL_IM_MODULE` |
| VD005 | Warn     | Engine đã cài nhưng chưa đăng ký vào framework |
| VD006 | Warn     | Dùng IBus trên Wayland (nên dùng Fcitx5) |
| VD007 | Error    | App Electron không chạy với `--ozone-platform=wayland` |
| VD008 | Warn     | Chrome/Chromium trên Wayland thiếu cờ Ozone |
| VD009 | Warn     | Cùng biến môi trường nhưng khác giá trị ở hai file |
| VD010 | Warn     | VS Code cài qua Snap (IM bị chặn) |
| VD011 | Warn     | App Flatpak thiếu quyền IM portal |
| VD012 | Info     | Không đặt `INPUT_METHOD` (gợi ý legacy) |
| VD013 | Warn     | Fcitx5 thiếu addon phù hợp (wayland-im / xim) |
| VD014 | Warn     | Locale hiện tại không phải UTF-8 |
| VD015 | Info     | Chưa cài engine tiếng Việt nào |

ID đề xuất là `VR001`…`VR014` (không có VR012 / VR015 — kiểm tra mức
Info không kèm hành động tự sửa).

## Tình huống thường gặp

**"Sao gõ tiếng Việt của tôi không hoạt động?"**

```bash
vietime-doctor | less
```

Đọc mục `## Checks` từ trên xuống. Dòng Critical/Error cho biết
nguyên nhân chính; mục Recommendations có lệnh cụ thể để sửa.

**"Kiểm tra trạng thái trong CI"**

```bash
vietime-doctor check
# Mã thoát 0 / 1 / 2 khớp với ngưỡng nghiêm trọng của CI
```

**"VS Code của tôi có vấn đề"**

```bash
vietime-doctor --app vscode
```

Khi truyền `--app`, các detector `app.generic` và `app.electron` sẽ
chạy — Doctor kiểm tra cờ Ozone và báo VD007/VD010 nếu cần.

## Báo lỗi & trợ giúp

* Báo bug: <https://github.com/vietime/vietime/issues>
* Thảo luận: <https://github.com/vietime/vietime/discussions>
* Xem [bảng thuật ngữ](GLOSSARY.md) nếu gặp từ viết tắt.
