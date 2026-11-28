//! Windows UI: 트레이 아이콘, Ctrl+C,C 감지(저수준 키보드 훅), 번역 팝업.
//! 모든 창과 훅은 메인 스레드 하나에서 돌고, 번역만 작업 스레드에서 한다.
#![allow(static_mut_refs)]

use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering::SeqCst};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::core;

const WM_TRAY: u32 = WM_APP + 1;
const WM_TRIGGER: u32 = WM_APP + 2; // Ctrl+C 가 두 번 눌림
const WM_DELTA: u32 = WM_APP + 3; // 번역 조각 도착 (wParam: 세대, lParam: Box<String>)
const WM_DONE: u32 = WM_APP + 4; // 번역 끝 (wParam: 세대, lParam: Box<Result<String, String>>)
const WM_OUTSIDE_CLICK: u32 = WM_APP + 5;

const TIMER_READ_CLIPBOARD: usize = 1;
const ID_COPY: usize = 100;
const CMD_PAUSE: usize = 1;
const CMD_AUTOSTART: usize = 2;
const CMD_KEYS: usize = 3;
const CMD_QUIT: usize = 4;
const CMD_GUIDE: usize = 5;

const DOUBLE_PRESS_MS: u64 = 600;
const CF_UNICODETEXT: u32 = 13;
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

// 96dpi 기준 치수
const WIDTH: i32 = 480;
const HEADER: i32 = 38;
const FOOTER: i32 = 46;
const MARGIN: i32 = 12; // 팝업 가장자리 ↔ 흰 카드
const PAD: i32 = 10; // 흰 카드 ↔ 글

static MAIN: AtomicIsize = AtomicIsize::new(0);
static POPUP: AtomicIsize = AtomicIsize::new(0);
static GENERATION: AtomicUsize = AtomicUsize::new(0);
static PAUSED: AtomicBool = AtomicBool::new(false);
static C_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_COPY: AtomicU64 = AtomicU64::new(0);

#[derive(PartialEq, Clone, Copy)]
enum Status {
    Loading,
    Text,
    Error,
}

struct Ui {
    edit: HWND,
    button: HWND,
    font: HFONT,
    bold: HFONT,
    small: HFONT,
    white: HBRUSH,
    mouse_hook: HHOOK,
    taskbar_created: u32,
    scale: f32,
    work: RECT,
    edit_height: i32,
    translation: String,
    label: String,
    model: String,
    status: Status,
}

/// 메인 스레드에서만 건드리는 UI 상태.
static mut UI: Ui = Ui {
    edit: null_mut(),
    button: null_mut(),
    font: null_mut(),
    bold: null_mut(),
    small: null_mut(),
    white: null_mut(),
    mouse_hook: null_mut(),
    taskbar_created: 0,
    scale: 0.0,
    work: RECT { left: 0, top: 0, right: 0, bottom: 0 },
    edit_height: 0,
    translation: String::new(),
    label: String::new(),
    model: String::new(),
    status: Status::Loading,
};

fn ui() -> &'static mut Ui {
    unsafe { &mut *std::ptr::addr_of_mut!(UI) }
}

fn main_hwnd() -> HWND {
    MAIN.load(SeqCst) as HWND
}

