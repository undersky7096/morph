//! Déclencheur global Shift + glisser (Windows).
//!
//! Un hook souris bas niveau surveille les glissers partant de l'explorateur ou du bureau.
//! Dès que Shift est enfoncé pendant un tel glisser, une zone de dépôt native, invisible et
//! couvrant tous les écrans, apparaît au premier plan (sans prendre le focus). Déposer des
//! fichiers dessus ouvre le menu radial au point de dépôt ; un halo suit le curseur pendant
//! le survol.
//!
//! La zone de dépôt implémente son propre `IDropTarget` pour toujours répondre
//! DROPEFFECT_COPY : avec Shift, l'explorateur propose un *déplacement*, et un effet
//! « move » renvoyé lui ferait supprimer les fichiers d'origine.

use std::cell::RefCell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use windows::Win32::Foundation::{
    CloseHandle, COLORREF, HWND, LPARAM, LRESULT, POINT, POINTL, WPARAM,
};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
    OleInitialize, RegisterDragDrop, ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GA_ROOT, GetAncestor,
    GetMessageW, GetSystemMetrics, GetWindowThreadProcessId, HC_ACTION, HWND_TOPMOST, IDC_ARROW,
    KillTimer, LWA_ALPHA, LoadCursorW, MSG, MSLLHOOKSTRUCT, RegisterClassW, SM_CXDRAG,
    SM_CXVIRTUALSCREEN, SM_CYDRAG, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetTimer, SetWindowPos,
    SetWindowsHookExW, ShowWindow, TranslateMessage, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows_core::{PWSTR, Ref, implement, w};

use crate::radial;

const HALO_LABEL: &str = "halo";
const HIDE_TIMER: usize = 1;
/// Délai laissé au dépôt OLE après le relâchement du bouton, avant de masquer la zone.
const HIDE_DELAY_MS: u32 = 500;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Traces de diagnostic, uniquement en build de debug.
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!("[shift-drag] {}", format!($($arg)*));
    };
}

#[derive(Default)]
struct State {
    overlay: HWND,
    halo_size: PhysicalSize<u32>,
    /// Point d'appui du bouton gauche, tant qu'il est enfoncé.
    press: Option<POINT>,
    /// Le glisser en cours part-il de l'explorateur ? (calculé une fois par glisser)
    from_explorer: Option<bool>,
    visible: bool,
    files_over: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn start(app: AppHandle) {
    let halo_size = app
        .get_webview_window(HALO_LABEL)
        .and_then(|halo| {
            // Le halo ne doit jamais intercepter la souris ni le dépôt.
            let _ = halo.set_ignore_cursor_events(true);
            halo.outer_size().ok()
        })
        .unwrap_or(PhysicalSize::new(140, 140));
    let _ = APP.set(app);

    std::thread::Builder::new()
        .name("morph-shift-drag".into())
        .spawn(move || {
            if let Err(e) = unsafe { run(halo_size) } {
                eprintln!("Morph : déclencheur Shift+glisser indisponible : {e}");
            }
        })
        .expect("impossible de démarrer le thread Shift+glisser");
}

unsafe fn run(halo_size: PhysicalSize<u32>) -> windows_core::Result<()> {
    unsafe {
        OleInitialize(None)?;
        let overlay = create_overlay()?;
        let target: IDropTarget = DropTarget.into();
        RegisterDragDrop(overlay, &target)?;
        STATE.with_borrow_mut(|s| {
            s.overlay = overlay;
            s.halo_size = halo_size;
        });

        let _hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)?;
        trace!("hook souris installé");
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}

/// Fenêtre popup quasi transparente (alpha 1/255 : invisible mais cible de dépôt valide).
unsafe fn create_overlay() -> windows_core::Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("MorphDropOverlay");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(overlay_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class,
            w!("Morph"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 1, LWA_ALPHA)?;
        Ok(hwnd)
    }
}

unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_TIMER && wparam.0 == HIDE_TIMER {
        hide_overlay();
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        on_mouse(wparam.0 as u32, info.pt);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn on_mouse(msg: u32, pt: POINT) {
    match msg {
        WM_LBUTTONDOWN => STATE.with_borrow_mut(|s| {
            s.press = Some(pt);
            s.from_explorer = None;
        }),
        WM_LBUTTONUP => {
            let (visible, files_over, overlay) = STATE.with_borrow_mut(|s| {
                s.press = None;
                (s.visible, s.files_over, s.overlay)
            });
            if visible {
                if files_over {
                    // Le dépôt OLE arrive juste après : on laisse la zone en place un instant.
                    unsafe { SetTimer(Some(overlay), HIDE_TIMER, HIDE_DELAY_MS, None) };
                } else {
                    hide_overlay();
                }
            }
        }
        WM_MOUSEMOVE => {
            let (press, visible, from_explorer) =
                STATE.with_borrow(|s| (s.press, s.visible, s.from_explorer));
            let Some(start) = press else { return };
            if visible || from_explorer == Some(false) {
                return;
            }
            let (dx, dy) = unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
            if (pt.x - start.x).abs() <= dx && (pt.y - start.y).abs() <= dy {
                return;
            }
            // Bit de poids fort = touche actuellement enfoncée.
            if unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } >= 0 {
                return;
            }
            let from_explorer = from_explorer.unwrap_or_else(|| {
                let v = unsafe { is_explorer_at(start) };
                trace!("glisser + Shift depuis ({}, {}) : explorateur = {v}", start.x, start.y);
                STATE.with_borrow_mut(|s| s.from_explorer = Some(v));
                v
            });
            if from_explorer {
                show_overlay();
            }
        }
        _ => {}
    }
}

