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
                FOS_PATHMUSTEXIST, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
            },
        },
        core::{Error, HRESULT, PCWSTR},
    };

    use super::{FileDialogResult, markdown_file_dialog_filters};

    const HRESULT_CANCELLED: HRESULT = HRESULT(0x800704C7_u32 as i32);

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        match show_file_open_dialog() {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) if is_cancelled(&error) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Windows file-open dialog failed: {error}"),
            },
        }
    }

    fn show_file_open_dialog() -> windows::core::Result<Option<PathBuf>> {
        let _com = ApartmentCom::initialize()?;
        // SAFETY: `CoCreateInstance` instantiates a fresh COM object on the
        // apartment initialized above; `FileOpenDialog` is a documented
        // in-process shell dialog class with `CLSCTX_INPROC_SERVER` server.
        // No raw pointers are passed; the returned `IFileOpenDialog` owns its
        // refcount and is released on drop.
        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };

        set_markdown_filters(&dialog)?;
        // SAFETY: The dialog COM object was created by `CoCreateInstance` and is used on
        // the same initialized apartment for option reads/writes.
        let options = unsafe { dialog.GetOptions()? };
        // SAFETY: The options are the dialog's current flags OR-ed with documented
        // file-system-only constraints; no pointers or borrowed buffers are involved.
        unsafe {
            dialog.SetOptions(
                options
                    | FOS_FORCEFILESYSTEM
                    | FOS_FILEMUSTEXIST
                    | FOS_PATHMUSTEXIST
                    | FOS_NOCHANGEDIR,
            )?;
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

#[cfg(not(windows))]
mod platform {
    use super::FileDialogResult;

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        FileDialogResult::Unsupported {
            message: "Clay native file-open dialog is supported on Windows only in Phase 19."
                .to_string(),
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
}
