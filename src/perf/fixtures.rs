use std::{
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

const DEFAULT_FIXTURE_DIR: &str = "target/perf-fixtures";
const COMMITTED_FIXTURE_DIR: &str = "tests/fixtures/perf";
const WRITE_BUFFER_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureKind {
    LongLines,
    ManyShortLines,
    MixedUnicode,
    NewlineHeavy,
}

impl FixtureKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "long-lines" => Some(Self::LongLines),
            "many-short-lines" => Some(Self::ManyShortLines),
            "mixed-unicode" => Some(Self::MixedUnicode),
            "newline-heavy" => Some(Self::NewlineHeavy),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LongLines => "long-lines",
            Self::ManyShortLines => "many-short-lines",
            Self::MixedUnicode => "mixed-unicode",
            Self::NewlineHeavy => "newline-heavy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureSpec {
    pub kind: FixtureKind,
    pub size_bytes: usize,
    pub seed: u64,
}

impl FixtureSpec {
    pub fn new(kind: FixtureKind, size_bytes: usize) -> Self {
        Self {
            kind,
            size_bytes,
            seed: 0xC1A4_F14E,
        }
    }
}

#[derive(Debug)]
pub enum FixtureError {
    UnsafeOutputPath { path: PathBuf, reason: String },
    Io(io::Error),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeOutputPath { path, reason } => {
                write!(
                    formatter,
                    "unsafe performance fixture output path {}: {reason}",
                    path.display()
                )
            }
            Self::Io(error) => write!(formatter, "performance fixture IO failed: {error}"),
        }
    }
}

impl Error for FixtureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::UnsafeOutputPath { .. } => None,
        }
    }
}

impl From<io::Error> for FixtureError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn default_fixture_path(kind: FixtureKind, size_mib: usize) -> PathBuf {
    repository_root()
        .join(DEFAULT_FIXTURE_DIR)
        .join(format!("{}-{}m.txt", kind.as_str(), size_mib))
}

pub fn generate_fixture_file(spec: &FixtureSpec, output: &Path) -> Result<PathBuf, FixtureError> {
    let output = validate_output_path(output)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(&output)?;
    generate_fixture(spec, &mut file)?;
    file.flush()?;
    Ok(output)
}

pub fn generate_fixture(spec: &FixtureSpec, writer: &mut impl Write) -> Result<(), FixtureError> {
    let mut generator = FixtureGenerator::new(spec.kind, spec.seed);
    let mut written = 0usize;
    let mut buffer = String::with_capacity(WRITE_BUFFER_LIMIT);

    while written < spec.size_bytes {
        let remaining = spec.size_bytes - written;
        let line = generator.next_line();
        if line.len() <= remaining {
            buffer.push_str(&line);
            written += line.len();
        } else {
            append_ascii_fill(&mut buffer, remaining);
            written += remaining;
        }

        if buffer.len() >= WRITE_BUFFER_LIMIT {
            writer.write_all(buffer.as_bytes())?;
            buffer.clear();
        }
    }

    if !buffer.is_empty() {
        writer.write_all(buffer.as_bytes())?;
    }
    Ok(())
}

pub fn validate_output_path(path: &Path) -> Result<PathBuf, FixtureError> {
    let repository_root = repository_root();
    let candidate = if path.is_absolute() {
        normalize_without_parent(path)?
    } else {
        normalize_without_parent(&repository_root.join(path))?
    };

    let allowed_target = repository_root.join(DEFAULT_FIXTURE_DIR);
    let allowed_committed = repository_root.join(COMMITTED_FIXTURE_DIR);
    if candidate.starts_with(&allowed_target) || candidate.starts_with(&allowed_committed) {
        Ok(candidate)
    } else {
        Err(FixtureError::UnsafeOutputPath {
            path: path.to_path_buf(),
            reason: format!(
                "output must be under {} or {}",
                allowed_target.display(),
                allowed_committed.display()
            ),
        })
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn normalize_without_parent(path: &Path) -> Result<PathBuf, FixtureError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(FixtureError::UnsafeOutputPath {
                    path: path.to_path_buf(),
                    reason: "parent-directory traversal is not allowed".to_string(),
                });
            }
        }
    }
    Ok(normalized)
}

fn append_ascii_fill(buffer: &mut String, byte_count: usize) {
    const FILL: &[u8] = b" clay-perf-fixture ";
    let mut remaining = byte_count;
    while remaining > 0 {
        let take = remaining.min(FILL.len());
        buffer.push_str(std::str::from_utf8(&FILL[..take]).expect("ASCII fill is UTF-8"));
        remaining -= take;
    }
}

struct FixtureGenerator {
    kind: FixtureKind,
    state: u64,
    line: usize,
}

impl FixtureGenerator {
    fn new(kind: FixtureKind, seed: u64) -> Self {
        Self {
            kind,
            state: seed,
            line: 0,
        }
    }

    fn next_line(&mut self) -> String {
        let line = self.line;
        self.line += 1;
        match self.kind {
            FixtureKind::LongLines => self.long_line(line),
            FixtureKind::ManyShortLines => self.short_line(line),
            FixtureKind::MixedUnicode => self.unicode_line(line),
            FixtureKind::NewlineHeavy => self.newline_heavy(line),
        }
    }

    fn long_line(&mut self, line: usize) -> String {
        let mut output = format!("long-line-{line:08} ");
        while output.len() < 8192 {
            output.push_str(token(self.next_u32() as usize));
            output.push(' ');
        }
        output.push('\n');
        output
    }

    fn short_line(&mut self, line: usize) -> String {
        format!("l{line:08} {}\n", token(self.next_u32() as usize))
    }

    fn unicode_line(&mut self, line: usize) -> String {
        const SCALARS: &[&str] = &[
            "é", "ß", "Ж", "中", "文", "🙂", "🦀", "𝄞", "क", "ا", "ש", "ह", "🌍",
        ];
        let mut output = format!("unicode-{line:08} ");
        for _ in 0..24 {
            let index = self.next_u32() as usize % SCALARS.len();
            output.push_str(SCALARS[index]);
            output.push(' ');
            output.push_str(token(self.next_u32() as usize));
            output.push(' ');
        }
        output.push('\n');
        output
    }

    fn newline_heavy(&mut self, line: usize) -> String {
        match line % 6 {
            0 => "\n".to_string(),
            1 => format!("paragraph-{line:08}\n\n"),
            2 => "\n\n\n".to_string(),
            _ => format!("n{line}\n"),
        }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }
}

fn token(index: usize) -> &'static str {
    const TOKENS: &[&str] = &[
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliet", "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo",
    ];
    TOKENS[index % TOKENS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bytes_are_valid_utf8_and_exact_size() {
        let spec = FixtureSpec::new(FixtureKind::MixedUnicode, 4097);
        let mut bytes = Vec::new();

        generate_fixture(&spec, &mut bytes).unwrap();

        assert_eq!(bytes.len(), spec.size_bytes);
        std::str::from_utf8(&bytes).expect("fixture output remains valid UTF-8");
    }
}
