# Workday Tracker

Một Windows desktop app nhỏ dùng **Tauri 2 + React + Rust** để ghi lại lần đăng nhập đầu tiên trong ngày, lần rời cuối cùng và tổng thời gian ở công ty.

App được thiết kế cho đúng một người dùng trên một laptop:

- không account hoặc authentication;
- không backend, cloud sync hoặc telemetry;
- không gửi network request khi chạy;
- database chỉ tồn tại trong Windows profile hiện tại.

> Đây là heuristic cá nhân, không phải hệ thống chấm công. Windows biết laptop bị lock hoặc logoff, nhưng không biết người dùng có thực sự đang ở công ty hay không.

## App ghi nhận thời gian như thế nào?

- **Giờ đến:** lần app khởi động đầu tiên trong ngày sau khi Windows login.
- **Departure candidate:** lần `lock`, `logoff`, `shutdown` hoặc `suspend` gần nhất.
- **Grace period:** candidate chỉ được xem là giờ rời sau 30 phút.
- **Quay lại:** `unlock`, `logon` hoặc `resume` hủy candidate và tiếp tục ngày làm việc.
- **Shutdown:** candidate được persist trước khi process dừng và được finalize ở lần chạy ngày tiếp theo.

Chi tiết state machine và edge cases nằm trong [docs/TRACKING.md](docs/TRACKING.md).

## Tính năng MVP

- React dashboard: giờ đến, giờ rời, tổng thời gian hôm nay.
- Lịch sử theo ngày, có filter độc lập theo năm và tháng.
- Theme sáng, tối hoặc tự động theo Windows; preference được lưu local.
- Native system tray; đóng cửa sổ chỉ ẩn app, không dừng tracking.
- Chạy cùng Windows cho user hiện tại, không cần admin.
- SQLite immutable event log và daily projection.
- Single-instance: mở app lần hai sẽ focus cửa sổ đang chạy.
- NSIS per-user installer được build bởi GitHub Actions.

## Cài đặt để sử dụng

1. Mở tab **Actions** của repository.
2. Mở workflow `build-test-package` mới nhất đã chạy thành công.
3. Tải artifact `WorkdayTracker-windows-x64`.
4. Giải nén và chạy file `.exe` installer.
5. Windows SmartScreen có thể cảnh báo vì installer chưa được code-sign. Trên company laptop, không bypass policy; hãy nhờ IT whitelist nếu policy chặn app.

Installer dùng `currentUser`, vì vậy không cần quyền admin. Lần chạy đầu tiên app tự bật autostart; có thể thay đổi setting này trong dashboard.

## Development nhanh

### Prerequisites trên Windows

- Windows 10 hoặc 11 với WebView2 Runtime.
- [Node.js 24 LTS](https://nodejs.org/).
- [Rust 1.97.1 qua rustup](https://www.rust-lang.org/tools/install).
- Microsoft C++ Build Tools với workload **Desktop development with C++**.
- WebView2 development prerequisites theo [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

### Chạy app

```powershell
git clone https://github.com/lvh1012/WorkdayTracker.git
cd WorkdayTracker
npm ci
npm run tauri dev
```

`npm run tauri dev` thực hiện hai phần song song:

1. Vite chạy React development server.
2. Cargo compile Rust backend rồi Tauri mở native window chứa WebView2.

### Test và build

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
npm run tauri build
```

Installer được tạo tại:

```text
src-tauri/target/release/bundle/nsis/
```

Đọc [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) nếu bạn chưa quen Rust/Tauri. Kiến trúc và trust boundaries nằm trong [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Data và privacy

Database:

```text
%LOCALAPPDATA%\com.lvh1012.workdaytracker\workday-tracker.db
```

SQLite dùng WAL, vì vậy khi app đang chạy có thể thấy thêm file `-wal` và `-shm`. Không copy riêng file `.db` trong lúc app đang ghi; thoát app trước khi backup cả thư mục.

Database và log files đã bị loại khỏi Git bằng `.gitignore`.

## Minute precision trên UI

SQLite vẫn lưu timestamp chính xác tới millisecond. UI chỉ hiển thị giờ và phút, vì vậy tổng thời gian cũng normalize hai endpoints về minute precision trước khi trừ. Ví dụ `08:33:58 → 13:33:02` được hiển thị `08:33 → 13:33` và tổng `05:00`, thay vì `04:59` gây hiểu nhầm.

History filters và theme preference chỉ là presentation state trong React. Chúng không sửa raw events hoặc daily projection trong SQLite.

## Repository map

```text
src/                         React UI
src-tauri/src/domain.rs      Event types và DTOs
src-tauri/src/repository.rs  SQLite event log + daily projection
src-tauri/src/windows_session.rs  Native Win32 message listener
src-tauri/src/lib.rs         Tauri composition root, commands, tray, timers
docs/                        Architecture và onboarding documents
.github/workflows/           Windows build/test/package pipeline
```

## Known limitations

- Login Windows ở nhà vẫn bị xem là “đến công ty”. MVP không kiểm tra Wi-Fi, network hoặc location.
- Duration là khoảng thời gian từ arrival tới departure; nghỉ trưa và lock ngắn vẫn được tính.
- System clock/timezone thay đổi có thể làm local calendar day khác dự kiến; duration luôn dùng UTC để tránh DST arithmetic errors.
- Installer chưa code-sign nên phụ thuộc company policy.
- App không sửa lịch sử thủ công trong MVP. Raw event log được giữ để tính năng correction có thể thêm sau mà không mất audit trail.