fn popup_hwnd() -> HWND {
    POPUP.load(SeqCst) as HWND
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn px(n: i32) -> i32 {
    (n as f32 * ui().scale).round() as i32
}

const fn rgb(r: u32, g: u32, b: u32) -> COLORREF {
    r | (g << 8) | (b << 16)
}

pub fn run() {
    unsafe {
        // 이미 실행 중이면 조용히 끝낸다.
        CreateMutexW(null(), 0, wide("Local\\DeepZSingleInstance").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let instance = GetModuleHandleW(null());
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES,
        });

        let main_class = wide("DeepZMain");
        let popup_class = wide("DeepZPopup");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.hInstance = instance;
        class.lpfnWndProc = Some(main_proc);
        class.lpszClassName = main_class.as_ptr();
        RegisterClassW(&class);
        class.lpfnWndProc = Some(popup_proc);
        class.lpszClassName = popup_class.as_ptr();
        class.style = CS_DROPSHADOW;
        class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        RegisterClassW(&class);

        let title = wide("DeepZ");
        let main = CreateWindowExW(
            0, main_class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), instance, null(),
        );
        MAIN.store(main as isize, SeqCst);
        let popup = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW, popup_class.as_ptr(), title.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN, 0, 0, WIDTH, 160, null_mut(), null_mut(), instance, null(),
        );
        POPUP.store(popup as isize, SeqCst);
        // Windows 11 의 둥근 모서리 (DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2). 10 에서는 무시된다.
        let round: u32 = 2;
        DwmSetWindowAttribute(popup, 33, &round as *const u32 as *const _, 4);

        let state = ui();
        state.white = CreateSolidBrush(rgb(255, 255, 255));
        state.edit = CreateWindowExW(
            0, wide("EDIT").as_ptr(), null(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32,
            0, 0, 0, 0, popup, null_mut(), instance, null(),
        );
        state.button = CreateWindowExW(
            0, wide("BUTTON").as_ptr(), wide("복사").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            0, 0, 0, 0, popup, ID_COPY as HMENU, instance, null(),
        );
        set_scale(1.0);

        state.taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        tray(NIM_ADD);
        let keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), instance, 0);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            // 팝업에 포커스가 있을 때 Esc 로 닫기
            if msg.message == WM_KEYDOWN
                && msg.wParam == VK_ESCAPE as usize
                && (msg.hwnd == popup || IsChild(popup, msg.hwnd) != 0)
            {
                hide_popup();
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(keyboard_hook);
        tray(NIM_DELETE);
    }
}

