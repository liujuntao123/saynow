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
    if text.trim().is_empty() {
        eprintln!("[saynow] skipped text injection for empty text");
        return Ok(());
    }

    platform_impl::inject_text(text)
}

pub fn remember_input_target() -> Result<(), String> {
    platform_impl::remember_input_target()
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
pub fn configure_no_activate_window(hwnd: isize) -> Result<(), String> {
    platform_impl::configure_no_activate_window(hwnd)
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
pub fn show_no_activate_window(hwnd: isize) -> Result<(), String> {
    platform_impl::show_no_activate_window(hwnd)
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
pub fn hide_window(hwnd: isize) -> Result<(), String> {
    platform_impl::hide_window(hwnd)
}

#[cfg(all(feature = "desktop", not(target_os = "windows")))]
pub fn configure_no_activate_window(_hwnd: isize) -> Result<(), String> {
    Ok(())
}

#[cfg(all(feature = "desktop", not(target_os = "windows")))]
pub fn show_no_activate_window(_hwnd: isize) -> Result<(), String> {
    Ok(())
}

#[cfg(all(feature = "desktop", not(target_os = "windows")))]
pub fn hide_window(_hwnd: isize) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "desktop")]
pub fn set_hotkey_monitor<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    parts: Option<Vec<String>>,
) -> Result<(), String> {
    platform_impl::set_hotkey_monitor(app, parts)
}

