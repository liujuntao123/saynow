use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStateEvent {
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

pub fn restore_input_target() -> Result<(), String> {
    platform_impl::restore_input_target()
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
            GetAsyncKeyState, SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
            VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_V,
        },
        UI::WindowsAndMessaging::{
            BringWindowToTop, CallNextHookEx, DispatchMessageW, GetForegroundWindow,
            GetGUIThreadInfo, GetWindowLongPtrW, GetWindowThreadProcessId, IsWindow, PeekMessageW,
            SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowsHookExW, ShowWindow,
            TranslateMessage, UnhookWindowsHookEx, GUITHREADINFO, GWL_EXSTYLE, HHOOK, HWND_TOPMOST,
            KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
            WM_SYSKEYDOWN, WM_SYSKEYUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    };

    use crate::platform::HotkeyStateEvent;

    const CF_UNICODETEXT: u32 = 13;
    const KEY_DOWN_MASK: i16 = i16::MIN;
    const HOOK_POLL_INTERVAL: Duration = Duration::from_millis(20);
    const HOTKEY_HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(100);
    const HOTKEY_AUTO_RELEASE_MISSES: u8 = 8;
    const HOTKEY_HOLD_DELAY: Duration = Duration::from_millis(200);
    const CLIPBOARD_OPEN_RETRIES: usize = 8;
    const CLIPBOARD_OPEN_RETRY_DELAY: Duration = Duration::from_millis(20);
    const INPUT_TARGET_SETTLE_DELAY: Duration = Duration::from_millis(80);
    const PASTE_COMPLETION_DELAY: Duration = Duration::from_millis(220);
    const KEY_CHORD_DELAY: Duration = Duration::from_millis(15);
    const FOCUS_RESTORE_RETRIES: usize = 3;
    const FOCUS_RESTORE_RETRY_DELAY: Duration = Duration::from_millis(35);
    const SAYNOW_INPUT_MARKER: usize = 0x5A1E_2026;
    static MONITOR: OnceLock<Mutex<Option<HotkeyMonitor>>> = OnceLock::new();
    static HOOK_CONTEXT: OnceLock<Mutex<Option<HookContext>>> = OnceLock::new();
    static INPUT_TARGET: OnceLock<Mutex<Option<InputTarget>>> = OnceLock::new();

    #[derive(Debug, Clone, Copy)]
    struct InputTarget {
        hwnd: isize,
        focus_hwnd: Option<isize>,
    }

    struct HotkeyMonitor {
        stop: mpsc::Sender<()>,
        thread: thread::JoinHandle<()>,
    }

    struct HookContext {
        hotkey: HotkeySpec,
        pressed: HashSet<HotkeyKey>,
        captured_keys: HashSet<HotkeyKey>,
        captured_order: Vec<HotkeyKey>,
        state: HotkeyState,
        release_miss_count: u8,
        emit_state: Box<dyn Fn(&str) + Send + Sync>,
        should_stop: AtomicBool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HotkeyState {
        Idle,
        Candidate { started_at: Instant },
        Recording,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum HotkeyKey {
        Modifier(ModifierKey),
        Virtual(u32),
    }

    impl HotkeyState {
        fn recording(&self) -> bool {
            matches!(self, HotkeyState::Recording)
        }

        fn candidate_started_at(&self) -> Option<Instant> {
            match self {
                HotkeyState::Candidate { started_at } => Some(*started_at),
                _ => None,
            }
        }
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
        required_keys: HashSet<HotkeyKey>,
        capture_events: bool,
    }

    pub fn inject_text(text: &str) -> Result<(), String> {
        eprintln!("[saynow] injecting text; chars={}", text.chars().count());
        let previous_clipboard_text = get_clipboard_text().unwrap_or_else(|error| {
            eprintln!("[saynow] failed to snapshot clipboard text before injection: {error}");
            None
        });
        set_clipboard_text(text, true)?;
        thread::sleep(CLIPBOARD_OPEN_RETRY_DELAY);
        let target_restored = restore_input_target_internal()?;
        if !target_restored {
            return Err("已写入剪贴板，但无法恢复输入目标窗口，请手动粘贴。".to_string());
        }
        thread::sleep(INPUT_TARGET_SETTLE_DELAY);
        release_all_modifiers();
        thread::sleep(KEY_CHORD_DELAY);
        let paste_result = paste_from_clipboard();
        if paste_result.is_ok() {
            thread::sleep(PASTE_COMPLETION_DELAY);
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
            Some(InputTarget {
                hwnd: hwnd.0 as isize,
                focus_hwnd: focused_child_window(hwnd).map(|focus| focus.0 as isize),
            })
        };
        let target_lock = INPUT_TARGET.get_or_init(|| Mutex::new(None));
        let mut current = target_lock
            .lock()
            .map_err(|_| "无法锁定输入目标窗口状态。".to_string())?;
        *current = target;
        eprintln!("[saynow] remembered input target; target={target:?}");
        Ok(())
    }

    pub fn restore_input_target() -> Result<(), String> {
        restore_input_target_internal().map(|_| ())
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
        *monitor = Some(HotkeyMonitor {
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
        let required_keys = modifiers
            .iter()
            .copied()
            .map(HotkeyKey::Modifier)
            .chain(trigger.map(HotkeyKey::Virtual))
            .collect();
        let capture_events = modifiers.contains(&ModifierKey::Alt);
        Ok(HotkeySpec {
            modifiers,
            required_keys,
            capture_events,
        })
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
        let emit_state = Box::new(move |state: &str| emit_hotkey_state(&emit_app, state));
        {
            let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
            let mut context = context_lock
                .lock()
                .map_err(|_| "无法锁定热键监听上下文。".to_string())?;
            *context = Some(HookContext {
                hotkey,
                pressed: HashSet::new(),
                captured_keys: HashSet::new(),
                captured_order: Vec::new(),
                state: HotkeyState::Idle,
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
            check_hotkey_candidate();
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
                if data.flags.contains(LLKHF_INJECTED) && data.dwExtraInfo == SAYNOW_INPUT_MARKER {
                    return unsafe { CallNextHookEx(None, code, wparam, lparam) };
                }
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

        let key = event_key(vk_code, modifier);
        let should_replay_capture = should_replay_captured_prefix(context, key, pressed);
        let should_capture = should_capture_event(context, key, pressed);
        let release_reason = if pressed {
            context.release_miss_count = 0;
            handle_hotkey_key_pressed(context, key)
        } else {
            handle_hotkey_key_released(context, key)
        };

        if !pressed && modifier.is_some() && !context.state.recording() {
            clear_stale_non_modifier_keys(context);
        }

        if let Some(reason) = release_reason {
            release_context_hotkey(context, reason);
        }

        if should_replay_capture {
            replay_captured_key_downs(context);
        }

        if should_capture {
            track_captured_event(context, key, pressed);
        }

        should_capture
    }

    fn handle_hotkey_key_pressed(
        context: &mut HookContext,
        key: HotkeyKey,
    ) -> Option<&'static str> {
        if context.pressed.contains(&key) {
            return None;
        }

        context.pressed.insert(key);

        if context.hotkey.required_keys.contains(&key) {
            if matches!(context.state, HotkeyState::Idle)
                && hotkey_exactly_pressed(&context.hotkey, &context.pressed)
            {
                context.state = HotkeyState::Candidate {
                    started_at: start_hotkey_candidate(),
                };
            }
            return None;
        }

        if matches!(context.state, HotkeyState::Candidate { .. }) {
            cancel_hotkey_candidate(context);
        }
        if context.state.recording() {
            return Some("interrupted");
        }

        None
    }

    fn handle_hotkey_key_released(
        context: &mut HookContext,
        key: HotkeyKey,
    ) -> Option<&'static str> {
        context.pressed.remove(&key);

        if context.hotkey.required_keys.contains(&key) {
            if matches!(context.state, HotkeyState::Candidate { .. }) {
                cancel_hotkey_candidate(context);
            }
            if context.state.recording() {
                return Some("released");
            }
        }

        None
    }

    fn event_key(vk_code: u32, modifier: Option<ModifierKey>) -> HotkeyKey {
        if let Some(key) = modifier {
            HotkeyKey::Modifier(key)
        } else {
            HotkeyKey::Virtual(vk_code)
        }
    }

    fn hotkey_exactly_pressed(hotkey: &HotkeySpec, pressed: &HashSet<HotkeyKey>) -> bool {
        *pressed == hotkey.required_keys
    }

    #[cfg(test)]
    fn start_hotkey_candidate() -> Instant {
        Instant::now()
    }

    #[cfg(not(test))]
    fn start_hotkey_candidate() -> Instant {
        let _ = remember_input_target();
        Instant::now()
    }

    fn cancel_hotkey_candidate(context: &mut HookContext) {
        context.state = HotkeyState::Idle;
    }

    fn check_hotkey_candidate() {
        let context_lock = HOOK_CONTEXT.get_or_init(|| Mutex::new(None));
        let Ok(mut context) = context_lock.lock() else {
            return;
        };
        let Some(context) = context.as_mut() else {
            return;
        };
        if context.should_stop.load(Ordering::Relaxed) || context.state.recording() {
            return;
        }

        let Some(started_at) = context.state.candidate_started_at() else {
            return;
        };
        if started_at.elapsed() < HOTKEY_HOLD_DELAY {
            return;
        }
        if !tracked_hotkey_is_down(context) {
            cancel_hotkey_candidate(context);
            return;
        }

        context.state = HotkeyState::Recording;
        context.release_miss_count = 0;
        eprintln!("[saynow] native hotkey pressed after hold");
        (context.emit_state)("Pressed");
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

        if !context.state.recording() {
            context.release_miss_count = 0;
            retain_physically_pressed_keys(context);
            return;
        }

        if tracked_hotkey_is_down(context) {
            context.release_miss_count = 0;
            return;
        }

        context.release_miss_count = context.release_miss_count.saturating_add(1);
        if context.release_miss_count >= HOTKEY_AUTO_RELEASE_MISSES {
            eprintln!("[saynow] native hotkey auto-released by health check");
            release_context_hotkey(context, "health check");
        }
    }

    fn tracked_hotkey_is_down(context: &HookContext) -> bool {
        if context.hotkey.capture_events {
            return hotkey_exactly_pressed(&context.hotkey, &context.pressed);
        }

        hotkey_exactly_pressed(&context.hotkey, &context.pressed)
            && all_unconfigured_modifiers_are_up(&context.hotkey)
            && context
                .hotkey
                .required_keys
                .iter()
                .all(|key| hotkey_key_is_down(*key))
    }

    fn all_unconfigured_modifiers_are_up(hotkey: &HotkeySpec) -> bool {
        [
            ModifierKey::Ctrl,
            ModifierKey::Alt,
            ModifierKey::Shift,
            ModifierKey::Meta,
        ]
        .into_iter()
        .all(|key| hotkey.modifiers.contains(&key) || !modifier_is_down(key))
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

    fn hotkey_key_is_down(key: HotkeyKey) -> bool {
        match key {
            HotkeyKey::Modifier(modifier) => modifier_is_down(modifier),
            HotkeyKey::Virtual(vk_code) => virtual_key_is_down(vk_code),
        }
    }

    fn retain_physically_pressed_keys(context: &mut HookContext) {
        if context.hotkey.capture_events {
            return;
        }

        context.pressed.retain(|key| hotkey_key_is_down(*key));
        if matches!(context.state, HotkeyState::Candidate { .. })
            && !hotkey_exactly_pressed(&context.hotkey, &context.pressed)
        {
            cancel_hotkey_candidate(context);
        }
    }

    fn clear_stale_non_modifier_keys(context: &mut HookContext) {
        if context
            .pressed
            .iter()
            .any(|key| matches!(key, HotkeyKey::Modifier(_)))
        {
            return;
        }

        if context.pressed.is_empty() {
            return;
        }

        eprintln!(
            "[saynow] clearing stale non-modifier hotkey state; pressed={:?}",
            context.pressed
        );
        context.pressed.clear();
    }

    fn should_replay_captured_prefix(context: &HookContext, key: HotkeyKey, pressed: bool) -> bool {
        pressed
            && context.hotkey.capture_events
            && !context.state.recording()
            && !context.captured_order.is_empty()
            && !context.hotkey.required_keys.contains(&key)
    }

    fn replay_captured_key_downs(context: &mut HookContext) {
        let captured = context.captured_order.clone();
        context.captured_keys.clear();
        context.captured_order.clear();

        for key in captured {
            if let Err(error) = send_hotkey_key_down(key) {
                eprintln!("[saynow] failed to replay captured hotkey prefix: {error}");
                break;
            }
        }
    }

    fn track_captured_event(context: &mut HookContext, key: HotkeyKey, pressed: bool) {
        if pressed {
            if context.captured_keys.insert(key) {
                context.captured_order.push(key);
            }
        } else {
            context.captured_keys.remove(&key);
            context.captured_order.retain(|captured| *captured != key);
        }
    }

    fn should_capture_event(context: &HookContext, key: HotkeyKey, pressed: bool) -> bool {
        if !context.hotkey.capture_events || !context.hotkey.required_keys.contains(&key) {
            return false;
        }

        if !pressed {
            return context.captured_keys.contains(&key);
        }

        key == HotkeyKey::Modifier(ModifierKey::Alt)
            || context
                .pressed
                .contains(&HotkeyKey::Modifier(ModifierKey::Alt))
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

    fn emit_hotkey_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: &str) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit(
                "hotkey-state",
                HotkeyStateEvent {
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
                if context.state.recording() {
                    release_context_hotkey(context, reason);
                }
                cancel_hotkey_candidate(context);
                if reason == "monitor stop requested" || reason == "hook cleanup" {
                    context.pressed.clear();
                    context.captured_keys.clear();
                    context.captured_order.clear();
                }
            }
        }
    }

    fn release_context_hotkey(context: &mut HookContext, reason: &str) {
        if !context.state.recording() {
            return;
        }
        context.state = HotkeyState::Idle;
        context.release_miss_count = 0;
        eprintln!("[saynow] native hotkey released; reason={reason}");
        (context.emit_state)("Released");
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
            open_clipboard_with_retry()?;
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
            open_clipboard_with_retry()?;
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
            open_clipboard_with_retry()?;
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

    fn open_clipboard_with_retry() -> Result<(), String> {
        let mut last_error = None;
        for attempt in 0..=CLIPBOARD_OPEN_RETRIES {
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < CLIPBOARD_OPEN_RETRIES {
                        thread::sleep(CLIPBOARD_OPEN_RETRY_DELAY);
                    }
                }
            }
        }

        Err(format!(
            "无法打开剪贴板：{}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "未知错误".to_string())
        ))
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
        send_key_down(VK_CONTROL)?;
        thread::sleep(KEY_CHORD_DELAY);
        send_key_down(VK_V)?;
        thread::sleep(KEY_CHORD_DELAY);
        send_key_up(VK_V)?;
        thread::sleep(KEY_CHORD_DELAY);
        send_key_up(VK_CONTROL)
    }

    fn focused_child_window(hwnd: HWND) -> Option<HWND> {
        let target_thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
        if target_thread == 0 {
            return None;
        }

        let mut gui = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetGUIThreadInfo(target_thread, &mut gui).is_ok() }
            && !gui.hwndFocus.0.is_null()
            && unsafe { IsWindow(Some(gui.hwndFocus)).as_bool() }
        {
            Some(gui.hwndFocus)
        } else {
            None
        }
    }

    fn restore_input_target_internal() -> Result<bool, String> {
        let Some(target_lock) = INPUT_TARGET.get() else {
            return Ok(true);
        };
        let Ok(current) = target_lock.lock() else {
            return Err("无法锁定输入目标窗口状态。".to_string());
        };
        let Some(target) = *current else {
            return Ok(true);
        };
        let hwnd = HWND(target.hwnd as *mut core::ffi::c_void);
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            eprintln!(
                "[saynow] skipped restoring missing input target; hwnd={:?}",
                target.hwnd
            );
            return Ok(false);
        }
        let restored = restore_foreground_window_with_retry(hwnd, target.focus_hwnd);
        if !restored {
            eprintln!("[saynow] failed to restore input target; target={target:?}");
        }
        Ok(restored)
    }

    fn restore_foreground_window_with_retry(hwnd: HWND, focus_hwnd: Option<isize>) -> bool {
        for attempt in 1..=FOCUS_RESTORE_RETRIES {
            if restore_foreground_window(hwnd, focus_hwnd) {
                thread::sleep(FOCUS_RESTORE_RETRY_DELAY);
                if unsafe { GetForegroundWindow() } == hwnd {
                    return true;
                }
            }

            eprintln!("[saynow] input target focus restore attempt {attempt} failed");
            thread::sleep(FOCUS_RESTORE_RETRY_DELAY);
        }

        false
    }

    fn restore_foreground_window(hwnd: HWND, focus_hwnd: Option<isize>) -> bool {
        unsafe {
            let current_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let attached = target_thread != 0
                && target_thread != current_thread
                && AttachThreadInput(current_thread, target_thread, true).as_bool();
            let _ = BringWindowToTop(hwnd);
            let restored = SetForegroundWindow(hwnd).as_bool();
            if let Some(focus_hwnd_value) = focus_hwnd {
                let focus = HWND(focus_hwnd_value as *mut core::ffi::c_void);
                if IsWindow(Some(focus)).as_bool() {
                    let _ = SetFocus(Some(focus));
                }
            }
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
                    dwExtraInfo: SAYNOW_INPUT_MARKER,
                },
            },
        }
    }

    fn send_key_down(key: VIRTUAL_KEY) -> Result<(), String> {
        send_keyboard_input(keyboard_input(key, false), "按下")
    }

    fn send_hotkey_key_down(key: HotkeyKey) -> Result<(), String> {
        let virtual_key = match key {
            HotkeyKey::Modifier(ModifierKey::Ctrl) => VK_CONTROL,
            HotkeyKey::Modifier(ModifierKey::Alt) => VK_MENU,
            HotkeyKey::Modifier(ModifierKey::Shift) => VK_SHIFT,
            HotkeyKey::Modifier(ModifierKey::Meta) => VK_LWIN,
            HotkeyKey::Virtual(vk_code) => VIRTUAL_KEY(vk_code as u16),
        };
        send_key_down(virtual_key)
    }

    fn send_key_up(key: VIRTUAL_KEY) -> Result<(), String> {
        send_keyboard_input(keyboard_input(key, true), "释放")
    }

    fn send_keyboard_input(input: INPUT, action: &str) -> Result<(), String> {
        let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err(format!(
                "已写入剪贴板，但模拟 Ctrl+V 失败（{action}按键失败）。"
            ))
        }
    }

    fn release_all_modifiers() {
        for key in [
            VK_CONTROL,
            VK_LCONTROL,
            VK_RCONTROL,
            VK_SHIFT,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_MENU,
            VK_LMENU,
            VK_RMENU,
            VK_LWIN,
            VK_RWIN,
        ] {
            if virtual_key_is_down(key.0 as u32) {
                let _ = send_key_up(key);
            }
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

    #[cfg(test)]
    mod tests {
        use super::*;

        fn hotkey(parts: &[&str]) -> HotkeySpec {
            parse_hotkey_parts(
                &parts
                    .iter()
                    .map(|part| part.to_string())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        }

        fn context(hotkey: HotkeySpec) -> HookContext {
            HookContext {
                hotkey,
                pressed: HashSet::new(),
                captured_keys: HashSet::new(),
                captured_order: Vec::new(),
                state: HotkeyState::Idle,
                release_miss_count: 0,
                emit_state: Box::new(|_| {}),
                should_stop: AtomicBool::new(false),
            }
        }

        fn ctrl() -> HotkeyKey {
            HotkeyKey::Modifier(ModifierKey::Ctrl)
        }

        fn alt() -> HotkeyKey {
            HotkeyKey::Modifier(ModifierKey::Alt)
        }

        fn space() -> HotkeyKey {
            HotkeyKey::Virtual(VK_SPACE.0 as u32)
        }

        fn key_a() -> HotkeyKey {
            HotkeyKey::Virtual('A' as u32)
        }

        fn key_b() -> HotkeyKey {
            HotkeyKey::Virtual('B' as u32)
        }

        #[test]
        fn standalone_modifier_enters_candidate_and_short_release_cancels() {
            let mut context = context(hotkey(&["Alt"]));

            let down = handle_hotkey_key_pressed(&mut context, alt());
            assert!(down.is_none());
            assert!(matches!(context.state, HotkeyState::Candidate { .. }));

            let up = handle_hotkey_key_released(&mut context, alt());
            assert!(up.is_none());
            assert_eq!(context.state, HotkeyState::Idle);
        }

        #[test]
        fn interrupted_standalone_candidate_passes_through_and_cancels() {
            let mut context = context(hotkey(&["Alt"]));
            let _ = handle_hotkey_key_pressed(&mut context, alt());

            let extra_down = handle_hotkey_key_pressed(&mut context, key_a());
            let alt_up = handle_hotkey_key_released(&mut context, alt());

            assert!(extra_down.is_none());
            assert_eq!(context.state, HotkeyState::Idle);
            assert!(alt_up.is_none());
        }

        #[test]
        fn combination_enters_candidate_only_after_full_chord() {
            let mut context = context(hotkey(&["Ctrl", "Space"]));

            let ctrl_down = handle_hotkey_key_pressed(&mut context, ctrl());
            assert!(ctrl_down.is_none());
            assert_eq!(context.state, HotkeyState::Idle);

            let space_down = handle_hotkey_key_pressed(&mut context, space());
            assert!(space_down.is_none());
            assert!(matches!(context.state, HotkeyState::Candidate { .. }));
        }

        #[test]
        fn extra_key_cancels_candidate_without_starting_recording() {
            let mut context = context(hotkey(&["Ctrl", "Space"]));
            let _ = handle_hotkey_key_pressed(&mut context, ctrl());
            let _ = handle_hotkey_key_pressed(&mut context, space());

            let extra_down = handle_hotkey_key_pressed(&mut context, key_a());
            let space_up = handle_hotkey_key_released(&mut context, space());

            assert!(extra_down.is_none());
            assert!(space_up.is_none());
            assert_eq!(context.state, HotkeyState::Idle);
        }

        #[test]
        fn preheld_extra_key_prevents_candidate() {
            let mut context = context(hotkey(&["Ctrl", "Space"]));

            let _ = handle_hotkey_key_pressed(&mut context, key_a());
            let _ = handle_hotkey_key_pressed(&mut context, ctrl());
            let space_down = handle_hotkey_key_pressed(&mut context, space());

            assert!(space_down.is_none());
            assert_eq!(context.state, HotkeyState::Idle);
        }

        #[test]
        fn candidate_keeps_exact_pressed_hotkey_state() {
            let mut context = context(hotkey(&["Alt"]));
            let _ = handle_hotkey_key_pressed(&mut context, alt());

            assert!(hotkey_exactly_pressed(&context.hotkey, &context.pressed));
        }

        #[test]
        fn alt_hotkeys_use_captured_software_state_for_candidate_confirmation() {
            let mut context = context(hotkey(&["Alt"]));
            let _ = handle_hotkey_key_pressed(&mut context, alt());

            assert!(context.hotkey.capture_events);
            assert!(tracked_hotkey_is_down(&context));
        }

        #[test]
        fn non_alt_hotkeys_do_not_use_capture_mode() {
            let context = context(hotkey(&["Ctrl", "Space"]));

            assert!(!context.hotkey.capture_events);
        }

        #[test]
        fn alt_capture_starts_with_alt_not_other_modifiers() {
            let mut context = context(hotkey(&["Ctrl", "Alt"]));

            assert!(!should_capture_event(&context, ctrl(), true));

            let _ = handle_hotkey_key_pressed(&mut context, ctrl());
            assert!(!should_capture_event(&context, ctrl(), true));
            assert!(should_capture_event(&context, alt(), true));

            let _ = handle_hotkey_key_pressed(&mut context, alt());
            assert!(should_capture_event(&context, ctrl(), true));
        }

        #[test]
        fn alt_only_hotkey_replays_tentative_alt_when_combo_key_follows() {
            let mut context = context(hotkey(&["Alt"]));

            assert!(should_capture_event(&context, alt(), true));
            track_captured_event(&mut context, alt(), true);
            let _ = handle_hotkey_key_pressed(&mut context, alt());

            assert!(should_replay_captured_prefix(&context, key_a(), true));
        }

        #[test]
        fn alt_combo_hotkey_captures_only_configured_combo_keys() {
            let mut context = context(hotkey(&["Alt", "Space"]));

            assert!(should_capture_event(&context, alt(), true));
            track_captured_event(&mut context, alt(), true);
            let _ = handle_hotkey_key_pressed(&mut context, alt());

            assert!(!should_replay_captured_prefix(&context, space(), true));
            assert!(should_capture_event(&context, space(), true));
            assert!(should_replay_captured_prefix(&context, key_b(), true));
        }

        #[test]
        fn recording_releases_on_required_key_up() {
            let mut context = context(hotkey(&["Ctrl", "Space"]));
            let _ = handle_hotkey_key_pressed(&mut context, ctrl());
            let _ = handle_hotkey_key_pressed(&mut context, space());
            context.state = HotkeyState::Recording;

            let space_up = handle_hotkey_key_released(&mut context, space());

            assert_eq!(space_up, Some("released"));
        }

        #[test]
        fn recording_releases_once_even_when_another_required_key_follows() {
            let mut context = context(hotkey(&["Ctrl", "Space"]));
            let _ = handle_hotkey_key_pressed(&mut context, ctrl());
            let _ = handle_hotkey_key_pressed(&mut context, space());
            context.state = HotkeyState::Recording;

            let ctrl_up = handle_hotkey_key_released(&mut context, ctrl());
            if let Some(reason) = ctrl_up {
                release_context_hotkey(&mut context, reason);
            }
            let space_up = handle_hotkey_key_released(&mut context, space());

            assert_eq!(ctrl_up, Some("released"));
            assert!(space_up.is_none());
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

    pub fn restore_input_target() -> Result<(), String> {
        Ok(())
    }
}
