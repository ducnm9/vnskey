# VietIME Bench — Bảng tương thích gõ tiếng Việt trên Linux

## Giới thiệu

VietIME Bench là công cụ tự động kiểm tra khả năng gõ tiếng Việt trên các
ứng dụng Linux. Nó tạo ra **bảng tương thích** (compatibility matrix) cho biết
mỗi tổ hợp (bộ gõ × ứng dụng × phiên × kiểu gõ) hoạt động chính xác đến
mức nào.

## Cách đọc bảng tương thích

Bảng hiển thị các cột:

| Cột | Ý nghĩa |
|-----|---------|
| Engine | Bộ gõ: `ibus-bamboo`, `fcitx5-bamboo`, `ibus-unikey`, `fcitx5-unikey` |
| App | Ứng dụng: gedit, kate, firefox, chromium, vscode, libreoffice, ... |
| Session | Phiên hiển thị: `x11` hoặc `wayland` |
| Mode | Kiểu gõ: `telex`, `vni`, `viqr`, `simple-telex` |
| Accuracy | Tỷ lệ chính xác (%) — số câu gõ đúng / tổng số câu |
| Exact | Số câu khớp hoàn toàn / tổng |
| Edit Dist | Tổng khoảng cách chỉnh sửa (Levenshtein) |

### Mã màu

- **Xanh (≥95%)**: Hoạt động tốt, dùng được ngay.
- **Vàng (80-95%)**: Có lỗi nhỏ, cần kiểm tra thêm.
- **Đỏ (<80%)**: Nhiều lỗi, không nên dùng combo này.

## Sử dụng

```bash
# Cài đặt
cargo install vietime-bench

# Chạy nhanh
vietime-bench run --profile smoke

# Xem kết quả
vietime-bench report --format markdown

# So sánh 2 lần chạy
vietime-bench compare --base <run-1> --head <run-2>

# Xem chi tiết lỗi
vietime-bench inspect <run-id> <vector-id>
```

## Kiểm tra vector

Bộ vector hiện tại gồm 500+ câu tiếng Việt bao gồm:
- Dấu thanh: sắc, huyền, hỏi, ngã, nặng
- Dấu phụ: â, ê, ô, ơ, ư, ă, đ
- Tổ hợp dấu: ấ, ầ, ẩ, ẫ, ậ, ...
- Từ thường dùng: tiếng Việt, xin chào, đường, người, ...
- Edge cases: số xen kẽ, chữ hoa, dấu câu

## Đóng góp thêm vector

Xem [contributing-test-vectors.md](contributing-test-vectors.md).

## FAQ

**Q: Tại sao accuracy không phải 100%?**
A: Một số tổ hợp bộ gõ + ứng dụng có bug thực sự (ví dụ Electron + IBus
trên Wayland). Bench ghi nhận đúng thực trạng.

**Q: Tôi nên chọn combo nào?**
A: Xem bảng tương thích, chọn combo có accuracy cao nhất cho ứng dụng bạn
dùng. Thông thường `fcitx5-bamboo` + X11 hoạt động ổn định nhất.
