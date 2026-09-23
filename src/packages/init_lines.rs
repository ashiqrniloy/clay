//! Exact-shape `init.js` load-line manager (plan 115 task 4).
//!
//! Clay-initiated install appends one marked `loadPackage("<name>")` block;
//! remove deletes that block only when the bytes still match. User-written
//! or edited lines are left untouched. No JS parsing.

use std::fs;
use std::path::{Path, PathBuf};

pub fn init_js_path(config_root: &Path) -> PathBuf {
    config_root.join("init.js")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendOutcome {
    Added,
    AlreadyPresent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveOutcome {
    Removed,
    Absent,
    /// A `loadPackage("<name>")` line exists but is not the Clay-appended shape.
    LeftUntouched,
}

#[derive(Debug)]
pub enum InitLineError {
    MissingInitJs { path: PathBuf },
    InvalidName { name: String },
    Io(std::io::Error),
}

impl std::fmt::Display for InitLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingInitJs { path } => write!(
                f,
                "packages.init_lines.missing: {} does not exist",
                path.display()
            ),
            Self::InvalidName { name } => {
                write!(f, "packages.init_lines.invalid_name: `{name}`")
            }
            Self::Io(error) => write!(f, "packages.init_lines.io: {error}"),
        }
    }
}

impl std::error::Error for InitLineError {}

/// Exact Clay-appended block, including the trailing newline of the load line.
pub fn clay_load_block(name: &str) -> Result<String, InitLineError> {
    validate_name(name)?;
    Ok(format!(
        "// clay install npm:{name} — remove with `clay remove npm:{name}`\nawait loadPackage(\"{name}\");\n"
    ))
}

pub fn append_load_line(init_js: &Path, name: &str) -> Result<AppendOutcome, InitLineError> {
    let block = clay_load_block(name)?;
    let contents = read_init_js(init_js)?;
    let block = with_file_newline(&block, &contents);
    if contains_block(&contents, &block) {
        return Ok(AppendOutcome::AlreadyPresent);
    }
    let mut out = contents;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&block);
    write_init_js(init_js, out.as_bytes())?;
    Ok(AppendOutcome::Added)
}

pub fn remove_load_line(init_js: &Path, name: &str) -> Result<RemoveOutcome, InitLineError> {
    let block = clay_load_block(name)?;
    let contents = read_init_js(init_js)?;
    let block = with_file_newline(&block, &contents);
    if let Some(range) = block_range(&contents, &block) {
        let mut out = String::with_capacity(contents.len() - (range.end - range.start));
        out.push_str(&contents[..range.start]);
        out.push_str(&contents[range.end..]);
        write_init_js(init_js, out.as_bytes())?;
        return Ok(RemoveOutcome::Removed);
    }
    if has_user_load_call(&contents, name) {
        Ok(RemoveOutcome::LeftUntouched)
    } else {
        Ok(RemoveOutcome::Absent)
    }
}

fn validate_name(name: &str) -> Result<(), InitLineError> {
    if name.is_empty()
        || name
            .chars()
            .any(|c| c == '"' || c == '\'' || c == '\\' || c == '\n' || c == '\r')
    {
        return Err(InitLineError::InvalidName {
            name: name.to_string(),
        });
    }
    Ok(())
}

fn read_init_js(path: &Path) -> Result<String, InitLineError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(InitLineError::MissingInitJs {
                path: path.to_path_buf(),
            })
        }
        Err(error) => Err(InitLineError::Io(error)),
    }
}

fn write_init_js(path: &Path, bytes: &[u8]) -> Result<(), InitLineError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "init.js".to_string());
    let temp = parent.join(format!(".{stem}.tmp-{}", std::process::id()));
    let result = (|| {
        fs::write(&temp, bytes)?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(InitLineError::Io)
}

fn with_file_newline(block: &str, contents: &str) -> String {
    if contents.contains("\r\n") {
        block.replace('\n', "\r\n")
    } else {
        block.to_string()
    }
}

fn contains_block(contents: &str, block: &str) -> bool {
    block_range(contents, block).is_some()
}

fn block_range(contents: &str, block: &str) -> Option<std::ops::Range<usize>> {
    let trimmed = block.trim_end_matches(['\n', '\r']);
    if let Some(start) = contents.find(block) {
        return Some(start..start + block.len());
    }
    contents.find(trimmed).map(|start| {
        let mut end = start + trimmed.len();
        if contents[end..].starts_with("\r\n") {
            end += 2;
        } else if contents[end..].starts_with('\n') {
            end += 1;
        }
        start..end
    })
}

fn has_user_load_call(contents: &str, name: &str) -> bool {
    contents.contains(&format!("loadPackage(\"{name}\")"))
        || contents.contains(&format!("loadPackage('{name}')"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_init(name: &str, body: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-init-lines-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("init.js");
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn append_is_idempotent_and_remove_deletes_only_clay_block() {
        let path = temp_init(
            "idempotent",
            "import { loadPackage } from \"clay:packages\";\n",
        );
        let name = "@arnilo/st";
        assert_eq!(append_load_line(&path, name).unwrap(), AppendOutcome::Added);
        assert_eq!(
            append_load_line(&path, name).unwrap(),
            AppendOutcome::AlreadyPresent
        );
        assert_eq!(
            append_load_line(&path, name).unwrap(),
            AppendOutcome::AlreadyPresent
        );
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents
                .matches("await loadPackage(\"@arnilo/st\");")
                .count(),
            1
        );
        assert!(contents.contains("// clay install npm:@arnilo/st"));

        assert_eq!(
            remove_load_line(&path, name).unwrap(),
            RemoveOutcome::Removed
        );
        let after = fs::read_to_string(&path).unwrap();
        assert!(!after.contains("loadPackage(\"@arnilo/st\")"));
        assert!(after.contains("import { loadPackage }"));

        assert_eq!(append_load_line(&path, name).unwrap(), AppendOutcome::Added);
        assert_eq!(
            fs::read_to_string(&path)
                .unwrap()
                .matches("await loadPackage(\"@arnilo/st\");")
                .count(),
            1
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn remove_leaves_user_written_load_line() {
        let path = temp_init(
            "user-line",
            "import { loadPackage } from \"clay:packages\";\nawait loadPackage(\"@arnilo/st\");\n",
        );
        assert_eq!(
            remove_load_line(&path, "@arnilo/st").unwrap(),
            RemoveOutcome::LeftUntouched
        );
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("await loadPackage(\"@arnilo/st\");"));
        assert!(!contents.contains("// clay install"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_init_js_fails_closed() {
        let path = PathBuf::from("/tmp/clay-init-lines-missing-no-such/init.js");
        match append_load_line(&path, "plain-mode") {
            Err(InitLineError::MissingInitJs { .. }) => {}
            other => panic!("expected missing, got {other:?}"),
        }
    }

    #[test]
    fn edited_clay_comment_is_left_untouched() {
        let path = temp_init(
            "edited",
            "// I edited this\nawait loadPackage(\"plain-mode\");\n",
        );
        assert_eq!(
            remove_load_line(&path, "plain-mode").unwrap(),
            RemoveOutcome::LeftUntouched
        );
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("await loadPackage(\"plain-mode\");")
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