// MARK: Ctrl+C,C 감지

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let key = &*(lparam as *const KBDLLHOOKSTRUCT);
        if key.vkCode == 0x43 {
            // 'C'
            match wparam as u32 {
                // 키를 누르고 있어서 생기는 반복 입력은 한 번으로 친다.
                WM_KEYDOWN if !C_DOWN.swap(true, SeqCst) => {
                    let down = |vk: VIRTUAL_KEY| GetAsyncKeyState(vk as i32) < 0;
                    if down(VK_CONTROL) && !down(VK_SHIFT) && !down(VK_MENU) && !down(VK_LWIN) && !down(VK_RWIN) {
                        let now = GetTickCount64();
                        if now.wrapping_sub(LAST_COPY.load(SeqCst)) <= DOUBLE_PRESS_MS {
                            LAST_COPY.store(0, SeqCst);
                            PostMessageW(main_hwnd(), WM_TRIGGER, 0, 0);
                        } else {
                            LAST_COPY.store(now, SeqCst);
                        }
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => C_DOWN.store(false, SeqCst),
                _ => {}
            }
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

/// 팝업이 떠 있는 동안만 설치된다. 팝업 바깥을 누르면 닫는다.
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && matches!(wparam as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        let mouse = &*(lparam as *const MSLLHOOKSTRUCT);
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(popup_hwnd(), &mut rect);
        if PtInRect(&rect, mouse.pt) == 0 {
            PostMessageW(main_hwnd(), WM_OUTSIDE_CLICK, 0, 0);
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

// MARK: 클립보드

unsafe fn read_clipboard() -> Option<String> {
    // 비밀번호 관리자가 기록 제외로 표시한 항목은 건드리지 않는다.
    let excluded = RegisterClipboardFormatW(wide("ExcludeClipboardContentFromMonitorProcessing").as_ptr());
    if IsClipboardFormatAvailable(excluded) != 0 || IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
        return None;
    }
    if !open_clipboard() {
        return None;
    }
    let mut text = None;
    let handle = GetClipboardData(CF_UNICODETEXT);
    if !handle.is_null() {
        let data = GlobalLock(handle) as *const u16;
        if !data.is_null() {
            let max = GlobalSize(handle) / 2;
            let mut len = 0;
            while len < max && *data.add(len) != 0 {
                len += 1;
            }
            text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(data, len)));
            GlobalUnlock(handle);
        }
    }
    CloseClipboard();
    text
}

unsafe fn write_clipboard(text: &str) {
    let data = wide(text);
    let bytes = data.len() * 2;
    let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
    if handle.is_null() {
        return;
    }
    let target = GlobalLock(handle) as *mut u16;
    std::ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
    GlobalUnlock(handle);
    if open_clipboard() {
        EmptyClipboard();
        if SetClipboardData(CF_UNICODETEXT, handle).is_null() {
            GlobalFree(handle);
        }
        CloseClipboard();
    } else {
        GlobalFree(handle);
    }
}

/// 다른 프로그램이 클립보드를 잡고 있을 수 있어서 잠깐씩 다시 시도한다.
unsafe fn open_clipboard() -> bool {
    for _ in 0..8 {
        if OpenClipboard(main_hwnd()) != 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

// MARK: 번역 팝업

unsafe fn set_scale(scale: f32) {
    let state = ui();
    if state.scale == scale {
        return;
    }
    state.scale = scale;
    for font in [state.font, state.bold, state.small] {
        if !font.is_null() {
            DeleteObject(font);
        }
    }
    let face = wide("Malgun Gothic");
    let make = |size: i32, weight: i32| {
        CreateFontW(
            -px(size), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as _, 0, 0, CLEARTYPE_QUALITY as _, 0,
            face.as_ptr(),
        )
    };
    state.font = make(15, FW_NORMAL as i32);
    state.bold = make(13, FW_SEMIBOLD as i32);
    state.small = make(12, FW_NORMAL as i32);
    SendMessageW(state.edit, WM_SETFONT, state.font as WPARAM, 1);
    SendMessageW(state.edit, EM_SETMARGINS, (EC_LEFTMARGIN | EC_RIGHTMARGIN) as WPARAM, 0);
    SendMessageW(state.button, WM_SETFONT, state.small as WPARAM, 1);
}

/// 마우스 커서 근처에 팝업을 띄우고 번역을 시작한다.
unsafe fn start_translation(text: String) {
    let popup = popup_hwnd();
    let target = core::target_for(&text);
    let generation = GENERATION.fetch_add(1, SeqCst) + 1;

    // 커서가 있는 모니터의 배율과 작업 영역
    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    set_scale(dpi_x.max(96) as f32 / 96.0);

    let state = ui();
    state.work = info.rcWork;
    state.translation.clear();
    state.model.clear();
    state.label = format!("→ {}", target.label);
    state.status = Status::Loading;
    SetWindowTextW(state.button, wide("복사").as_ptr());
    EnableWindow(state.button, 0);
    set_edit_text("번역 중…");

    let left = (cursor.x + px(14)).min(info.rcWork.right - px(WIDTH) - 8).max(info.rcWork.left + 8);
    layout(Some((left, cursor.y + px(18))));
    ShowWindow(popup, SW_SHOWNOACTIVATE);
    if state.mouse_hook.is_null() {
        state.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), GetModuleHandleW(null()), 0);
    }

    let length = text.chars().count();
    if length > core::MAX_CHARS {
        show_error(&format!(
            "선택한 글이 너무 깁니다 ({length}자). {}자까지 번역할 수 있습니다.",
            core::MAX_CHARS
        ));
        return;
    }
    std::thread::spawn(move || {
        let post = |message: u32, payload: isize| unsafe {
            PostMessageW(MAIN.load(SeqCst) as HWND, message, generation, payload);
        };
        let result = core::translate(&text, &target, &core::load_env(), &mut |delta| {
            if GENERATION.load(SeqCst) != generation {
                return false; // 팝업이 닫혔거나 새 번역이 시작됨
            }
            post(WM_DELTA, Box::into_raw(Box::new(delta.to_string())) as isize);
            true
        });
        post(WM_DONE, Box::into_raw(Box::new(result)) as isize);
    });
}

unsafe fn set_edit_text(text: &str) {
    // EDIT 컨트롤은 줄바꿈이 \r\n 이어야 한다.
    let normalized = text.replace("\r\n", "\n").replace('\n', "\r\n");
    SetWindowTextW(ui().edit, wide(&normalized).as_ptr());
}

unsafe fn show_error(message: &str) {
    let state = ui();
    state.status = Status::Error;
    let text = if state.translation.is_empty() {
        message.to_string()
    } else {
        format!("{}\n\n⚠ {message}", state.translation)
    };
    set_edit_text(&text);
    layout(None);
}

unsafe fn hide_popup() {
    let state = ui();
    GENERATION.fetch_add(1, SeqCst); // 진행 중인 번역을 취소한다.
    ShowWindow(popup_hwnd(), SW_HIDE);
    if !state.mouse_hook.is_null() {
        UnhookWindowsHookEx(state.mouse_hook);
        state.mouse_hook = null_mut();
    }
}

/// 글 길이에 맞춰 팝업 높이를 정한다. `position` 이 없으면 지금 위치를 유지한다.
unsafe fn layout(position: Option<(i32, i32)>) {
    let state = ui();
    let popup = popup_hwnd();
    let width = px(WIDTH);
    let edit_width = width - 2 * px(MARGIN) - 2 * px(PAD);

    // 글이 차지할 높이를 잰다.
    let length = GetWindowTextLengthW(state.edit);
    let mut buffer = vec![0u16; length as usize + 1];
    GetWindowTextW(state.edit, buffer.as_mut_ptr(), length + 1);
    let dc = GetDC(state.edit);
    let old = SelectObject(dc, state.font);
    let mut measured = RECT { left: 0, top: 0, right: edit_width, bottom: 0 };
    DrawTextW(
        dc, buffer.as_ptr(), -1, &mut measured,
        DT_CALCRECT | DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX | DT_EXPANDTABS,
    );
    SelectObject(dc, old);
    ReleaseDC(state.edit, dc);

    let chrome = px(HEADER) + 2 * px(PAD) + px(FOOTER);
    let wanted = measured.bottom.max(px(22)) + px(4);
    let limit = px(420).min(state.work.bottom - state.work.top - chrome - 16).max(px(60));
    state.edit_height = wanted.min(limit);
    let height = chrome + state.edit_height;

    let (left, mut top) = position.unwrap_or_else(|| {
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(popup, &mut rect);
        (rect.left, rect.top)
    });
    top = top.min(state.work.bottom - 8 - height).max(state.work.top + 8);

    SetWindowPos(popup, HWND_TOPMOST, left, top, width, height, SWP_NOACTIVATE);
    MoveWindow(state.edit, px(MARGIN) + px(PAD), px(HEADER) + px(PAD), edit_width, state.edit_height, 1);
    MoveWindow(state.button, px(MARGIN), height - px(FOOTER) + px(9), px(84), px(28), 1);
    ShowScrollBar(state.edit, SB_VERT, (wanted > limit) as i32);
    InvalidateRect(popup, null(), 1);
}

unsafe fn paint(hwnd: HWND) {
    let state = ui();
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);

    let background = CreateSolidBrush(rgb(243, 243, 243));
    let border = CreateSolidBrush(rgb(200, 200, 200));
    FillRect(dc, &client, background);
    FrameRect(dc, &client, border);
    DeleteObject(background);
    DeleteObject(border);
    SetBkMode(dc, TRANSPARENT as _);

    // 머리줄: 이름과 번역 방향
    let old_font = SelectObject(dc, state.bold);
    let title = wide("DeepZ");
    let mut title_size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, title.as_ptr(), 5, &mut title_size);
    let mut rect = RECT { left: px(14), top: 0, right: client.right, bottom: px(HEADER) };
    SetTextColor(dc, rgb(32, 32, 32));
    DrawTextW(dc, title.as_ptr(), -1, &mut rect, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    SelectObject(dc, state.small);
    rect.left += title_size.cx + px(8);
    SetTextColor(dc, rgb(110, 110, 110));
    DrawTextW(dc, wide(&state.label).as_ptr(), -1, &mut rect, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);

    // 닫기 (X)
    let pen = CreatePen(PS_SOLID, px(1).max(1), rgb(110, 110, 110));
    let old_pen = SelectObject(dc, pen);
    let (cx, cy, r) = (client.right - px(22), px(HEADER) / 2, px(5));
    MoveToEx(dc, cx - r, cy - r, null_mut());
    LineTo(dc, cx + r + 1, cy + r + 1);
    MoveToEx(dc, cx + r, cy - r, null_mut());
    LineTo(dc, cx - r - 1, cy + r + 1);
    SelectObject(dc, old_pen);
    DeleteObject(pen);

    // 글이 놓이는 흰 카드
    let card_pen = CreatePen(PS_SOLID, 1, rgb(225, 225, 225));
    let old_pen = SelectObject(dc, card_pen);
    let old_brush = SelectObject(dc, state.white);
    RoundRect(
        dc, px(MARGIN), px(HEADER), client.right - px(MARGIN),
        px(HEADER) + state.edit_height + 2 * px(PAD), px(10), px(10),
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(card_pen);

    // 바닥줄: 사용한 모델
    let mut rect = RECT {
        left: px(110), top: client.bottom - px(FOOTER), right: client.right - px(14), bottom: client.bottom,
    };
    SetTextColor(dc, rgb(150, 150, 150));
    DrawTextW(
        dc, wide(&state.model).as_ptr(), -1, &mut rect,
        DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_NOPREFIX | DT_END_ELLIPSIS,
    );
    SelectObject(dc, old_font);
    EndPaint(hwnd, &ps);
}

unsafe fn in_close_button(hwnd: HWND, x: i32, y: i32) -> bool {
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);
    y < px(HEADER) && x >= client.right - px(40)
}

unsafe extern "system" fn popup_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let point = || ((lparam & 0xFFFF) as i16 as i32, ((lparam >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        // 읽기 전용 EDIT 는 이 메시지로 색을 묻는다.
        WM_CTLCOLORSTATIC if lparam as HWND == ui().edit => {
            let color = match ui().status {
                Status::Loading => rgb(130, 130, 130),
                Status::Text => rgb(26, 26, 26),
                Status::Error => rgb(196, 43, 28),
            };
            SetTextColor(wparam as HDC, color);
            SetBkColor(wparam as HDC, rgb(255, 255, 255));
            ui().white as LRESULT
        }
        // 머리줄을 잡고 끌어서 옮길 수 있게 한다.
        WM_NCHITTEST => {
            let (x, y) = point();
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            if pt.y < px(HEADER) && !in_close_button(hwnd, pt.x, pt.y) {
                HTCAPTION as LRESULT
            } else {
                HTCLIENT as LRESULT
            }
        }
        WM_LBUTTONUP => {
            let (x, y) = point();
            if in_close_button(hwnd, x, y) {
                hide_popup();
            }
            0
        }
        WM_COMMAND if (wparam & 0xFFFF) == ID_COPY => {
            let state = ui();
            if !state.translation.is_empty() {
                write_clipboard(state.translation.trim());
                SetWindowTextW(state.button, wide("복사됨").as_ptr());
            }
            0
        }
        WM_CLOSE => {
            hide_popup();
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// MARK: 메인 창 (트레이, 타이머, 작업 스레드 메시지)

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let state = ui();
    match msg {
        WM_TRIGGER => {
            // 팝업 안에서 복사한 것은 번역하지 않는다.
            let foreground = GetForegroundWindow();
            if !PAUSED.load(SeqCst) && foreground != popup_hwnd() && IsChild(popup_hwnd(), foreground) == 0 {
                // 앞쪽 프로그램이 클립보드에 쓰기를 마칠 시간을 준다.
                SetTimer(hwnd, TIMER_READ_CLIPBOARD, 120, None);
            }
            0
        }
        WM_TIMER if wparam == TIMER_READ_CLIPBOARD => {
            KillTimer(hwnd, TIMER_READ_CLIPBOARD);
            if let Some(text) = read_clipboard().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
                start_translation(text);
            }
            0
        }
        WM_DELTA => {
            let delta = Box::from_raw(lparam as *mut String);
            if wparam == GENERATION.load(SeqCst) {
                state.translation.push_str(&delta);
                state.status = Status::Text;
                EnableWindow(state.button, 1);
                set_edit_text(&state.translation);
                layout(None);
            }
            0
        }
        WM_DONE => {
            let result = Box::from_raw(lparam as *mut Result<String, String>);
            if wparam == GENERATION.load(SeqCst) {
                match *result {
                    Ok(model) => {
                        state.model = model;
                        state.translation = state.translation.trim().to_string();
                        set_edit_text(&state.translation);
                        layout(None);
                    }
                    Err(message) if !message.is_empty() => show_error(&message),
                    Err(_) => {}
                }
            }
            0
        }
        WM_OUTSIDE_CLICK => {
            hide_popup();
            0
        }
        WM_TRAY => {
            if matches!(lparam as u32, WM_RBUTTONUP | WM_LBUTTONUP) {
                tray_menu(hwnd);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        // 탐색기가 다시 시작되면 트레이 아이콘을 다시 넣는다.
        _ if msg == state.taskbar_created && msg != 0 => {
            tray(NIM_ADD);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn tray(action: NOTIFY_ICON_MESSAGE) {
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = main_hwnd();
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = LoadImageW(
        GetModuleHandleW(null()), 1 as *const u16, IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), 0,
    );
    if data.hIcon.is_null() {
        data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
    }
    let tip = wide("DeepZ — Ctrl+C 두 번으로 번역");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    Shell_NotifyIconW(action, &data);
}

unsafe fn tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    let add = |flags: u32, id: usize, title: &str| {
        AppendMenuW(menu, flags, id, wide(title).as_ptr());
    };
    let checked = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
    add(MF_STRING | MF_GRAYED, 0, "글을 선택하고 Ctrl+C 를 두 번 누르면 번역합니다");
    if core::free_available() && !core::has_key(&core::load_env()) {
        add(
            MF_STRING | MF_GRAYED,
            0,
            &format!(
                "무료 번역: 오늘 {}회 남음 (하루 {}회) · 본인 API 키를 넣으면 제한 없음",
                core::FREE_DAILY_LIMIT.saturating_sub(core::free_used_today()),
                core::FREE_DAILY_LIMIT
            ),
        );
    }
    add(MF_SEPARATOR, 0, "");
    add(MF_STRING | checked(PAUSED.load(SeqCst)), CMD_PAUSE, "일시 정지");
    add(MF_STRING | checked(autostart_enabled()), CMD_AUTOSTART, "Windows 시작 시 자동 실행");
    add(MF_STRING, CMD_KEYS, "API 키 설정…");
    add(MF_STRING, CMD_GUIDE, "API 키 발급 안내");
    add(MF_SEPARATOR, 0, "");
    add(MF_STRING, CMD_QUIT, "DeepZ 종료");

    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    SetForegroundWindow(hwnd); // 이게 없으면 메뉴 바깥을 눌러도 메뉴가 닫히지 않는다.
    let command = TrackPopupMenu(
        menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, cursor.x, cursor.y, 0, hwnd, null(),
    );
    DestroyMenu(menu);
    match command as usize {
        CMD_PAUSE => {
            PAUSED.fetch_xor(true, SeqCst);
        }
        CMD_AUTOSTART => set_autostart(!autostart_enabled()),
        CMD_KEYS => open_key_file(),
        CMD_GUIDE => {
            ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(core::KEY_GUIDE_URL).as_ptr(), null(), null(), SW_SHOWNORMAL);
        }
        CMD_QUIT => {
            DestroyWindow(hwnd);
        }
        _ => {}
    }
}

// MARK: 자동 실행, 키 파일

unsafe fn autostart_enabled() -> bool {
    let mut key: HKEY = null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
        return false;
    }
    let found = RegQueryValueExW(key, wide("DeepZ").as_ptr(), null(), null_mut(), null_mut(), null_mut());
    RegCloseKey(key);
    found == ERROR_SUCCESS
}

unsafe fn set_autostart(enable: bool) {
    let mut key: HKEY = null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) != ERROR_SUCCESS {
        return;
    }
    let name = wide("DeepZ");
    if enable {
        if let Ok(exe) = std::env::current_exe() {
            let command = wide(&format!("\"{}\"", exe.display()));
            RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, command.as_ptr() as *const u8, (command.len() * 2) as u32);
        }
    } else {
        RegDeleteValueW(key, name.as_ptr());
    }
    RegCloseKey(key);
}

/// 키 파일이 없으면 틀을 만들어 주고 메모장으로 연다.
unsafe fn open_key_file() {
    let Some(path) = core::env_paths().into_iter().next() else { return };
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(
            &path,
            "# DeepZ API 키. 하나만 있어도 횟수 제한 없이 번역합니다. 저장하면 다음 번역부터 적용됩니다.\r\nANTHROPIC_API_KEY=\r\nOPENAI_API_KEY=\r\nGEMINI_API_KEY=\r\n",
        );
    }
    ShellExecuteW(
        null_mut(), wide("open").as_ptr(), wide("notepad.exe").as_ptr(),
        wide(&format!("\"{}\"", path.display())).as_ptr(), null(), SW_SHOWNORMAL,
    );
}
