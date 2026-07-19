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

/// Translate a shared glob pattern into a portal FileChooser glob.
///
/// Portal filters use shell-style globs. Windows' `*.*` all-files sentinel becomes
/// `*` so extensionless names remain selectable.
#[cfg_attr(not(any(test, target_os = "linux")), allow(dead_code))]
pub(crate) fn portal_glob_for_pattern(pattern: &str) -> Option<&str> {
    match pattern {
        "" => None,
        "*.*" | "*" => Some("*"),
        other => Some(other),
    }
}

/// Extract macOS `allowedFileTypes` extensions from shared globs.
///
/// Returns extension tokens without the leading `*.`. All-files sentinels (`*.*` / `*`)
/// are ignored for the extension list; callers that want an all-files fallback should
/// pair this with `allowsOtherFileTypes = true` or clear allowed types when the list is
/// empty.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub(crate) fn macos_allowed_extensions(filters: &[FileDialogFilter]) -> Vec<String> {
    let mut extensions = Vec::new();
    for filter in filters {
        for pattern in filter.patterns {
            match *pattern {
                "*.*" | "*" => {}
                other => {
                    let trimmed = other
                        .strip_prefix("*.")
                        .or_else(|| other.strip_prefix('.'))
                        .unwrap_or(other);
                    if !trimmed.is_empty()
                        && !trimmed.contains(['*', '?', '/', '\\'])
                        && !extensions.iter().any(|existing| existing == trimmed)
                    {
                        extensions.push(trimmed.to_string());
                    }
                }
            }
        }
    }
    extensions
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

    use super::{FileDialogResult, markdown_file_dialog_filters, portal_glob_for_pattern};

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
        match open_dialog_via_portal(OpenDialogKind::File) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Linux file-open dialog failed: {error}"),
            },
        }
    }

    pub(super) fn open_folder_dialog() -> FileDialogResult {
        match open_dialog_via_portal(OpenDialogKind::Folder) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("Linux folder dialog failed: {error}"),
            },
        }
    }

    #[derive(Clone, Copy)]
    enum OpenDialogKind {
        File,
        Folder,
    }

    fn open_dialog_via_portal(kind: OpenDialogKind) -> Result<Option<PathBuf>, String> {
        use std::sync::atomic::{AtomicU64, Ordering};

        static HANDLE_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

        let connection = Connection::session().map_err(|error| error.to_string())?;
        let proxy =
            FileChooserProxyBlocking::new(&connection).map_err(|error| error.to_string())?;
        let sender = connection
            .unique_name()
            .ok_or_else(|| "D-Bus connection has no unique name".to_string())?
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = format!(
            "clay{}_{}",
            std::process::id(),
            HANDLE_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");

        // Subscribe before OpenFile: the portal may emit Response immediately after
        // returning its Request handle.
        let request = RequestProxyBlocking::builder(&connection)
            .path(request_path.as_str())
            .map_err(|error| error.to_string())?
            .build()
            .map_err(|error| error.to_string())?;
        let mut responses = request
            .receive_response()
            .map_err(|error| error.to_string())?;

        let mut options = HashMap::new();
        options.insert("handle_token", Value::from(token));
        options.insert("modal", Value::from(false));
        match kind {
            OpenDialogKind::Folder => {
                options.insert("directory", Value::from(true));
            }
            OpenDialogKind::File => {
                options.insert("directory", Value::from(false));
                options.insert("filters", portal_filters_value());
            }
        }
        let title = match kind {
            OpenDialogKind::File => "Open File",
            OpenDialogKind::Folder => "Open Folder",
        };
        let handle = proxy
            .open_file("", title, options)
            .map_err(|error| error.to_string())?;
        if handle.as_str() != request_path {
            return Err(format!(
                "portal returned unexpected request handle {handle}; expected {request_path}"
            ));
        }

        let signal = responses
            .next()
            .ok_or_else(|| "portal response stream ended".to_string())?;
        let args = signal.args().map_err(|error| error.to_string())?;
        match *args.response() {
            0 => {}
            1 => return Ok(None),
            code => {
                return Err(format!(
                    "portal file chooser failed with response code {code}"
                ));
            }
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

    fn portal_filters_value() -> Value<'static> {
        type Rule = (u32, String);
        type Filter = (String, Vec<Rule>);
        let filters: Vec<Filter> = markdown_file_dialog_filters()
            .iter()
            .filter_map(|filter| {
                let rules: Vec<Rule> = filter
                    .patterns
                    .iter()
                    .filter_map(|pattern| {
                        portal_glob_for_pattern(pattern).map(|glob| (0u32, glob.to_string()))
                    })
                    .collect();
                if rules.is_empty() {
                    None
                } else {
                    Some((filter.name.to_string(), rules))
                }
            })
            .collect();
        Value::new(filters)
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

    #[cfg(test)]
    pub(super) fn portal_filters_signature_for_test() -> String {
        portal_filters_value().value_signature().to_string()
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::PathBuf;

    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSArray, NSString, NSURL};

    use super::{FileDialogResult, macos_allowed_extensions, markdown_file_dialog_filters};

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        match show_open_panel(false) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("macOS file-open dialog failed: {error}"),
            },
        }
    }

    pub(super) fn open_folder_dialog() -> FileDialogResult {
        match show_open_panel(true) {
            Ok(Some(path)) => FileDialogResult::Selected(path),
            Ok(None) => FileDialogResult::Cancelled,
            Err(error) => FileDialogResult::Failed {
                message: format!("macOS folder dialog failed: {error}"),
            },
        }
    }

    fn show_open_panel(pick_folder: bool) -> Result<Option<PathBuf>, String> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "macOS open panel must run on the main thread".to_string())?;
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(!pick_folder);
        panel.setCanChooseDirectories(pick_folder);
        panel.setAllowsMultipleSelection(false);
        panel.setResolvesAliases(true);
        let title = if pick_folder {
            NSString::from_str("Open Folder")
        } else {
            NSString::from_str("Open File")
        };
        panel.setTitle(Some(&title));
        if !pick_folder {
            apply_markdown_filters(&panel);
        }

        let response = panel.runModal();
        if response != NSModalResponseOK {
            return Ok(None);
        }

        let urls = panel.URLs();
        let Some(url) = urls.firstObject() else {
            return Ok(None);
        };
        path_from_url(&url)
    }

    fn apply_markdown_filters(panel: &NSOpenPanel) {
        let extensions = macos_allowed_extensions(markdown_file_dialog_filters());
        if extensions.is_empty() {
            #[allow(deprecated)]
            panel.setAllowedFileTypes(None);
            return;
        }
        let ns_extensions: Vec<_> = extensions
            .iter()
            .map(|extension| NSString::from_str(extension))
            .collect();
        let types = NSArray::from_retained_slice(&ns_extensions);
        // Deprecated allowedFileTypes still accepts extension tokens and avoids
        // pulling UniformTypeIdentifiers solely for Markdown filters. All-files
        // fallback is preserved by allowing other types beyond the Markdown list.
        #[allow(deprecated)]
        panel.setAllowedFileTypes(Some(&types));
        panel.setAllowsOtherFileTypes(true);
    }

    fn path_from_url(url: &NSURL) -> Result<Option<PathBuf>, String> {
        let Some(path) = url.path() else {
            return Err("macOS open panel returned a URL without a file path".to_string());
        };
        Ok(Some(PathBuf::from(path.to_string())))
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    use super::FileDialogResult;

    pub(super) fn open_markdown_file_dialog() -> FileDialogResult {
        FileDialogResult::Unsupported {
            message: "Clay native file-open dialog is not supported on this platform yet."
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
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    use super::{FileDialogResult, open_folder_dialog, open_markdown_file_dialog};
    use super::{macos_allowed_extensions, markdown_file_dialog_filters, portal_glob_for_pattern};

    #[test]
    fn windows_file_dialog_filter_allows_markdown_extensions() {
        let filters = markdown_file_dialog_filters();
        let markdown = filters
            .iter()
            .find(|filter| filter.name == "Markdown files")
            .expect("Markdown filter exists");

        assert_eq!(markdown.patterns, ["*.md", "*.markdown", "*.mdown"]);
    }

    #[test]
    fn portal_glob_normalizes_all_files_sentinel() {
        assert_eq!(portal_glob_for_pattern("*.*"), Some("*"));
        assert_eq!(portal_glob_for_pattern("*.md"), Some("*.md"));
        assert_eq!(portal_glob_for_pattern(""), None);
    }

    #[test]
    fn macos_extensions_ignore_all_files_sentinel_and_keep_markdown_tokens() {
        assert_eq!(
            macos_allowed_extensions(markdown_file_dialog_filters()),
            vec![
                "md".to_string(),
                "markdown".to_string(),
                "mdown".to_string()
            ]
        );
    }

    #[test]
    fn macos_extensions_extract_markdown_tokens_without_all_files() {
        let filters = [super::FileDialogFilter {
            name: "Markdown files",
            patterns: &["*.md", "*.markdown", "*.mdown"],
        }];
        assert_eq!(
            macos_allowed_extensions(&filters),
            vec![
                "md".to_string(),
                "markdown".to_string(),
                "mdown".to_string()
            ]
        );
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    #[test]
    fn unsupported_platform_open_file_dialog_reports_unsupported() {
        assert_eq!(
            open_markdown_file_dialog(),
            FileDialogResult::Unsupported {
                message: "Clay native file-open dialog is not supported on this platform yet."
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_portal_filters_use_file_chooser_signature() {
        assert_eq!(
            super::platform::portal_filters_signature_for_test(),
            "a(sa(us))"
        );
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    #[test]
    fn unsupported_platform_open_folder_dialog_reports_unsupported() {
        assert_eq!(
            open_folder_dialog(),
            FileDialogResult::Unsupported {
                message: "Clay native folder dialog is not supported on this platform yet."
                    .to_string(),
            }
        );
    }
}
