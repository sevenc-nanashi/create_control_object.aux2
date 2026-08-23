use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            COLOR_BTNFACE, DEFAULT_GUI_FONT, GetStockObject, GetSysColorBrush, UpdateWindow,
        },
        UI::{
            Controls::{BST_CHECKED, WC_BUTTON, WC_STATIC},
            Input::KeyboardAndMouse::{EnableWindow, VK_ESCAPE},
            WindowsAndMessaging::*,
        },
    },
    core::PCWSTR,
};

use crate::config::{IncompatibleEffectHandling, Settings};

const CLASS_NAME: &str = "CreateControlObjectAux2ConfigDialog";
const WINDOW_WIDTH: i32 = 480;
const WINDOW_HEIGHT: i32 = 250;
const IDC_REMOVE_INCOMPATIBLE_EFFECTS: usize = 2001;
const IDC_SKIP_TO_CONVERTIBLE_OBJECT: usize = 2002;
const IDC_OK: usize = 2003;
const IDC_CANCEL: usize = 2004;

static REGISTER_DIALOG_CLASS: std::sync::Once = std::sync::Once::new();

struct DialogState {
    settings: std::sync::Arc<std::sync::RwLock<Settings>>,
    remove_incompatible_effects: HWND,
    skip_to_convertible_object: HWND,
}

impl DialogState {
    fn new(settings: std::sync::Arc<std::sync::RwLock<Settings>>) -> Self {
        Self {
            settings,
            remove_incompatible_effects: HWND::default(),
            skip_to_convertible_object: HWND::default(),
        }
    }

    fn create_controls(&mut self, dialog: HWND, instance: HINSTANCE) -> windows::core::Result<()> {
        create_control(
            WC_STATIC,
            &translate("変換時に非互換のエフェクトをどうするか"),
            WS_VISIBLE | WS_CHILD,
            20,
            20,
            420,
            24,
            dialog,
            0,
            instance,
        )?;
        self.remove_incompatible_effects = create_control(
            WC_BUTTON,
            &translate("非互換のエフェクトを削除"),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | WS_GROUP | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            36,
            58,
            390,
            24,
            dialog,
            IDC_REMOVE_INCOMPATIBLE_EFFECTS,
            instance,
        )?;
        self.skip_to_convertible_object = create_control(
            WC_BUTTON,
            &translate("変換できるオブジェクトまでスキップ"),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            36,
            92,
            390,
            24,
            dialog,
            IDC_SKIP_TO_CONVERTIBLE_OBJECT,
            instance,
        )?;
        create_control(
            WC_BUTTON,
            "OK",
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | WS_GROUP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            120,
            150,
            100,
            32,
            dialog,
            IDC_OK,
            instance,
        )?;
        create_control(
            WC_BUTTON,
            &translate("キャンセル"),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            260,
            150,
            100,
            32,
            dialog,
            IDC_CANCEL,
            instance,
        )?;

        unsafe { SendMessageW(dialog, DM_SETDEFID, Some(WPARAM(IDC_OK)), None) };
        self.populate();
        Ok(())
    }

    fn populate(&self) {
        let settings = self
            .settings
            .read()
            .expect("Settings lock must not be poisoned.");
        set_checked(
            match settings.incompatible_effect_handling {
                IncompatibleEffectHandling::RemoveIncompatibleEffects => {
                    self.remove_incompatible_effects
                }
                IncompatibleEffectHandling::SkipToConvertibleObject => {
                    self.skip_to_convertible_object
                }
            },
            true,
        );
    }

    fn read_settings(&self) -> aviutl2::common::AnyResult<Settings> {
        let incompatible_effect_handling = match (
            is_checked(self.remove_incompatible_effects),
            is_checked(self.skip_to_convertible_object),
        ) {
            (true, false) => IncompatibleEffectHandling::RemoveIncompatibleEffects,
            (false, true) => IncompatibleEffectHandling::SkipToConvertibleObject,
            _ => {
                aviutl2::anyhow::bail!("Exactly one incompatible effect handling must be selected.")
            }
        };
        Ok(Settings {
            incompatible_effect_handling,
        })
    }

    fn save(&self) -> aviutl2::common::AnyResult<()> {
        let settings = self.read_settings()?;
        settings.save()?;
        *self
            .settings
            .write()
            .expect("Settings lock must not be poisoned.") = settings;
        Ok(())
    }
}