#[cfg(not(feature = "desktop"))]
pub fn set_hotkey_monitor(parts: Option<Vec<String>>) -> Result<(), String> {
    platform_impl::set_hotkey_monitor(parts)
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
        time::{Duration, Instant},
    };

    use tauri::{Emitter, Manager};

    use windows::core::w;
    use windows::Win32::{
        Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
            },
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
            VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_V,
        },
        UI::WindowsAndMessaging::{
            BringWindowToTop, CallNextHookEx, DispatchMessageW, GetForegroundWindow,
            GetWindowLongPtrW, GetWindowThreadProcessId, IsWindow, PeekMessageW,
            SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow,
            TranslateMessage, UnhookWindowsHookEx, GWL_EXSTYLE, HHOOK, HWND_TOPMOST,
            KBDLLHOOKSTRUCT, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE,
            SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    };

    use crate::platform::ModifierHotkeyEvent;

    const CF_UNICODETEXT: u32 = 13;
    const KEY_DOWN_MASK: i16 = i16::MIN;
    const HOOK_POLL_INTERVAL: Duration = Duration::from_millis(20);
    const HOTKEY_HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(100);
    const HOTKEY_AUTO_RELEASE_MISSES: u8 = 8;
    static MONITOR: OnceLock<Mutex<Option<ModifierHotkeyMonitor>>> = OnceLock::new();
    static HOOK_CONTEXT: OnceLock<Mutex<Option<HookContext>>> = OnceLock::new();
    static INPUT_TARGET: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

    struct ModifierHotkeyMonitor {
        stop: mpsc::Sender<()>,
        thread: thread::JoinHandle<()>,
    }

    struct HookContext {
        hotkey: HotkeySpec,
        pressed: HashSet<ModifierKey>,
        active: bool,
        release_miss_count: u8,
        emit_state: Box<dyn Fn(&str) + Send + Sync>,
        should_stop: AtomicBool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum ModifierKey {
        Ctrl,
        Alt,
        Shift,
        Meta,
    }

    #[derive(Debug, Clone)]
    struct HotkeySpec {
        modifiers: HashSet<ModifierKey>,
        trigger: Option<u32>,
    }

    pub fn inject_text(text: &str) -> Result<(), String> {
        eprintln!("[saynow] injecting text; chars={}", text.chars().count());
        let previous_clipboard_text = get_clipboard_text().unwrap_or_else(|error| {
            eprintln!("[saynow] failed to snapshot clipboard text before injection: {error}");
            None
        });
        set_clipboard_text(text, true)?;
        thread::sleep(Duration::from_millis(80));
        restore_input_target();
        thread::sleep(Duration::from_millis(40));
        let paste_result = paste_from_clipboard();
        if paste_result.is_ok() {
            let restore_result = match previous_clipboard_text {
                Some(previous_text) => set_clipboard_text(&previous_text, true),
                None => clear_clipboard(true),
            };
            if let Err(error) = restore_result {
                eprintln!("[saynow] failed to restore clipboard after injection: {error}");
            }
        }
        paste_result
    }

    pub fn remember_input_target() -> Result<(), String> {
        let hwnd = unsafe { GetForegroundWindow() };
        let target = if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        };
        let target_lock = INPUT_TARGET.get_or_init(|| Mutex::new(None));
        let mut current = target_lock
            .lock()
            .map_err(|_| "无法锁定输入目标窗口状态。".to_string())?;
        *current = target;
        eprintln!("[saynow] remembered input target; hwnd={target:?}");
        Ok(())
    }

    pub fn configure_no_activate_window(hwnd_value: isize) -> Result<(), String> {
        let hwnd = valid_hwnd(hwnd_value)?;
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        let next_style = style | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW).0 as isize;
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_style);
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .map_err(|error| format!("无法配置录音浮窗为非激活窗口：{error}"))?;
        }
        eprintln!("[saynow] configured recorder overlay as no-activate; hwnd={hwnd_value:?}");
        Ok(())
    }

    pub fn show_no_activate_window(hwnd_value: isize) -> Result<(), String> {
        let hwnd = valid_hwnd(hwnd_value)?;
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .map_err(|error| format!("无法以非激活方式显示录音浮窗：{error}"))?;
        }
        eprintln!("[saynow] showed recorder overlay without activation; hwnd={hwnd_value:?}");
        Ok(())
    }

    pub fn hide_window(hwnd_value: isize) -> Result<(), String> {
        let hwnd = valid_hwnd(hwnd_value)?;
        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        eprintln!("[saynow] hid recorder overlay; hwnd={hwnd_value:?}");
        Ok(())
    }

    fn valid_hwnd(hwnd_value: isize) -> Result<HWND, String> {
        let hwnd = HWND(hwnd_value as *mut core::ffi::c_void);
        if unsafe { IsWindow(Some(hwnd)).as_bool() } {
            Ok(hwnd)
        } else {
            Err("窗口句柄无效。".to_string())
        }
    }

    pub fn set_hotkey_monitor<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        parts: Option<Vec<String>>,
    ) -> Result<(), String> {
        let monitor_lock = MONITOR.get_or_init(|| Mutex::new(None));
        let mut monitor = monitor_lock
            .lock()
            .map_err(|_| "无法锁定热键监听状态。".to_string())?;

        if let Some(current) = monitor.take() {
            eprintln!("[saynow] stopping native hotkey monitor");
            current.stop.send(()).ok();
            current.thread.join().ok();
        }

        let Some(parts) = parts else {
            return Ok(());
        };
        let hotkey = parse_hotkey_parts(&parts)?;

        eprintln!("[saynow] starting native hotkey monitor; parts={parts:?}");
        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::spawn(move || run_hotkey_hook(app, hotkey, stop_rx));
        *monitor = Some(ModifierHotkeyMonitor {
            stop: stop_tx,
            thread,
        });
        Ok(())
    }

    fn parse_hotkey_parts(parts: &[String]) -> Result<HotkeySpec, String> {
        let mut modifiers = HashSet::new();
        let mut trigger = None;
        for part in parts {
            match part.as_str() {
                "Ctrl" => {
                    modifiers.insert(ModifierKey::Ctrl);
                }
                "Alt" => {
                    modifiers.insert(ModifierKey::Alt);
                }
                "Shift" => {
                    modifiers.insert(ModifierKey::Shift);
                }
                "Meta" => {
                    modifiers.insert(ModifierKey::Meta);
                }
                other => {
                    if trigger.replace(parse_trigger_key(other)?).is_some() {
                        return Err("热键只能包含一个非修饰键。".to_string());
                    }
                }
            }
        }
        if modifiers.is_empty() && trigger.is_none() {
            return Err("快捷键不能为空。".to_string());
        }
        Ok(HotkeySpec { modifiers, trigger })
    }

    fn parse_trigger_key(part: &str) -> Result<u32, String> {
        match part {
            "Space" => Ok(VK_SPACE.0 as u32),
            "Left" => Ok(0x25),
            "Up" => Ok(0x26),
            "Right" => Ok(0x27),
            "Down" => Ok(0x28),
            single if single.chars().count() == 1 => {
                Ok(single.chars().next().unwrap().to_ascii_uppercase() as u32)
            }
            function
                if function.len() >= 2
                    && function.starts_with('F')
                    && function[1..].parse::<u32>().is_ok() =>
            {
                let number = function[1..].parse::<u32>().unwrap();
                if (1..=24).contains(&number) {
                    Ok(0x70 + number - 1)
                } else {
                    Err(format!("不支持的功能键：{part}"))
                }
            }
            other => Err(format!("不支持的快捷键：{other}")),
        }
    }

    fn run_hotkey_hook<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        hotkey: HotkeySpec,
        stop_rx: mpsc::Receiver<()>,
    ) {
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            if let Err(error) = install_hotkey_hook(app.clone(), hotkey.clone(), &stop_rx) {
                eprintln!("[saynow] native hotkey monitor failed: {error}; retrying");
                clear_hook_context();
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            break;
        }
    }

    fn install_hotkey_hook<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        hotkey: HotkeySpec,
        stop_rx: &mpsc::Receiver<()>,
    ) -> Result<(), String> {
        let emit_app = app.clone();
        let emit_state = Box::new(move |state: &str| emit_modifier_state(&emit_app, state));
        {
            let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
            let mut context = context_lock
                .lock()
                .map_err(|_| "无法锁定热键监听上下文。".to_string())?;
            *context = Some(HookContext {
                hotkey,
                pressed: HashSet::new(),
                active: false,
                release_miss_count: 0,
                emit_state,
                should_stop: AtomicBool::new(false),
            });
        }

        let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0) }
        {
            Ok(hook) => hook,
            Err(error) => {
                clear_hook_context();
                return Err(format!("无法安装键盘监听：{error}"));
            }
        };
        let _hook_guard = HookGuard(hook);
        eprintln!("[saynow] native hotkey hook installed");

        let mut message = MSG::default();
        let mut last_health_check = Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() {
                release_active_hotkey("monitor stop requested");
                mark_hook_stop();
                break;
            }

            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if last_health_check.elapsed() >= HOTKEY_HEALTH_CHECK_INTERVAL {
                check_hotkey_health();
                last_health_check = Instant::now();
            }
            thread::sleep(HOOK_POLL_INTERVAL);
        }

        release_active_hotkey("hook cleanup");
        clear_hook_context();
        eprintln!("[saynow] native hotkey hook stopped");
        Ok(())
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let event = wparam.0 as u32;
            let data = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let pressed = event == WM_KEYDOWN || event == WM_SYSKEYDOWN;
            let released = event == WM_KEYUP || event == WM_SYSKEYUP;
            if pressed || released {
                let modifier = modifier_from_vk(data.vkCode);
                if handle_key_event(data.vkCode, modifier, pressed) {
                    return LRESULT(1);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    fn handle_key_event(vk_code: u32, modifier: Option<ModifierKey>, pressed: bool) -> bool {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        let Ok(mut context) = context_lock.lock() else {
            return false;
        };
        let Some(context) = context.as_mut() else {
            return false;
        };
        if context.should_stop.load(Ordering::Relaxed) {
            return false;
        }

        let owned_system_modifier = modifier.is_some_and(|key| {
            matches!(key, ModifierKey::Alt | ModifierKey::Meta)
                && context.hotkey.modifiers.contains(&key)
        });

        if pressed {
            context.release_miss_count = 0;
            if let Some(key) = modifier {
                context.pressed.insert(key);
            }

            if hotkey_matches(&context.hotkey, &context.pressed, vk_code) {
                if !context.active {
                    context.active = true;
                    let _ = remember_input_target();
                    eprintln!("[saynow] native hotkey pressed");
                    (context.emit_state)("Pressed");
                }
                return true;
            }

            return owned_system_modifier
                || (context.active && hotkey_contains_key(&context.hotkey, vk_code, modifier));
        }

        let releases_active_hotkey =
            context.active && hotkey_contains_key(&context.hotkey, vk_code, modifier);
        if releases_active_hotkey {
            context.active = false;
            context.release_miss_count = 0;
            eprintln!("[saynow] native hotkey released");
            (context.emit_state)("Released");
        }

        if let Some(key) = modifier {
            context.pressed.remove(&key);
        }
        owned_system_modifier || releases_active_hotkey
    }

    fn hotkey_matches(hotkey: &HotkeySpec, pressed: &HashSet<ModifierKey>, vk_code: u32) -> bool {
        hotkey.modifiers.iter().all(|part| pressed.contains(part))
            && hotkey.trigger.map_or(true, |trigger| trigger == vk_code)
    }

    fn hotkey_contains_key(
        hotkey: &HotkeySpec,
        vk_code: u32,
        modifier: Option<ModifierKey>,
    ) -> bool {
        if let Some(key) = modifier {
            hotkey.modifiers.contains(&key)
        } else {
            hotkey.trigger == Some(vk_code)
        }
    }

    fn check_hotkey_health() {
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

        if !context.active {
            context.release_miss_count = 0;
            context.pressed.retain(|key| modifier_is_down(*key));
            return;
        }

        if physical_hotkey_is_down(&context.hotkey) {
            context.release_miss_count = 0;
            return;
        }

        context.release_miss_count = context.release_miss_count.saturating_add(1);
        if context.release_miss_count >= HOTKEY_AUTO_RELEASE_MISSES {
            context.active = false;
            context.release_miss_count = 0;
            context.pressed.clear();
            eprintln!("[saynow] native hotkey auto-released by health check");
            (context.emit_state)("Released");
        }
    }

    fn physical_hotkey_is_down(hotkey: &HotkeySpec) -> bool {
        hotkey.modifiers.iter().all(|key| modifier_is_down(*key))
            && hotkey
                .trigger
                .map_or(true, |trigger| virtual_key_is_down(trigger))
    }

    fn modifier_is_down(key: ModifierKey) -> bool {
        match key {
            ModifierKey::Ctrl => {
                virtual_key_is_down(VK_CONTROL.0 as u32)
                    || virtual_key_is_down(VK_LCONTROL.0 as u32)
                    || virtual_key_is_down(VK_RCONTROL.0 as u32)
            }
            ModifierKey::Alt => {
                virtual_key_is_down(VK_MENU.0 as u32)
                    || virtual_key_is_down(VK_LMENU.0 as u32)
                    || virtual_key_is_down(VK_RMENU.0 as u32)
            }
            ModifierKey::Shift => {
                virtual_key_is_down(VK_SHIFT.0 as u32)
                    || virtual_key_is_down(VK_LSHIFT.0 as u32)
                    || virtual_key_is_down(VK_RSHIFT.0 as u32)
            }
            ModifierKey::Meta => {
                virtual_key_is_down(VK_LWIN.0 as u32) || virtual_key_is_down(VK_RWIN.0 as u32)
            }
        }
    }

    fn virtual_key_is_down(vk_code: u32) -> bool {
        unsafe { GetAsyncKeyState(vk_code as i32) & KEY_DOWN_MASK != 0 }
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

    fn emit_modifier_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: &str) {
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

    fn release_active_hotkey(reason: &str) {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        if let Ok(mut context) = context_lock.lock() {
            if let Some(context) = context.as_mut() {
                if context.active {
                    context.active = false;
                    context.release_miss_count = 0;
                    context.pressed.clear();
                    eprintln!("[saynow] native hotkey released; reason={reason}");
                    (context.emit_state)("Released");
                }
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

    fn get_clipboard_text() -> Result<Option<String>, String> {
        unsafe {
            OpenClipboard(None).map_err(|error| format!("无法打开剪贴板：{error}"))?;
            let _clipboard_guard = ClipboardGuard;

            if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
                return Ok(None);
            }

            let handle = GetClipboardData(CF_UNICODETEXT)
                .map_err(|error| format!("无法读取剪贴板文本：{error}"))?;
            if handle.0.is_null() {
                return Ok(None);
            }

            let global = HGLOBAL(handle.0);
            let locked = GlobalLock(global);
            if locked.is_null() {
                return Err("无法锁定剪贴板文本内存。".to_string());
            }

            let ptr = locked.cast::<u16>();
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Ok(Some(text))
        }
    }

    fn set_clipboard_text(text: &str, exclude_from_history: bool) -> Result<(), String> {
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
            if exclude_from_history {
                set_clipboard_history_exclusion()?;
            }

            std::mem::forget(clipboard_guard);
            CloseClipboard().map_err(|error| format!("无法关闭剪贴板：{error}"))?;
        }

        Ok(())
    }

    fn clear_clipboard(exclude_from_history: bool) -> Result<(), String> {
        unsafe {
            OpenClipboard(None).map_err(|error| format!("无法打开剪贴板：{error}"))?;
            let clipboard_guard = ClipboardGuard;
            EmptyClipboard().map_err(|error| format!("无法清空剪贴板：{error}"))?;
            if exclude_from_history {
                set_clipboard_history_exclusion()?;
            }
            std::mem::forget(clipboard_guard);
            CloseClipboard().map_err(|error| format!("无法关闭剪贴板：{error}"))?;
        }

        Ok(())
    }

    fn set_clipboard_history_exclusion() -> Result<(), String> {
        let format =
            unsafe { RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")) };
        if format == 0 {
            return Err("无法注册剪贴板历史排除格式。".to_string());
        }

        let value: u32 = 1;
        let byte_len = size_of::<u32>();
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE, byte_len)
                .map_err(|error| format!("无法分配剪贴板历史排除标记内存：{error}"))?;
            let locked = GlobalLock(handle);
            if locked.is_null() {
                return Err("无法锁定剪贴板历史排除标记内存。".to_string());
            }

            copy_nonoverlapping(
                (&value as *const u32).cast::<u8>(),
                locked.cast::<u8>(),
                byte_len,
            );
            let _ = GlobalUnlock(handle);
            SetClipboardData(format, Some(HANDLE(handle.0)))
                .map_err(|error| format!("无法写入剪贴板历史排除标记：{error}"))?;
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

    fn restore_input_target() {
        let Some(target_lock) = INPUT_TARGET.get() else {
            return;
        };
        let Ok(current) = target_lock.lock() else {
            return;
        };
        let Some(hwnd_value) = *current else {
            return;
        };
        let hwnd = HWND(hwnd_value as *mut core::ffi::c_void);
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            eprintln!("[saynow] skipped restoring missing input target; hwnd={hwnd_value:?}");
            return;
        }
        if !restore_foreground_window(hwnd) {
            eprintln!("[saynow] failed to restore input target; hwnd={hwnd_value:?}");
        }
    }

    fn restore_foreground_window(hwnd: HWND) -> bool {
        unsafe {
            let current_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let attached = target_thread != 0
                && target_thread != current_thread
                && AttachThreadInput(current_thread, target_thread, true).as_bool();
            let _ = BringWindowToTop(hwnd);
            let restored = SetForegroundWindow(hwnd).as_bool();
            if attached {
                let _ = AttachThreadInput(current_thread, target_thread, false);
            }
            restored
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
    pub fn set_hotkey_monitor<R: tauri::Runtime>(
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
    pub fn set_hotkey_monitor(_parts: Option<Vec<String>>) -> Result<(), String> {
        Ok(())
    }

    pub fn inject_text(_text: &str) -> Result<(), String> {
        Err("当前环境不是 Windows，无法执行文本注入；识别文本已保存在记录中。".to_string())
    }

    pub fn remember_input_target() -> Result<(), String> {
        Ok(())
    }
}
