<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Tham chiếu kiểm tra (VD001 – VD015)

Một trang cho mỗi kiểm tra: điều kiện kích hoạt, dữ liệu Doctor in
ra, và việc đề xuất đi kèm thật sự làm gì. ID ổn định giữa các bản
vá — cứ trích dẫn trong bug report.

## VD001 — Không có IM framework nào chạy (Critical)

* **Kích hoạt:** Đã cài engine tiếng Việt (ví dụ `ibus-bamboo`)
  nhưng cả `ibus-daemon` và `fcitx5` đều không chạy.
* **Sửa (VR001):** Bật framework bạn muốn dùng.
  * IBus: `systemctl --user enable --now ibus`
  * Fcitx5: `systemctl --user enable --now fcitx5`

## VD002 — Cả hai framework cùng chạy (Error)

* **Kích hoạt:** IBus và Fcitx5 đều sống.
* **Vì sao:** Chúng tranh socket IM; kẻ thua âm thầm mất phím.
* **Sửa (VR002):** Tắt một bên. Thường Fcitx5 thắng trên KDE / Sway,
  IBus thắng trên GNOME.

## VD003 — Biến IM không khớp framework đang dùng (Error)

* **Kích hoạt:** `GTK_IM_MODULE`, `QT_IM_MODULE`, hoặc `XMODIFIERS`
  trỏ sai framework (hoặc bất đồng với nhau).
* **Sửa (VR003):** Đặt cả bốn biến cùng giá trị, ghi vào
  `/etc/environment` hoặc `~/.config/environment.d/`.

## VD004 — Thiếu `SDL_IM_MODULE` (Warn)

* **Kích hoạt:** `SDL_IM_MODULE` trống và có framework đang chạy.
* **Vì sao:** Game SDL (và nhiều app Electron) sẽ không nhận IME.
* **Sửa (VR004):** Đặt giá trị giống `GTK_IM_MODULE`.

## VD005 — Engine đã cài nhưng chưa đăng ký (Warn)

* **Kích hoạt:** Engine tiếng Việt có trên máy nhưng framework chưa
  liệt kê.
* **Sửa (VR005):** Chạy `ibus-setup` hoặc `fcitx5-configtool`, tick
  engine, đăng xuất rồi đăng nhập lại.

## VD006 — IBus trên Wayland (Warn)

* **Kích hoạt:** Session Wayland; active framework là IBus.
* **Vì sao:** Hỗ trợ Wayland của IBus đã cải thiện nhưng vẫn kém
  Fcitx5 trên KDE Plasma và Sway. Nếu gõ "kẹt", thường là đây.
* **Sửa (VR006):** Chuyển sang Fcitx5 (tùy chọn, không bắt buộc).

## VD007 — App Electron thiếu Ozone/Wayland (Error, cần `--app`)

* **Kích hoạt:** App Electron chỉ định đang chạy không có
  `--ozone-platform=wayland`.
* **Sửa (VR007):** Khởi động lại với cờ đó, hoặc ghi vào file
  desktop / `electron-flags.conf`.

## VD008 — Chrome Wayland thiếu Ozone (Warn, cần `--app`)

* **Kích hoạt:** Chrome/Chromium + Wayland + không có cờ Ozone.
* **Sửa (VR008):** `google-chrome --ozone-platform=wayland` (và ghi
  vào launcher).

## VD009 — Biến khác nhau ở hai file (Warn)

* **Kích hoạt:** Cùng một key xuất hiện với giá trị khác nhau ở hai
  nguồn (ví dụ `/etc/environment` và `~/.profile`).
* **Sửa (VR009):** Gộp về một nơi (`/etc/environment` cho toàn hệ
  thống, `~/.config/environment.d/` cho người dùng).

## VD010 — VS Code Snap (Warn, `--app vscode`)

* **Kích hoạt:** Binary VS Code là Snap.
* **Vì sao:** Sandbox Snap chặn tín hiệu IM.
* **Sửa (VR010):** Dùng bản `.deb` hoặc `.rpm`.

## VD011 — Flatpak thiếu IM portal (Warn)

* **Kích hoạt:** App Flatpak thiếu quyền
  `--talk-name=org.freedesktop.portal.IBus` (hoặc tương đương).
* **Sửa (VR011):** `flatpak override --user --talk-name=…`.

## VD012 — `INPUT_METHOD` chưa đặt (Info)

* **Kích hoạt:** Chỉ thông tin — app rất cũ mới cần biến này.
* **Sửa:** Không có. Chỉ Info.

## VD013 — Fcitx5 thiếu addon phù hợp (Warn)

* **Kích hoạt:** Fcitx5 chạy nhưng chưa bật addon tương ứng:
  * Wayland → cần `wayland-im`
  * X11 → cần `xim`
* **Sửa (VR013):** Vào `fcitx5-configtool` → Addons và bật.

## VD014 — Locale không phải UTF-8 (Warn)

* **Kích hoạt:** `LC_ALL` / `LC_CTYPE` / `LANG` không cho ra locale
  UTF-8 (hoặc không có).
* **Sửa (VR014):** `sudo locale-gen en_US.UTF-8` (hoặc locale UTF-8
  bạn thích), export vào `/etc/default/locale` hoặc profile shell.

## VD015 — Chưa cài engine tiếng Việt (Info)

* **Kích hoạt:** Không có engine tiếng Việt nào trên máy.
* **Sửa:** Cài một cái (`ibus-bamboo`, `fcitx5-bamboo`,…). Info-only;
  Doctor không càu nhàu người chưa set-up tiếng Việt.
