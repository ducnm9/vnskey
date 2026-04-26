<!--
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Bảng thuật ngữ (Tiếng Việt)

**IM / Input Method / Phương thức nhập**
Phần mềm biến phím bấm thành ký tự tổ hợp. Cần thiết cho tiếng Việt
vì `đ` và dấu thanh đều yêu cầu nhiều hơn một phím.

**IM framework / Khung nhập liệu**
Hệ thống mà phương thức nhập dùng để giao tiếp với ứng dụng. Trên
Linux thường là **IBus** hoặc **Fcitx5**. Chỉ nên bật một.

**IBus**
Intelligent Input Bus — framework mặc định của GNOME. Đơn giản, hoạt
động tốt trên X11 và GNOME Wayland hiện đại.

**Fcitx5**
Đối thủ phổ biến của IBus. Hỗ trợ Wayland tốt hơn trên KDE Plasma và
Sway, có nhiều add-on hơn.

**Engine**
Mô-đun ngôn ngữ bên trong framework. Tiếng Việt thường có: `Bamboo`,
`Unikey`, `TCVN`,… Cài package không tự động đăng ký — bạn thường
vẫn cần `ibus-setup` hoặc `fcitx5-configtool`.

**Ozone / Ozone Platform**
Lớp trừu tượng của Chromium cho hệ thống cửa sổ. Truyền
`--ozone-platform=wayland` để app Electron/Chromium chạy native trên
Wayland; nếu không, chúng quay về XWayland và thường nuốt mất dấu.

**XWayland**
Lớp tương thích giúp app X11 chạy trên Wayland. Gõ IME qua XWayland
hay bị mất dấu — đa số lỗi gõ tiếng Việt có gốc ở đây.

**Session (X11 / Wayland) / Phiên hiển thị**
Giao thức máy chủ hiển thị mà desktop đang dùng. Doctor đọc từ
`$XDG_SESSION_TYPE`.

**Locale**
Ngôn ngữ + mã hóa mà shell đang quảng bá. Các biến
**`LC_ALL` / `LC_CTYPE` / `LANG`** quyết định giá trị này; Doctor
cần locale UTF-8, nếu không các ký tự ngoài ASCII sẽ hỏng.

**Biến môi trường Doctor xem xét**

* `GTK_IM_MODULE` — IM mà app GTK3/GTK4 dùng (`ibus` / `fcitx`).
* `QT_IM_MODULE` — tương tự cho app Qt.
* `XMODIFIERS` — bộ chọn cũ trên X11 (`@im=ibus` / `@im=fcitx`).
* `SDL_IM_MODULE` — game và app dùng SDL.
* `GLFW_IM_MODULE` — app GLFW.
* `CLUTTER_IM_MODULE` — app GNOME Clutter.
* `INPUT_METHOD` — gợi ý cũ, ít dùng.

**VD### / VR###**
ID ổn định mà Doctor in cho mỗi kiểm tra / đề xuất. An toàn để trích
dẫn trong báo bug — chúng không đổi giữa các bản vá.

**Redaction / Che dấu**
Bước bảo mật mặc định, xóa `$USER`, `$HOSTNAME` và đường dẫn home.
Tắt bằng `--no-redact`.

**Snap / Flatpak / AppImage**
Các định dạng đóng gói sandbox. Mỗi loại có câu chuyện IM-portal
riêng: Flatpak cần portal `org.freedesktop.portal.IBus`; Snap có IM
dễ vỡ — đó là lý do Doctor nêu VD010.
