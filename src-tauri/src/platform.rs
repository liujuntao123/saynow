use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierHotkeyEvent {
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStatus {
    pub supported: bool,
    pub message: String,
}

pub fn current_platform_status() -> PlatformStatus {
    if cfg!(target_os = "windows") {
        PlatformStatus {
            supported: true,
            message: String::new(),
        }
    } else {
        PlatformStatus {
            supported: false,
            message: "当前环境不是 Windows，已跳过托盘、全局快捷键、录音和文本注入的真实系统调用。"
                .to_string(),
        }
    }
}

pub fn inject_text(text: &str) -> Result<(), String> {
    platform_impl::inject_text(text)
}

#[cfg(feature = "desktop")]
pub fn set_modifier_hotkey_monitor<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    parts: Option<Vec<String>>,
) -> Result<(), String> {
    platform_impl::set_modifier_hotkey_monitor(app, parts)
}

#[cfg(not(feature = "desktop"))]
pub fn set_modifier_hotkey_monitor(parts: Option<Vec<String>>) -> Result<(), String> {
    platform_impl::set_modifier_hotkey_monitor(parts)
}

#[cfg(target_os = "windows")]
mod platform_impl {
    use std::{
        collections::HashSet,
        mem::size_of,
        ptr::copy_nonoverlapping,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Mutex, OnceLock,
        },
        thread,
        time::Duration,
    };

    use tauri::{Emitter, Manager};

    use windows::Win32::{
        Foundation::{HANDLE, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        },
        UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
            VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
            VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_V,
        },
        UI::WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, PeekMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, WH_KEYBOARD_LL,
            WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    };

    use crate::platform::ModifierHotkeyEvent;

    const CF_UNICODETEXT: u32 = 13;
    static MONITOR: OnceLock<Mutex<Option<ModifierHotkeyMonitor>>> = OnceLock::new();
    static HOOK_CONTEXT: OnceLock<Mutex<Option<HookContext>>> = OnceLock::new();

    struct ModifierHotkeyMonitor {
        stop: mpsc::Sender<()>,
        thread: thread::JoinHandle<()>,
    }

    struct HookContext {
        required: HashSet<ModifierKey>,
        pressed: HashSet<ModifierKey>,
        active: bool,
        app: tauri::AppHandle,
        should_stop: AtomicBool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum ModifierKey {
        Ctrl,
        Alt,
        Shift,
        Meta,
    }

    pub fn inject_text(text: &str) -> Result<(), String> {
        eprintln!("[saynow] injecting text; chars={}", text.chars().count());
        set_clipboard_text(text)?;
        thread::sleep(Duration::from_millis(80));
        paste_from_clipboard()
    }

    pub fn set_modifier_hotkey_monitor<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        parts: Option<Vec<String>>,
    ) -> Result<(), String> {
        let app = app.clone().into();
        let monitor_lock = MONITOR.get_or_init(|| Mutex::new(None));
        let mut monitor = monitor_lock
            .lock()
            .map_err(|_| "无法锁定修饰键监听状态。".to_string())?;

        if let Some(current) = monitor.take() {
            eprintln!("[saynow] stopping native modifier hotkey monitor");
            current.stop.send(()).ok();
            current.thread.join().ok();
        }

        let Some(parts) = parts else {
            return Ok(());
        };
        let required = parse_modifier_parts(&parts)?;
        if required.is_empty() {
            return Err("修饰键快捷键不能为空。".to_string());
        }

        eprintln!("[saynow] starting native modifier hotkey monitor; parts={parts:?}");
        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::spawn(move || run_modifier_hook(app, required, stop_rx));
        *monitor = Some(ModifierHotkeyMonitor {
            stop: stop_tx,
            thread,
        });
        Ok(())
    }

    fn parse_modifier_parts(parts: &[String]) -> Result<HashSet<ModifierKey>, String> {
        let mut required = HashSet::new();
        for part in parts {
            match part.as_str() {
                "Ctrl" => {
                    required.insert(ModifierKey::Ctrl);
                }
                "Alt" => {
                    required.insert(ModifierKey::Alt);
                }
                "Shift" => {
                    required.insert(ModifierKey::Shift);
                }
                "Meta" => {
                    required.insert(ModifierKey::Meta);
                }
                other => return Err(format!("不支持的修饰键：{other}")),
            }
        }
        Ok(required)
    }

    fn run_modifier_hook(
        app: tauri::AppHandle,
        required: HashSet<ModifierKey>,
        stop_rx: mpsc::Receiver<()>,
    ) {
        if let Err(error) = install_modifier_hook(app, required, stop_rx) {
            eprintln!("[saynow] native modifier hotkey monitor failed: {error}");
        }
    }

    fn install_modifier_hook(
        app: tauri::AppHandle,
        required: HashSet<ModifierKey>,
        stop_rx: mpsc::Receiver<()>,
    ) -> Result<(), String> {
        {
            let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
            let mut context = context_lock
                .lock()
                .map_err(|_| "无法锁定修饰键监听上下文。".to_string())?;
            *context = Some(HookContext {
                required,
                pressed: HashSet::new(),
                active: false,
                app,
                should_stop: AtomicBool::new(false),
            });
        }

        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) }
            .map_err(|error| format!("无法安装键盘监听：{error}"))?;
        let _hook_guard = HookGuard(hook);
        eprintln!("[saynow] native modifier hotkey hook installed");

        let mut message = MSG::default();
        loop {
            if stop_rx.try_recv().is_ok() {
                mark_hook_stop();
                break;
            }

            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        clear_hook_context();
        eprintln!("[saynow] native modifier hotkey hook stopped");
        Ok(())
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let event = wparam.0 as u32;
            let data = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if let Some(key) = modifier_from_vk(data.vkCode) {
                let pressed = event == WM_KEYDOWN || event == WM_SYSKEYDOWN;
                let released = event == WM_KEYUP || event == WM_SYSKEYUP;
                if pressed || released {
                    handle_modifier_event(key, pressed);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    fn handle_modifier_event(key: ModifierKey, pressed: bool) {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        let Ok(mut context) = context_lock.lock() else {
            return;
        };
        let Some(context) = context.as_mut() else {
            return;
        };
        if context.should_stop.load(Ordering::Relaxed) {
            return;
        }

        if pressed {
            context.pressed.insert(key);
        } else {
            context.pressed.remove(&key);
        }

        let matches = context
            .required
            .iter()
            .all(|part| context.pressed.contains(part));
        if matches && !context.active {
            context.active = true;
            eprintln!("[saynow] native modifier hotkey pressed");
            emit_modifier_state(&context.app, "Pressed");
        } else if context.active && !matches {
            context.active = false;
            eprintln!("[saynow] native modifier hotkey released");
            emit_modifier_state(&context.app, "Released");
        }
    }

    fn modifier_from_vk(vk_code: u32) -> Option<ModifierKey> {
        let key = VIRTUAL_KEY(vk_code as u16);
        if key == VK_CONTROL || key == VK_LCONTROL || key == VK_RCONTROL {
            Some(ModifierKey::Ctrl)
        } else if key == VK_MENU || key == VK_LMENU || key == VK_RMENU {
            Some(ModifierKey::Alt)
        } else if key == VK_SHIFT || key == VK_LSHIFT || key == VK_RSHIFT {
            Some(ModifierKey::Shift)
        } else if key == VK_LWIN || key == VK_RWIN {
            Some(ModifierKey::Meta)
        } else {
            None
        }
    }

    fn emit_modifier_state(app: &tauri::AppHandle, state: &str) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit(
                "modifier-hotkey-state",
                ModifierHotkeyEvent {
                    state: state.to_string(),
                },
            );
        }
    }

    fn mark_hook_stop() {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        if let Ok(mut context) = context_lock.lock() {
            if let Some(context) = context.as_mut() {
                context.should_stop.store(true, Ordering::Relaxed);
            }
        }
    }

    fn clear_hook_context() {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        if let Ok(mut context) = context_lock.lock() {
            *context = None;
        }
    }

    struct HookGuard(HHOOK);

    impl Drop for HookGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = UnhookWindowsHookEx(self.0);
            }
        }
    }

    fn set_clipboard_text(text: &str) -> Result<(), String> {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let byte_len = wide.len() * size_of::<u16>();

        unsafe {
            OpenClipboard(None).map_err(|error| format!("无法打开剪贴板：{error}"))?;
            let clipboard_guard = ClipboardGuard;

            EmptyClipboard().map_err(|error| format!("无法清空剪贴板：{error}"))?;
            let handle = GlobalAlloc(GMEM_MOVEABLE, byte_len)
                .map_err(|error| format!("无法分配剪贴板内存：{error}"))?;
            let locked = GlobalLock(handle);
            if locked.is_null() {
                return Err("无法锁定剪贴板内存。".to_string());
            }

            copy_nonoverlapping(wide.as_ptr().cast::<u8>(), locked.cast::<u8>(), byte_len);
            let _ = GlobalUnlock(handle);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(handle.0)))
                .map_err(|error| format!("无法写入剪贴板：{error}"))?;

            std::mem::forget(clipboard_guard);
            CloseClipboard().map_err(|error| format!("无法关闭剪贴板：{error}"))?;
        }

        Ok(())
    }

    fn paste_from_clipboard() -> Result<(), String> {
        let inputs = [
            keyboard_input(VK_CONTROL, false),
            keyboard_input(VK_V, false),
            keyboard_input(VK_V, true),
            keyboard_input(VK_CONTROL, true),
        ];
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 {
            Ok(())
        } else {
            Err("已写入剪贴板，但模拟 Ctrl+V 失败。".to_string())
        }
    }

    fn keyboard_input(key: VIRTUAL_KEY, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: if key_up {
                        KEYEVENTF_KEYUP
                    } else {
                        Default::default()
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    struct ClipboardGuard;

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform_impl {
    #[cfg(feature = "desktop")]
    pub fn set_modifier_hotkey_monitor<R: tauri::Runtime>(
        _app: tauri::AppHandle<R>,
        parts: Option<Vec<String>>,
    ) -> Result<(), String> {
        if parts.is_some() {
            Err("当前环境不是 Windows，无法监听纯修饰键全局快捷键。".to_string())
        } else {
            Ok(())
        }
    }

    #[cfg(not(feature = "desktop"))]
    pub fn set_modifier_hotkey_monitor(_parts: Option<Vec<String>>) -> Result<(), String> {
        Ok(())
    }

    pub fn inject_text(_text: &str) -> Result<(), String> {
        Err("当前环境不是 Windows，无法执行文本注入；识别文本已保存在记录中。".to_string())
    }
}