pub(crate) fn show(
    parent: aviutl2::Win32WindowHandle,
    settings: std::sync::Arc<std::sync::RwLock<Settings>>,
) -> aviutl2::common::AnyResult<()> {
    let parent = HWND(parent.hwnd.get() as *mut std::ffi::c_void);
    let module = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };
    let instance = HINSTANCE(module.0);
    register_dialog_class(instance);

    let mut parent_rect = RECT::default();
    unsafe { GetWindowRect(parent, &mut parent_rect)? };
    let x = parent_rect.left + (parent_rect.right - parent_rect.left - WINDOW_WIDTH) / 2;
    let y = parent_rect.top + (parent_rect.bottom - parent_rect.top - WINDOW_HEIGHT) / 2;

    let state = Box::new(DialogState::new(settings));
    let state_pointer = Box::into_raw(state);
    let class_name = wide(CLASS_NAME);
    let title = wide(&translate("create_control_object.aux2 設定"));
    let dialog = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            Some(parent),
            None,
            Some(instance),
            Some(state_pointer.cast()),
        )
    };
    let dialog = match dialog {
        Ok(dialog) => dialog,
        Err(error) => {
            drop(unsafe { Box::from_raw(state_pointer) });
            return Err(error.into());
        }
    };

    let _parent_guard = ParentWindowGuard::new(parent);
    unsafe {
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = UpdateWindow(dialog);
    }

    let mut message = MSG::default();
    while unsafe { IsWindow(Some(dialog)).as_bool() } {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == -1 {
            let error = windows::core::Error::from_thread();
            unsafe { DestroyWindow(dialog) }.ok();
            drop(unsafe { Box::from_raw(state_pointer) });
            return Err(error.into());
        }
        if !result.as_bool() {
            unsafe {
                DestroyWindow(dialog).ok();
                PostQuitMessage(message.wParam.0 as i32);
            }
            break;
        }
        if message.message == WM_KEYDOWN && message.wParam.0 == VK_ESCAPE.0 as usize {
            unsafe { DestroyWindow(dialog)? };
            continue;
        }
        if !unsafe { IsDialogMessageW(dialog, &message) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    drop(unsafe { Box::from_raw(state_pointer) });
    Ok(())
}

fn register_dialog_class(instance: HINSTANCE) {
    REGISTER_DIALOG_CLASS.call_once(|| {
        let class_name = wide(CLASS_NAME);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(dialog_window_proc),
            hInstance: instance,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.expect("Arrow cursor must load."),
            hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let atom = unsafe { RegisterClassExW(&class) };
        assert_ne!(atom, 0, "Dialog window class must register.");
    });
}

unsafe extern "system" fn dialog_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match std::panic::catch_unwind(|| unsafe {
        dialog_window_proc_inner(hwnd, message, wparam, lparam)
    }) {
        Ok(result) => result,
        Err(_) => {
            aviutl2::tracing::error!("The config dialog panicked.");
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

unsafe fn dialog_window_proc_inner(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
    }
    let state_pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState };

    match message {
        WM_CREATE => {
            assert!(
                !state_pointer.is_null(),
                "Dialog state must be initialized."
            );
            let state = unsafe { &mut *state_pointer };
            if let Err(error) = state.create_controls(hwnd, unsafe { create_instance(hwnd) }) {
                show_error(
                    Some(hwnd),
                    &format!("設定画面の作成に失敗しました: {error}"),
                );
                unsafe { DestroyWindow(hwnd) }.ok();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let command = wparam.0 & 0xffff;
            if command == IDC_OK {
                let state = unsafe { &*state_pointer };
                match state.save() {
                    Ok(()) => {
                        unsafe { DestroyWindow(hwnd) }.ok();
                    }
                    Err(error) => show_error(
                        Some(hwnd),
                        &format!("{}: {error:#}", translate("設定を保存できません")),
                    ),
                }
                LRESULT(0)
            } else if command == IDC_CANCEL {
                unsafe { DestroyWindow(hwnd) }.ok();
                LRESULT(0)
            } else {
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) }.ok();
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn create_instance(hwnd: HWND) -> HINSTANCE {
    let value = unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) };
    HINSTANCE(value as *mut std::ffi::c_void)
}

#[allow(clippy::too_many_arguments)]
fn create_control(
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: HWND,
    id: usize,
    instance: HINSTANCE,
) -> windows::core::Result<HWND> {
    let text = wide(text);
    let control = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(text.as_ptr()),
            style,
            x,
            y,
            width,
            height,
            Some(parent),
            (id != 0).then_some(HMENU(id as *mut std::ffi::c_void)),
            Some(instance),
            None,
        )
    }?;
    let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };
    unsafe {
        SendMessageW(
            control,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        )
    };
    Ok(control)
}

fn set_checked(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(if checked { BST_CHECKED.0 as usize } else { 0 })),
            None,
        )
    };
}

fn is_checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, None, None) }.0 as u32 == BST_CHECKED.0
}

fn show_error(parent: Option<HWND>, message: &str) {
    let message = wide(message);
    let title = wide("create_control_object.aux2");
    unsafe {
        MessageBoxW(
            parent,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn translate(text: &str) -> String {
    aviutl2::config::translate(text)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

struct ParentWindowGuard(HWND);

impl ParentWindowGuard {
    fn new(parent: HWND) -> Self {
        let _ = unsafe { EnableWindow(parent, false) };
        Self(parent)
    }
}

impl Drop for ParentWindowGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = EnableWindow(self.0, true);
            let _ = SetForegroundWindow(self.0);
        }
    }
}
