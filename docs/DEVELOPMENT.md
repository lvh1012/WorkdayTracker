# Development guide for Tauri and Rust newcomers

## Mental model

Một Tauri app có hai phần build riêng:

1. **Frontend:** React/TypeScript được Vite compile thành static HTML/CSS/JavaScript.
2. **Native shell:** Rust binary mở WebView2, cung cấp native menu/tray và gọi Windows APIs.

Trong development, Vite phục vụ frontend qua `localhost:1420`. Trong production, frontend files được nhúng vào native bundle; app không cần web server.

## Rust concepts xuất hiện trong project

### `Result<T, E>`

Rust không dùng exception cho expected failures. Một function có thể trả:

```rust
Result<Dashboard, String>
```

`Ok(value)` là thành công; `Err(error)` là thất bại. Operator `?` return sớm nếu gặp `Err` và chuyển error sang type phù hợp.

### Ownership và borrowing

`&Occurrence` nghĩa là function mượn read-only occurrence, không sở hữu và không được giữ nó sau khi function return. `&mut self` nghĩa là method cần exclusive mutable access.

### `Mutex<Repository>`

Tauri commands và Windows callback có thể chạy từ các threads khác nhau. `Mutex` bảo đảm tại một thời điểm chỉ có một operation dùng SQLite connection.

Nếu thấy `.lock()` trong code, guard trả về sẽ tự unlock khi ra khỏi scope nhờ RAII; không có lệnh unlock thủ công.

### `unsafe`

Rust compiler không thể chứng minh safety của raw Win32 handles và callbacks. `unsafe` không tự động nghĩa là code nguy hiểm; nó nghĩa là developer chịu trách nhiệm giữ các invariants mà compiler không kiểm tra được.

Project cô lập `unsafe` trong `windows_session.rs` và đặt safety comment ở mỗi boundary quan trọng.

### Conditional compilation

```rust
#[cfg(target_os = "windows")]
```

Chỉ compile block đó trên Windows. Non-Windows stub cho phép domain/repository code dễ phân tích hơn, nhưng app chính thức chỉ support Windows.

## Tauri commands và events

React gọi Rust:

```ts
const dashboard = await invoke<Dashboard>("get_dashboard");
```

Rust command:

```rust
#[tauri::command]
fn get_dashboard(...) -> Result<Dashboard, String> { ... }
```

Khi Rust update projection, nó emit `workday-updated`. React listen event này và query dashboard lại. Event chỉ là invalidation signal, không chứa mutable state snapshot; cách này tránh race giữa nhiều events liên tiếp.

## Thêm một command mới

1. Viết function có `#[tauri::command]` trong `lib.rs` hoặc module riêng.
2. Thêm function vào `tauri::generate_handler![...]`.
3. Định nghĩa matching TypeScript type.
4. Gọi bằng `invoke()`.
5. Validate input trong Rust; không tin frontend chỉ vì app local.

## Thay đổi database schema

MVP dùng idempotent `CREATE TABLE IF NOT EXISTS`. Trước khi release schema thay đổi thực sự:

1. thêm table `schema_migrations`;
2. tạo numbered migrations;
3. chạy tất cả migration trong transaction;
4. backup database trước destructive migration;
5. thêm test upgrade từ schema cũ.

Không sửa raw `events` rows để “fix” history. Nên thêm correction event hoặc rebuild projection.

## Debug checklist

### React không load

- Kiểm tra `npm run dev` có listen port 1420.
- Mở WebView DevTools trong debug build.
- Kiểm tra command name và camelCase DTO fields.

### Không nhận lock/unlock

- Xác nhận app vẫn chạy trong system tray.
- Kiểm tra `WTSRegisterSessionNotification` thành công.
- Đặt breakpoint/log ở `window_proc` cho `WM_WTSSESSION_CHANGE`.
- Xác nhận message-only window chưa bị destroy.

### Autostart không chạy

- Kiểm tra toggle trong app.
- Kiểm tra company Group Policy/endpoint security.
- Không tự thay đổi Registry để bypass company policy.

### Database inspection

Thoát app trước, copy toàn bộ data directory sang nơi tạm rồi dùng SQLite client trên bản copy:

```sql
SELECT * FROM events ORDER BY occurred_at_utc_ms;
SELECT * FROM workdays ORDER BY local_date DESC;
```

## Code quality rules

- Domain rules phải có Rust unit test.
- UI formatting logic phải có Vitest test.
- Mọi Win32 resource acquisition phải có teardown tương ứng.
- Không thêm Tauri permission nếu feature chưa cần.
- Không log raw personal history ra console trong release.
- Chạy `cargo fmt`, `clippy`, frontend build và cả hai test suites trước merge.

