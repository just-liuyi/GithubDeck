//! Windows integration for the GPUI shell: browser detection, launching a
//! specific browser, the native file picker and clipboard.

use ghdeck_core::{BrowserPlatform, BrowserTarget};
pub struct DesktopPlatform;

impl DesktopPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl BrowserPlatform for DesktopPlatform {
    fn detect(&self) -> Vec<BrowserTarget> {
        ghdeck_core::detect()
    }

    /// `None` asks Windows to open the system default browser.
    fn open(&self, target: Option<&BrowserTarget>, url: &str) -> Result<(), String> {
        match target {
            Some(target) => ghdeck_core::launch(target, url),
            None => open_with_shell(url),
        }
    }
}

#[cfg(windows)]
fn open_with_shell(url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // `start` is a cmd builtin, so go through cmd with an empty window title.
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .creation_flags(0x0800_0000)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| "无法打开系统默认浏览器".to_string())
}

#[cfg(not(windows))]
fn open_with_shell(_url: &str) -> Result<(), String> {
    Err("当前平台不支持打开浏览器".into())
}

/// Native open dialog for gh.exe. Returns the chosen path.
#[cfg(windows)]
pub fn pick_executable() -> Option<String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[repr(C)]
    struct OpenFileName {
        l_struct_size: u32,
        hwnd_owner: *mut c_void,
        h_instance: *mut c_void,
        lpstr_filter: *const u16,
        lpstr_custom_filter: *const u16,
        n_max_cust_filter: u32,
        n_filter_index: u32,
        lpstr_file: *mut u16,
        n_max_file: u32,
        lpstr_file_title: *const u16,
        flags: u32,
        n_file_offset: u16,
        n_file_extension: u16,
        lpstr_def_ext: *const u16,
        lpstr_custom_data: *const u16,
        lpfn_hook: *const c_void,
        lp_template_name: *const u16,
        pv_reserved: *mut c_void,
        dw_reserved: u32,
        flags_ex: u32,
        pv_reserved2: *mut c_void,
        pv_reserved3: *mut c_void,
    }

    #[link(name = "comdlg32")]
    unsafe extern "system" {
        fn GetOpenFileNameW(ofn: *mut OpenFileName) -> i32;
    }

    const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
    const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
    const OFN_EXPLORER: u32 = 0x0008_0000;
    const OFN_NOCHANGEDIR: u32 = 0x0000_0008;

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let mut buffer = vec![0u16; 1024];
    let filter: Vec<u16> = "GitHub CLI (gh.exe)\0*.exe\0所有文件\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title = wide("选择 gh.exe");

    let mut ofn = OpenFileName {
        l_struct_size: std::mem::size_of::<OpenFileName>() as u32,
        hwnd_owner: ptr::null_mut(),
        h_instance: ptr::null_mut(),
        lpstr_filter: filter.as_ptr(),
        lpstr_custom_filter: ptr::null(),
        n_max_cust_filter: 0,
        n_filter_index: 0,
        lpstr_file: buffer.as_mut_ptr(),
        n_max_file: buffer.len() as u32,
        lpstr_file_title: title.as_ptr(),
        flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_NOCHANGEDIR,
        n_file_offset: 0,
        n_file_extension: 0,
        lpstr_def_ext: ptr::null(),
        lpstr_custom_data: ptr::null(),
        lpfn_hook: ptr::null(),
        lp_template_name: ptr::null(),
        pv_reserved: ptr::null_mut(),
        dw_reserved: 0,
        flags_ex: 0,
        pv_reserved2: ptr::null_mut(),
        pv_reserved3: ptr::null_mut(),
    };

    // SAFETY: `ofn` points at live buffers that outlive the call.
    let accepted = unsafe { GetOpenFileNameW(&mut ofn) } != 0;
    if !accepted {
        return None;
    }
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..end]);
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(not(windows))]
pub fn pick_executable() -> Option<String> {
    None
}
