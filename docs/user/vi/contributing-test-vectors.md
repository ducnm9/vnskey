# Hướng dẫn đóng góp Test Vector

## Thêm vector mới

1. Mở file `test-vectors/telex.toml` (hoặc tạo file mới trong thư mục `test-vectors/`).
2. Thêm entry theo format:

```toml
[[vectors]]
id = "T500"                    # ID duy nhất, không trùng
input_keys = "tieesng Vieejt"  # Chuỗi phím gõ theo Telex
expected_output = "tiếng Việt" # Kết quả mong đợi (Unicode NFC)
tags = ["basic", "two-word"]   # Tag để lọc
```

3. Đảm bảo `expected_output` ở dạng Unicode NFC (dùng các ký tự tiếng Việt
   precomposed, không dùng combining marks).
4. Chạy validate:

```bash
vietime-bench validate
```

5. Tạo PR.

## Quy tắc đặt ID

- Telex vectors: `T001` – `T999`
- Bug regression vectors: `BUG-<app>-<year>-<seq>` (ví dụ: `BUG-VSCode-2024-01`)
- Không đánh lại số khi xóa vector cũ

## Tag phổ biến

| Tag | Ý nghĩa |
|-----|---------|
| `basic` | Câu đơn giản |
| `tone` | Có dấu thanh |
| `modifier` | Có dấu phụ (â, ê, ô, ơ, ư, ă, đ) |
| `combined` | Tổ hợp dấu thanh + dấu phụ |
| `word` | Từ thông dụng |
| `edge` | Trường hợp đặc biệt |
| `regression` | Bug đã được báo cáo |
| `electron` | Liên quan đến ứng dụng Electron |
| `number-mixed` | Có số xen kẽ |

## Unicode NFC

`expected_output` **phải** ở dạng NFC. Kiểm tra:
- `ắ` → đúng (precomposed, U+1EAF)
- `a` + `̆` + `́` → sai (decomposed, 3 code points)

Tool `vietime-bench validate` sẽ bắt lỗi này tự động.