/// La fenêtre sous `pt` appartient-elle à l'explorateur Windows (dossiers ou bureau) ?
unsafe fn is_explorer_at(pt: POINT) -> bool {
    unsafe {
        let hwnd = WindowFromPoint(pt);
        if hwnd.is_invalid() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(GetAncestor(hwnd, GA_ROOT), Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            trace!("OpenProcess({pid}) a échoué");
            return false;
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
            .is_ok();
        let _ = CloseHandle(process);
        trace!("fenêtre sous le point : pid {pid}, ok = {ok}, {}", String::from_utf16_lossy(&buf[..len as usize]));
        ok && String::from_utf16_lossy(&buf[..len as usize])
            .to_ascii_lowercase()
            .ends_with("\\explorer.exe")
    }
}

fn show_overlay() {
    trace!("zone de dépôt affichée");
    let overlay = STATE.with_borrow_mut(|s| {
        s.visible = true;
        s.files_over = false;
        s.overlay
    });
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let _ = SetWindowPos(overlay, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

fn hide_overlay() {
    let overlay = STATE.with_borrow_mut(|s| {
        s.visible = false;
        s.files_over = false;
        s.overlay
    });
    unsafe {
        let _ = KillTimer(Some(overlay), HIDE_TIMER);
        let _ = ShowWindow(overlay, SW_HIDE);
    }
    hide_halo();
}

fn halo_window() -> Option<tauri::WebviewWindow> {
    APP.get()?.get_webview_window(HALO_LABEL)
}

fn move_halo(pt: &POINTL, appear: bool) {
    let Some(halo) = halo_window() else { return };
    let size = STATE.with_borrow(|s| s.halo_size);
    let _ = halo.set_position(PhysicalPosition::new(
        pt.x - size.width as i32 / 2,
        pt.y - size.height as i32 / 2,
    ));
    if appear {
        let _ = halo.show();
        let _ = halo.emit_to(HALO_LABEL, "halo-show", ());
    }
}

fn hide_halo() {
    if let Some(halo) = halo_window() {
        let _ = halo.hide();
        // Une fenêtre masquée continue d'être dessinée par le webview : le halo vide son
        // contenu (et son animation en boucle) pour ne pas occuper la carte graphique au repos.
        let _ = halo.emit_to(HALO_LABEL, "halo-hide", ());
    }
}

/// Chemins des fichiers transportés par un glisser (format CF_HDROP).
unsafe fn read_paths(data: &IDataObject) -> Vec<String> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    let mut paths = Vec::new();
    unsafe {
        let Ok(mut medium) = data.GetData(&format) else {
            return paths;
        };
        let hdrop = HDROP(medium.u.hGlobal.0 as _);
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            paths.push(OsString::from_wide(&buf[..len]).to_string_lossy().into_owned());
        }
        ReleaseStgMedium(&mut medium);
    }
    paths
}

#[implement(IDropTarget)]
struct DropTarget;

#[allow(non_snake_case)]
impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let has_files = data.as_ref().is_some_and(|d| unsafe { !read_paths(d).is_empty() });
        STATE.with_borrow_mut(|s| s.files_over = has_files);
        trace!("survol : fichiers = {has_files}");
        unsafe { *effect = if has_files { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        if has_files {
            move_halo(pt, true);
        }
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> windows_core::Result<()> {
        let files_over = STATE.with_borrow(|s| s.files_over);
        unsafe { *effect = if files_over { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        if files_over {
            move_halo(pt, false);
        }
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        STATE.with_borrow_mut(|s| s.files_over = false);
        hide_halo();
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let paths = data.as_ref().map(|d| unsafe { read_paths(d) }).unwrap_or_default();
        trace!("dépôt : {paths:?}");
        // Toujours une copie, jamais un déplacement : l'original reste intact.
        unsafe { *effect = if paths.is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
        hide_overlay();
        if let (false, Some(app)) = (paths.is_empty(), APP.get().cloned()) {
            // Hors du thread OLE : l'explorateur attend le retour de Drop.
            std::thread::spawn(move || {
                let _ = radial::show(&app, paths);
            });
        }
        Ok(())
    }
}
