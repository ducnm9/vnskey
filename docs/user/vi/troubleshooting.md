<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Xử lý sự cố gõ tiếng Việt trên Linux

## Câu hỏi thường gặp

### 1. Không gõ được tiếng Việt trong bất kỳ ứng dụng nào

**Triệu chứng**: Gõ Telex nhưng chỉ ra ký tự ASCII (ví dụ `aa` ra `aa` thay vì `â`).

**Nguyên nhân phổ biến**:
- Chưa cài bộ gõ (ibus-bamboo hoặc fcitx5-bamboo).
- Biến môi trường `GTK_IM_MODULE`, `QT_IM_MODULE`, `XMODIFIERS` chưa được set.
- Daemon bộ gõ chưa chạy.

**Khắc phục**:
```bash
# Chạy Doctor để xem lỗi cụ thể
vietime-doctor report

# Hoặc cài tự động
vietime-installer install fcitx5-bamboo
```

### 2. Gõ được trong gedit nhưng không được trong Firefox/Chrome

**Nguyên nhân**: Trình duyệt có thể dùng framework IM khác hoặc thiếu biến môi trường.

**Khắc phục**:
```bash
vietime-doctor report --app firefox
# Xem mục VD003 (EnvVarMismatch) và VD007 (ElectronWaylandNoOzone)
```

### 3. VS Code/Electron không nhận bộ gõ

**Nguyên nhân**: Electron trên Wayland cần flag `--enable-wayland-ime` hoặc `--ozone-platform=wayland`.

**Khắc phục**:
```bash
vietime-doctor report --app vscode
# Xem mục VD007 và VD010
```

Nếu VS Code cài qua Snap:
```bash
# Snap sandbox chặn IM module. Chuyển sang .deb:
sudo snap remove code
sudo apt install code  # từ Microsoft repo
```

### 4. Gõ được nhưng dấu thanh sai vị trí

**Triệu chứng**: Gõ `tieesng` ra `tiến` thay vì `tiếng`.

**Nguyên nhân**: Phiên bản bộ gõ cũ hoặc cấu hình sai.

**Khắc phục**:
```bash
# Kiểm tra version
ibus version
# Nếu ibus-bamboo < 0.8.3, cần cập nhật
vietime-installer install ibus-bamboo  # cài bản mới nhất
```

### 5. Gõ tiếng Việt trong terminal nhưng không được trong GUI

**Nguyên nhân**: Terminal thường dùng trực tiếp X11 input, còn GUI app dùng IM module qua GTK/Qt.

**Khắc phục**: Kiểm tra biến môi trường:
```bash
echo $GTK_IM_MODULE    # phải là "ibus" hoặc "fcitx"
echo $QT_IM_MODULE     # phải là "ibus" hoặc "fcitx"
echo $XMODIFIERS       # phải là "@im=ibus" hoặc "@im=fcitx"
```

### 6. Sau khi cài xong vẫn không gõ được — cần restart?

**Đúng vậy**. Sau khi cài bộ gõ, cần **logout rồi login lại** (hoặc restart) để biến môi trường có hiệu lực.

### 7. Ibus và Fcitx5 cùng chạy — conflict

**Triệu chứng**: Doctor báo VD002 (ImFrameworkConflict).

**Khắc phục**:
```bash
# Chọn 1 trong 2, gỡ cái còn lại
vietime-installer install fcitx5-bamboo  # sẽ tự gỡ ibus nếu có
```

### 8. Flatpak app không nhận bộ gõ

**Nguyên nhân**: Flatpak sandbox cần portal IM.

**Khắc phục**: Cài `xdg-desktop-portal-gtk` (cho IBus) hoặc `fcitx5-frontend-gtk3` (cho Fcitx5). Doctor sẽ báo VD011 nếu thiếu.

### 9. Wayland session — IBus không hoạt động ổn định

**Nguyên nhân**: IBus trên Wayland có bug đã biết với một số compositor.

**Gợi ý**: Chuyển sang Fcitx5 nếu dùng Wayland:
```bash
vietime-installer install fcitx5-bamboo
```

Xem thêm bảng tương thích tại [bench.md](bench.md).

### 10. LibreOffice không nhận bộ gõ

**Khắc phục**:
```bash
# LibreOffice cần biến riêng
echo 'SAL_USE_VCLPLUGIN=gtk3' >> ~/.profile
# Logout rồi login lại
```

### 11. Chromium/Chrome trên Wayland

**Khắc phục**: Thêm flag:
```bash
# Trong ~/.config/chromium-flags.conf hoặc chrome-flags.conf
--enable-wayland-ime
--ozone-platform=wayland
```

### 12. Benchmark cho biết accuracy thấp — có nên lo?

**Giải thích**: Accuracy dưới 95% nghĩa là combo đó có bug thực sự. Xem [bench.md](bench.md) để chọn combo tốt nhất cho ứng dụng bạn dùng.

### 13. Muốn đóng góp test vector

Xem [contributing-test-vectors.md](contributing-test-vectors.md).

### 14. Doctor report có lộ thông tin cá nhân không?

Mặc định **không**. Doctor tự động ẩn username, hostname, và đường dẫn home. Dùng `--no-redact` nếu cần gửi cho maintainer debug.

### 15. Lỗi "ibus-daemon not running" nhưng đã cài ibus

**Khắc phục**:
```bash
ibus-daemon -drx   # khởi động daemon
# Nếu muốn tự động chạy khi login:
cp /usr/share/applications/ibus-daemon.desktop ~/.config/autostart/
```

### 16. Fedora/RHEL — fcitx5-bamboo không có trong repo chính

Dùng COPR:
```bash
sudo dnf copr enable phuongdong/fcitx5-bamboo
sudo dnf install fcitx5-bamboo
```

### 17. Arch Linux — cài bộ gõ từ AUR

```bash
yay -S ibus-bamboo  # hoặc fcitx5-bamboo
```

### 18. Làm sao kiểm tra bộ gõ đang hoạt động?

```bash
# IBus
ibus engine     # hiện engine đang active

# Fcitx5
fcitx5-remote -n   # hiện input method đang dùng
```

### 19. Gõ nhanh bị mất ký tự

**Nguyên nhân**: Delay giữa các keystroke quá thấp, IME chưa kịp xử lý.

**Gợi ý**: Trong ibus-bamboo, tăng "Delay" setting lên 30-50ms.

### 20. Muốn test bộ gõ trên nhiều app cùng lúc

Dùng vietime-bench:
```bash
vietime-bench run --profile smoke
vietime-bench report --format markdown
```

## Cần thêm trợ giúp?

- Mở issue trên GitHub kèm output `vietime-doctor report --json`.
- Xem [bảng tương thích](bench.md) để chọn combo phù hợp.
