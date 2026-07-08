use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDialogFilter {
    pub name: &'static str,
    pub patterns: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDialogResult {
    Selected(PathBuf),
    Cancelled,
    Unsupported { message: String },
    Failed { message: String },
}

const MARKDOWN_PATTERNS: &[&str] = &["*.md", "*.markdown", "*.mdown"];
const ALL_FILE_PATTERNS: &[&str] = &["*.*"];
const MARKDOWN_FILTERS: &[FileDialogFilter] = &[
    FileDialogFilter {
        name: "Markdown files",
        patterns: MARKDOWN_PATTERNS,
    },
    FileDialogFilter {
        name: "All files",
        patterns: ALL_FILE_PATTERNS,
    },
];

pub fn markdown_file_dialog_filters() -> &'static [FileDialogFilter] {
    MARKDOWN_FILTERS
}

pub fn open_markdown_file_dialog() -> FileDialogResult {
    platform::open_markdown_file_dialog()
}

pub fn open_folder_dialog() -> FileDialogResult {
    platform::open_folder_dialog()
}

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, path::PathBuf};

    use windows::{
        Win32::{
            Foundation::ERROR_CANCELLED,
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
                FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
                SIGDN_FILESYSPATH,
            },
        },
        core::{Error, HRESULT, PCWSTR},
    };

    use super::{FileDialogResult, markdown_file_dialog_filters};

    const HRESULT_CANCELLED: HRESULT = HRESULT(0x800704C7_u32 as i32);

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        match show_file_open_dialog(false) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) if is_cancelled(&error) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Windows file-open dialog failed: {error}"),
            },
        }
    }

    pub(super) fn open_folder_dialog() -> FileDialogResult {
        match show_file_open_dialog(true) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) if is_cancelled(&error) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Windows folder dialog failed: {error}"),
            },
        }
    }

    fn show_file_open_dialog(pick_folder: bool) -> windows::core::Result<Option<PathBuf>> {
        let _com = ApartmentCom::initialize()?;
        // SAFETY: `CoCreateInstance` instantiates a fresh COM object on the
        // apartment initialized above; `FileOpenDialog` is a documented
        // in-process shell dialog class with `CLSCTX_INPROC_SERVER` server.
        // No raw pointers are passed; the returned `IFileOpenDialog` owns its
        // refcount and is released on drop.
        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };

        if !pick_folder {
            set_markdown_filters(&dialog)?;
        }
        // SAFETY: The dialog COM object was created by `CoCreateInstance` and is used on
        // the same initialized apartment for option reads/writes.
        let options = unsafe { dialog.GetOptions()? };
        let mut options = options | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR;
        if pick_folder {
            options |= FOS_PICKFOLDERS;
        } else {
            options |= FOS_FILEMUSTEXIST;
        }
        // SAFETY: The options are the dialog's current flags OR-ed with documented
        // file-system-only constraints; no pointers or borrowed buffers are involved.
        unsafe {
            dialog.SetOptions(options)?;
        }

        // SAFETY: `Show` runs a modal UI on the owning apartment; the dialog
        // has valid options/filters set above. A user cancel surfaces as an
        // `ERROR_CANCELLED` HRESULT that we map to `Ok(None)`, not UB.
        if let Err(error) = unsafe { dialog.Show(None) } {
            if is_cancelled(&error) {
                return Ok(None);
            }
            return Err(error);
        }

        // SAFETY: `Show` succeeded, so `GetResult` may return the user-selected
        // shell item for this single-selection dialog.
        let item = unsafe { dialog.GetResult()? };
        // SAFETY: The shell item is valid (returned by `GetResult`) and
        // `SIGDN_FILESYSPATH` requests a file-system path; the returned
        // `PWSTR` is COM-allocated and freed via `CoTaskMemFree` below before
        // any other use of the apartment.
        let raw_path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
        // SAFETY: `GetDisplayName(SIGDN_FILESYSPATH)` returns a null-terminated
        // file-system path string allocated by COM for the selected shell item;
        // `to_string` copies it out into a Rust `String` before `CoTaskMemFree`
        // releases the COM allocation below.
        let path_text = unsafe { raw_path.to_string() };
        // SAFETY: `raw_path` is a COM-allocated string obtained from
        // `GetDisplayName`; `CoTaskMemFree` is its matching deallocator. The
        // pointer is cast to `c_void` per the API and is not used after this
        // call (the path has already been copied into `path_text`).
        unsafe {
            CoTaskMemFree(Some(raw_path.as_ptr().cast::<c_void>()));
        }
        Ok(Some(PathBuf::from(path_text?)))
    }

    fn set_markdown_filters(dialog: &IFileOpenDialog) -> windows::core::Result<()> {
        let display_specs: Vec<Vec<u16>> = markdown_file_dialog_filters()
            .iter()
            .map(|filter| wide_null(filter.name))
            .collect();
        let pattern_specs: Vec<Vec<u16>> = markdown_file_dialog_filters()
            .iter()
            .map(|filter| wide_null(&filter.patterns.join(";")))
            .collect();
        let specs: Vec<COMDLG_FILTERSPEC> = display_specs
            .iter()
            .zip(pattern_specs.iter())
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();

        // SAFETY: `specs` lives for the duration of this call and each
        // `COMDLG_FILTERSPEC` points at UTF-16 null-terminated buffers (`wide_null`)
        // that outlive `SetFileTypes`'s internal read; the dialog copies the
        // filter table, so no borrows escape.
        unsafe { dialog.SetFileTypes(&specs) }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn is_cancelled(error: &Error) -> bool {
        error.code() == HRESULT_CANCELLED || error.code() == ERROR_CANCELLED.to_hresult()
    }

    struct ApartmentCom {
        initialized: bool,
    }

    impl ApartmentCom {
        fn initialize() -> windows::core::Result<Self> {
            // SAFETY: `CoInitializeEx` with `COINIT_APARTMENTTHREADED` is
            // called once per thread at the start of `show_file_open_dialog`;
            // a prior init returns `S_FALSE`/`RPC_E_CHANGED_MODE` which `.ok()`
            // tolerates, and the matching `CoUninitialize` runs in `Drop`.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()?;
            Ok(Self { initialized: true })
        }
    }

    impl Drop for ApartmentCom {
        fn drop(&mut self) {
            if self.initialized {
                // SAFETY: `CoUninitialize` is the symmetric counterpart to the
                // `CoInitializeEx` call recorded by `initialized`; it balances
                // the apartment's COM refcount on this thread only.
                unsafe { CoUninitialize() };
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{collections::HashMap, path::PathBuf};

    use zbus::{
        blocking::Connection,
        proxy,
        zvariant::{OwnedObjectPath, OwnedValue, Value},
    };

    use super::FileDialogResult;

    #[proxy(
        interface = "org.freedesktop.portal.FileChooser",
        default_service = "org.freedesktop.portal.Desktop",
        default_path = "/org/freedesktop/portal/desktop"
    )]
    trait FileChooser {
        fn open_file(
            &self,
            parent_window: &str,
            title: &str,
            options: HashMap<&str, Value<'_>>,
        ) -> zbus::Result<OwnedObjectPath>;
    }

    #[proxy(
        interface = "org.freedesktop.portal.Request",
        default_service = "org.freedesktop.portal.Desktop"
    )]
    trait Request {
        #[zbus(signal)]
        fn response(&self, response: u32, results: HashMap<String, OwnedValue>)
        -> zbus::Result<()>;
    }

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        FileDialogResult::Unsupported {
            message: "Clay native file-open dialog is supported on Windows only in Phase 19."
                .to_string(),
        }
    }

    pub(super) fn open_folder_dialog() -> FileDialogResult {
        match open_folder_dialog_via_portal() {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Linux folder dialog failed: {error}"),
            },
        }
    }

    fn open_folder_dialog_via_portal() -> Result<Option<PathBuf>, String> {
        let connection = Connection::session().map_err(|error| error.to_string())?;
        let proxy =
            FileChooserProxyBlocking::new(&connection).map_err(|error| error.to_string())?;
        let mut options = HashMap::new();
        options.insert("directory", Value::from(true));
        options.insert("modal", Value::from(true));
        let handle = proxy
            .open_file("", "Open Folder", options)
            .map_err(|error| error.to_string())?;
        let request = RequestProxyBlocking::builder(&connection)
            .path(handle)
            .map_err(|error| error.to_string())?
            .build()
            .map_err(|error| error.to_string())?;
        let mut responses = request
            .receive_response()
            .map_err(|error| error.to_string())?;
        let signal = responses
            .next()
            .ok_or_else(|| "portal response stream ended".to_string())
            .map_err(|error| error.to_string())?;
        let args = signal.args().map_err(|error| error.to_string())?;
        if *args.response() != 0 {
            return Ok(None);
        }
        let uris = args
            .results()
            .get("uris")
            .ok_or_else(|| "portal response did not include selected URI".to_string())?;
        let uris = Vec::<String>::try_from(uris.clone())
            .map_err(|error| format!("portal returned invalid URI list: {error}"))?;
        let Some(uri) = uris.first() else {
            return Ok(None);
        };
        file_uri_to_path(uri)
    }

    fn file_uri_to_path(uri: &str) -> Result<Option<PathBuf>, String> {
        let parsed = url::Url::parse(uri).map_err(|error| format!("invalid URI: {error}"))?;
        if parsed.scheme() != "file" {
            return Err("portal returned a non-file URI".to_string());
        }
        parsed
            .to_file_path()
            .map(Some)
            .map_err(|_| "portal returned an invalid file URI path".to_string())
    }

    #[cfg(test)]
    pub(super) fn parse_file_uri_for_test(uri: &str) -> Result<Option<PathBuf>, String> {
        file_uri_to_path(uri)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::FileDialogResult;

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        FileDialogResult::Unsupported {
            message: "Clay native file-open dialog is supported on Windows only in Phase 19."
                .to_string(),
        }
    }

    pub(super) fn open_folder_dialog() -> FileDialogResult {
        FileDialogResult::Unsupported {
            message: "Clay native folder dialog is not supported on this platform yet.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::markdown_file_dialog_filters;
    #[cfg(not(windows))]
    use super::{FileDialogResult, open_markdown_file_dialog};

    #[test]
    fn windows_file_dialog_filter_allows_markdown_extensions() {
        let filters = markdown_file_dialog_filters();
        let markdown = filters
            .iter()
            .find(|filter| filter.name == "Markdown files")
            .expect("Markdown filter exists");

        assert_eq!(markdown.patterns, ["*.md", "*.markdown", "*.mdown"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_open_file_dialog_reports_unsupported() {
        assert_eq!(
            open_markdown_file_dialog(),
            FileDialogResult::Unsupported {
                message: "Clay native file-open dialog is supported on Windows only in Phase 19."
                    .to_string(),
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_file_uri_parser_accepts_file_paths() {
        assert_eq!(
            super::platform::parse_file_uri_for_test("file:///tmp/clay%20workspace").unwrap(),
            Some(std::path::PathBuf::from("/tmp/clay workspace"))
        );
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    #[test]
    fn unsupported_platform_open_folder_dialog_reports_unsupported() {
        assert_eq!(
            super::open_folder_dialog(),
            FileDialogResult::Unsupported {
                message: "Clay native folder dialog is not supported on this platform yet."
                    .to_string(),
            }
        );
    }
}
