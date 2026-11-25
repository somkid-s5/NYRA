use crate::{
    settings::current_custom_theme_path,
    theme::{Theme, ThemeParseError},
};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const MAX_CUSTOM_THEME_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomThemeFileError {
    NoLocation,
    ReadFailed,
    TooLarge,
    Invalid(&'static str),
    AlreadyExists,
    WriteFailed,
}

impl CustomThemeFileError {
    pub const fn user_message(self) -> &'static str {
        match self {
            Self::NoLocation => "NYRA has no writable theme directory",
            Self::ReadFailed => "custom theme file could not be read",
            Self::TooLarge => "custom theme file exceeds the 16 KiB size limit",
            Self::Invalid(message) => message,
            Self::AlreadyExists => "destination already exists; pass --force to replace it",
            Self::WriteFailed => "custom theme file could not be written",
        }
    }

    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Invalid(_) | Self::TooLarge | Self::AlreadyExists => 2,
            Self::NoLocation | Self::ReadFailed | Self::WriteFailed => 3,
        }
    }
}

impl std::fmt::Display for CustomThemeFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.user_message())
    }
}

impl std::error::Error for CustomThemeFileError {}

pub fn validate_custom_theme_file(path: &Path) -> Result<String, CustomThemeFileError> {
    let source = read_bounded(path)?;
    parse_theme(&source).map(|theme| theme.name().to_owned())
}

pub fn import_custom_theme_file(
    source_path: &Path,
    replace_existing: bool,
) -> Result<PathBuf, CustomThemeFileError> {
    let source = read_bounded(source_path)?;
    parse_theme(&source)?;
    let destination = current_custom_theme_path().ok_or(CustomThemeFileError::NoLocation)?;
    write_theme(&destination, source.as_bytes(), replace_existing, true)?;
    Ok(destination)
}

pub fn export_custom_theme_file(
    destination: &Path,
    replace_existing: bool,
) -> Result<PathBuf, CustomThemeFileError> {
    let source_path = current_custom_theme_path().ok_or(CustomThemeFileError::NoLocation)?;
    let source = read_bounded(&source_path)?;
    parse_theme(&source)?;
    write_theme(destination, source.as_bytes(), replace_existing, false)?;
    Ok(destination.to_path_buf())
}

fn parse_theme(source: &str) -> Result<Theme, CustomThemeFileError> {
    Theme::parse_custom(source)
        .map_err(|error: ThemeParseError| CustomThemeFileError::Invalid(error.user_message()))
}

fn read_bounded(path: &Path) -> Result<String, CustomThemeFileError> {
    let metadata = fs::metadata(path).map_err(|_| CustomThemeFileError::ReadFailed)?;
    if !metadata.is_file() {
        return Err(CustomThemeFileError::ReadFailed);
    }

    let mut options = fs::OpenOptions::new();
    options.read(true);
    configure_nonblocking_file_open(&mut options);
    let file = options
        .open(path)
        .map_err(|_| CustomThemeFileError::ReadFailed)?;
    if !file
        .metadata()
        .map_err(|_| CustomThemeFileError::ReadFailed)?
        .is_file()
    {
        return Err(CustomThemeFileError::ReadFailed);
    }

    let mut reader = file.take(MAX_CUSTOM_THEME_BYTES + 1);
    let mut source = String::new();
    reader
        .read_to_string(&mut source)
        .map_err(|_| CustomThemeFileError::ReadFailed)?;
    if source.len() as u64 > MAX_CUSTOM_THEME_BYTES {
        return Err(CustomThemeFileError::TooLarge);
    }
    Ok(source)
}

#[cfg(unix)]
fn configure_nonblocking_file_open(options: &mut fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.custom_flags(libc::O_NONBLOCK);
}

#[cfg(not(unix))]
fn configure_nonblocking_file_open(_options: &mut fs::OpenOptions) {}

fn write_theme(
    destination: &Path,
    source: &[u8],
    replace_existing: bool,
    create_parent: bool,
) -> Result<(), CustomThemeFileError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if create_parent {
        fs::create_dir_all(parent).map_err(|_| CustomThemeFileError::WriteFailed)?;
    }
    if !replace_existing && destination.exists() {
        return Err(CustomThemeFileError::AlreadyExists);
    }

    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|_| CustomThemeFileError::WriteFailed)?;
    temporary
        .write_all(source)
        .map_err(|_| CustomThemeFileError::WriteFailed)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| CustomThemeFileError::WriteFailed)?;
    if replace_existing {
        temporary
            .persist(destination)
            .map_err(|_| CustomThemeFileError::WriteFailed)?;
    } else {
        temporary.persist_noclobber(destination).map_err(|error| {
            if error.error.kind() == io::ErrorKind::AlreadyExists {
                CustomThemeFileError::AlreadyExists
            } else {
                CustomThemeFileError::WriteFailed
            }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_the_shipped_custom_theme_example() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/custom-theme.toml");

        assert_eq!(
            validate_custom_theme_file(&example).unwrap(),
            "Midnight Circuit"
        );
    }

    #[test]
    fn rejects_invalid_and_oversized_theme_files() {
        let workspace = tempfile::tempdir().unwrap();
        let invalid = workspace.path().join("invalid.toml");
        fs::write(&invalid, "version = 2\n").unwrap();
        assert!(matches!(
            validate_custom_theme_file(&invalid),
            Err(CustomThemeFileError::Invalid(_))
        ));

        let oversized = workspace.path().join("oversized.toml");
        fs::write(
            &oversized,
            vec![b' '; (MAX_CUSTOM_THEME_BYTES + 8) as usize],
        )
        .unwrap();
        assert_eq!(
            validate_custom_theme_file(&oversized),
            Err(CustomThemeFileError::TooLarge)
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_fifo_without_blocking() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt, sync::mpsc, time::Duration};

        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("theme-fifo");
        let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);

        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = validate_custom_theme_file(&path);
            let _ = sender.send(result);
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(CustomThemeFileError::ReadFailed)
        );
    }

    #[test]
    fn atomic_export_refuses_replacement_without_force_and_preserves_source() {
        let workspace = tempfile::tempdir().unwrap();
        let destination = workspace.path().join("custom.toml");
        fs::write(&destination, b"existing").unwrap();
        let replacement = include_bytes!("../examples/custom-theme.toml");

        assert_eq!(
            write_theme(&destination, replacement, false, false),
            Err(CustomThemeFileError::AlreadyExists)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"existing");

        write_theme(&destination, replacement, true, false).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), replacement);
    }
}
