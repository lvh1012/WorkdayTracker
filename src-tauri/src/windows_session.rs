use crate::domain::EventKind;

#[cfg(target_os = "windows")]
mod implementation {
    use std::{
        sync::{OnceLock, mpsc},
        thread::{self, JoinHandle},
    };

    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
            System::{
                LibraryLoader::GetModuleHandleW,
                RemoteDesktop::{
                    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
                    WTSUnRegisterSessionNotification,
                },
            },
            UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, PostMessageW, PostQuitMessage,
                RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
                WM_DESTROY, WM_ENDSESSION, WM_POWERBROADCAST, WM_QUERYENDSESSION,
                WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_SESSION_LOCK, WTS_SESSION_LOGOFF,
                WTS_SESSION_LOGON, WTS_SESSION_UNLOCK,
            },
        },
        core::{Error as WindowsError, Result as WindowsResult, w},
    };

    use super::EventKind;

    static EVENT_CALLBACK: OnceLock<Box<dyn Fn(EventKind) + Send + Sync>> = OnceLock::new();

    /// A dedicated hidden Win32 top-level window.
    ///
    /// We intentionally do not subclass Tauri's WebView window. Keeping native session messages
    /// on their own thread avoids coupling tracking reliability to WebView window recreation.
    /// It must remain top-level because message-only windows do not receive system broadcasts.
    pub struct WindowsSessionMonitor {
        window_handle: isize,
        thread: Option<JoinHandle<()>>,
    }

    impl WindowsSessionMonitor {
        pub fn start(callback: impl Fn(EventKind) + Send + Sync + 'static) -> Result<Self, String> {
            EVENT_CALLBACK
                .set(Box::new(callback))
                .map_err(|_| "Windows session monitor was initialized twice".to_owned())?;

            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            let thread = thread::spawn(move || {
                // SAFETY: All Win32 window creation and message-loop operations remain on this
                // dedicated thread. The raw HWND is only used cross-thread with PostMessageW,
                // which is explicitly designed for this purpose.
                let result = unsafe { run_message_loop(ready_sender) };
                if let Err(error) = result {
                    eprintln!("Windows session monitor stopped: {error}");
                }
            });

            let window_handle = ready_receiver
                .recv()
                .map_err(|_| "Windows session monitor exited during startup".to_owned())?
                .map_err(|error| format!("Cannot initialize Windows session monitor: {error}"))?;

            Ok(Self {
                window_handle,
                thread: Some(thread),
            })
        }
    }

    impl Drop for WindowsSessionMonitor {
        fn drop(&mut self) {
            // SAFETY: Posting WM_CLOSE from another thread is supported by Win32. The window
            // procedure performs the unregister/destroy sequence on its owning thread.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.window_handle as *mut _)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    unsafe fn run_message_loop(
        ready_sender: mpsc::SyncSender<Result<isize, String>>,
    ) -> WindowsResult<()> {
        let module = unsafe { GetModuleHandleW(None)? };
        let instance = HINSTANCE(module.0);
        let class_name = w!("WorkdayTrackerSessionMessageWindow");
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name,
            ..Default::default()
        };

        if unsafe { RegisterClassW(&window_class) } == 0 {
            let error = WindowsError::from_thread();
            let _ = ready_sender.send(Err(error.to_string()));
            return Err(error);
        }

        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("WorkdayTrackerSessionMonitor"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                // A parentless, zero-sized window remains invisible while still receiving
                // WM_POWERBROADCAST and end-session broadcasts.
                None,
                None,
                Some(instance),
                None,
            )?
        };

        unsafe { WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION)? };
        let _ = ready_sender.send(Ok(window.0 as isize));

        let mut message = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if result.0 == -1 {
                return Err(WindowsError::from_thread());
            }
            if !result.as_bool() {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        Ok(())
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        match message {
            WM_WTSSESSION_CHANGE => {
                let kind = match w_param.0 as u32 {
                    WTS_SESSION_LOGON => Some(EventKind::SessionLogon),
                    WTS_SESSION_LOGOFF => Some(EventKind::SessionLogoff),
                    WTS_SESSION_LOCK => Some(EventKind::SessionLock),
                    WTS_SESSION_UNLOCK => Some(EventKind::SessionUnlock),
                    _ => None,
                };
                if let Some(kind) = kind {
                    send(kind);
                }
                LRESULT(0)
            }
            WM_POWERBROADCAST => {
                match w_param.0 as u32 {
                    PBT_APMSUSPEND => send(EventKind::SystemSuspend),
                    PBT_APMRESUMEAUTOMATIC => send(EventKind::SystemResume),
                    _ => {}
                }
                LRESULT(1)
            }
            WM_QUERYENDSESSION => {
                // Persist before Windows starts terminating processes. Returning TRUE allows
                // shutdown/logoff to continue; this app must never block the user's session.
                send(EventKind::SystemShutdown);
                LRESULT(1)
            }
            WM_ENDSESSION => {
                if w_param.0 != 0 {
                    send(EventKind::SystemShutdown);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = unsafe { WTSUnRegisterSessionNotification(window) };
                let _ = unsafe { DestroyWindow(window) };
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
        }
    }

    fn send(kind: EventKind) {
        if let Some(callback) = EVENT_CALLBACK.get() {
            // This is deliberately synchronous. WM_QUERYENDSESSION must not return until the
            // shutdown candidate has reached SQLite, otherwise Windows may terminate the process
            // while the event is still queued on another thread.
            callback(kind);
        }
    }
}

#[cfg(target_os = "windows")]
pub use implementation::WindowsSessionMonitor;

#[cfg(not(target_os = "windows"))]
pub struct WindowsSessionMonitor;

#[cfg(not(target_os = "windows"))]
impl WindowsSessionMonitor {
    pub fn start(_callback: impl Fn(EventKind) + Send + Sync + 'static) -> Result<Self, String> {
        Ok(Self)
    }
}
