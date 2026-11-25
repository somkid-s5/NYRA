use crate::theme::{Theme, ThemeId, ThemeParseError};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
};
use tempfile::NamedTempFile;
use toml_edit::{Array, DocumentMut, Item, Table, Value, value};

const MAX_RECENT_TOOLS: usize = 10;
const MAX_SETTINGS_FILE_BYTES: u64 = 64 * 1024;
const MAX_CUSTOM_THEME_FILE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveIssue {
    NoLocation,
    InvalidFile,
    UnreadableFile,
    WriteFailed,
}

impl SaveIssue {
    pub(crate) const fn user_message(self) -> &'static str {
        match self {
            Self::NoLocation => "no writable settings location",
            Self::InvalidFile => "settings file is invalid",
            Self::UnreadableFile => "settings file could not be read",
            Self::WriteFailed => "settings could not be saved",
        }
    }
}

pub(crate) struct SettingsStore {
    path: Option<PathBuf>,
    custom_theme_path: Option<PathBuf>,
    document: DocumentMut,
    load_issue: Option<SaveIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolPreferences {
    pub(crate) favorite_tool_ids: Vec<String>,
    pub(crate) recent_tool_ids: Vec<String>,
    /// Whether NYRA should record tool IDs in the recent-tools list after a run starts.
    /// Stored as `[discovery].record_recent`; omitted legacy values default to `true`.
    pub(crate) record_recent: bool,
}

impl Default for ToolPreferences {
    fn default() -> Self {
        Self {
            favorite_tool_ids: Vec::new(),
            recent_tool_ids: Vec::new(),
            record_recent: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnimationSpeed {
    Slow,
    Normal,
    Fast,
}

impl AnimationSpeed {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Slow => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
        }
    }

    pub(crate) const fn as_config_value(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Normal => "normal",
            Self::Fast => "fast",
        }
    }

    pub(crate) fn from_config_value(value: &str) -> Option<Self> {
        match value {
            "slow" => Some(Self::Slow),
            "normal" => Some(Self::Normal),
            "fast" => Some(Self::Fast),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MotionPreferences {
    pub(crate) animations: bool,
    pub(crate) reduced_motion: bool,
    pub(crate) animation_speed: AnimationSpeed,
}

impl MotionPreferences {
    pub(crate) const fn is_animated(self) -> bool {
        self.animations && !self.reduced_motion
    }
}

impl SettingsStore {
    pub(crate) fn memory() -> Self {
        Self {
            path: None,
            custom_theme_path: None,
            document: DocumentMut::new(),
            load_issue: None,
        }
    }

    pub(crate) fn load_from_path(path: &Path) -> Self {
        match read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES) {
            Ok(contents) => match contents.parse::<DocumentMut>() {
                Ok(document) => Self {
                    path: Some(path.to_path_buf()),
                    custom_theme_path: None,
                    document,
                    load_issue: None,
                },
                Err(_) => Self {
                    path: Some(path.to_path_buf()),
                    custom_theme_path: None,
                    document: DocumentMut::new(),
                    load_issue: Some(SaveIssue::InvalidFile),
                },
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                path: Some(path.to_path_buf()),
                custom_theme_path: None,
                document: DocumentMut::new(),
                load_issue: None,
            },
            Err(_) => Self {
                path: Some(path.to_path_buf()),
                custom_theme_path: None,
                document: DocumentMut::new(),
                load_issue: Some(SaveIssue::UnreadableFile),
            },
        }
    }

    pub(crate) fn load_current() -> Self {
        let Ok(executable) = std::env::current_exe() else {
            return Self::memory();
        };
        let directories = ConfigDirectories::from_environment();
        let Some(resolved) =
            resolve_settings_path(&executable, HostPlatform::current(), &directories)
        else {
            return Self::memory();
        };
        Self::load_from_resolved(resolved)
    }

    fn load_from_resolved(resolved: ResolvedSettingsPath) -> Self {
        let mut settings = Self::load_from_path(&resolved.path);
        settings.custom_theme_path = Some(resolved.custom_theme_path);
        settings
    }

    pub(crate) fn load_custom_theme(&self) -> Result<Option<Theme>, ThemeLoadError> {
        let Some(path) = self.custom_theme_path.as_deref() else {
            return Ok(None);
        };
        let source = match read_bounded_regular_file(path, MAX_CUSTOM_THEME_FILE_BYTES) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ThemeLoadError::ReadFailed),
        };
        Theme::parse_custom(&source)
            .map(Some)
            .map_err(ThemeLoadError::Invalid)
    }

    pub(crate) fn selected_theme(&self) -> ThemeId {
        self.document
            .get("appearance")
            .and_then(Item::as_table)
            .and_then(|appearance| appearance.get("theme"))
            .and_then(Item::as_str)
            .and_then(ThemeId::from_config_value)
            .unwrap_or_default()
    }

    pub(crate) fn knowledge_favorites(&self) -> Vec<String> {
        self.document
            .get("knowledge")
            .and_then(Item::as_table)
            .and_then(|knowledge| knowledge.get("favorites"))
            .and_then(Item::as_array)
            .map(|favorites| {
                let mut unique = std::collections::HashSet::new();
                favorites
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|id| {
                        crate::find_knowledge_card(id).is_some() && unique.insert((*id).to_owned())
                    })
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn tool_preferences(&self) -> ToolPreferences {
        ToolPreferences {
            favorite_tool_ids: self.read_tool_ids("favorite_tools", usize::MAX),
            recent_tool_ids: self.read_tool_ids("recent_tools", MAX_RECENT_TOOLS),
            record_recent: self
                .document
                .get("discovery")
                .and_then(Item::as_table)
                .and_then(|discovery| discovery.get("record_recent"))
                .and_then(Item::as_bool)
                .unwrap_or(true),
        }
    }

    fn read_tool_ids(&self, key: &str, limit: usize) -> Vec<String> {
        let Some(ids) = self
            .document
            .get("discovery")
            .and_then(Item::as_table)
            .and_then(|discovery| discovery.get(key))
            .and_then(Item::as_array)
        else {
            return Vec::new();
        };
        let mut unique = std::collections::HashSet::new();
        ids.iter()
            .filter_map(Value::as_str)
            .filter(|id| crate::find_tool(id).is_some() && unique.insert((*id).to_owned()))
            .take(limit)
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn no_color(&self) -> bool {
        std::env::var_os("NO_COLOR").is_some()
            || self
                .document
                .get("appearance")
                .and_then(Item::as_table)
                .and_then(|appearance| appearance.get("no_color"))
                .and_then(Item::as_bool)
                .unwrap_or(false)
    }

    pub(crate) fn motion_preferences(&self, ssh_session: bool) -> MotionPreferences {
        let appearance = self.document.get("appearance").and_then(Item::as_table);
        let animations = appearance
            .and_then(|settings| settings.get("animations"))
            .and_then(Item::as_bool)
            .unwrap_or(!ssh_session);
        let reduced_motion = appearance
            .and_then(|settings| settings.get("reduced_motion"))
            .and_then(Item::as_bool)
            .unwrap_or(ssh_session);
        let animation_speed = appearance
            .and_then(|settings| settings.get("animation_speed"))
            .and_then(Item::as_str)
            .and_then(AnimationSpeed::from_config_value)
            .unwrap_or(AnimationSpeed::Normal);
        MotionPreferences {
            animations,
            reduced_motion,
            animation_speed,
        }
    }

    pub(crate) fn startup_issue(&self) -> Option<SaveIssue> {
        self.load_issue
    }

    pub(crate) fn save_motion_preferences(
        &mut self,
        preferences: MotionPreferences,
    ) -> Result<(), SaveIssue> {
        let Some(path) = self.path.as_deref() else {
            return Err(SaveIssue::NoLocation);
        };
        if let Some(issue) = self.load_issue {
            return Err(issue);
        }

        let mut document = match read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES) {
            Ok(contents) => contents
                .parse::<DocumentMut>()
                .map_err(|_| SaveIssue::InvalidFile)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.document.clone(),
            Err(_) => return Err(SaveIssue::UnreadableFile),
        };
        if document
            .get("appearance")
            .is_some_and(|item| !item.is_table())
        {
            return Err(SaveIssue::InvalidFile);
        }
        if document.get("appearance").is_none() {
            document["appearance"] = Item::Table(Table::new());
        }
        let Some(appearance) = document.get_mut("appearance").and_then(Item::as_table_mut) else {
            return Err(SaveIssue::InvalidFile);
        };
        appearance.insert("animations", value(preferences.animations));
        appearance.insert("reduced_motion", value(preferences.reduced_motion));
        appearance.insert(
            "animation_speed",
            value(preferences.animation_speed.as_config_value()),
        );

        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Err(SaveIssue::WriteFailed);
        };
        fs::create_dir_all(parent).map_err(|_| SaveIssue::WriteFailed)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .write_all(document.to_string().as_bytes())
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .persist(path)
            .map_err(|_| SaveIssue::WriteFailed)?;
        self.document = document;
        Ok(())
    }

    pub(crate) fn save_theme(&mut self, theme: ThemeId) -> Result<(), SaveIssue> {
        let Some(path) = self.path.as_deref() else {
            return Err(SaveIssue::NoLocation);
        };
        if let Some(issue) = self.load_issue {
            return Err(issue);
        }

        let mut document = match read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES) {
            Ok(contents) => contents
                .parse::<DocumentMut>()
                .map_err(|_| SaveIssue::InvalidFile)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.document.clone(),
            Err(_) => return Err(SaveIssue::UnreadableFile),
        };
        if document
            .get("appearance")
            .is_some_and(|item| !item.is_table())
        {
            return Err(SaveIssue::InvalidFile);
        }
        if document.get("appearance").is_none() {
            document["appearance"] = Item::Table(Table::new());
        }
        let Some(appearance) = document.get_mut("appearance").and_then(Item::as_table_mut) else {
            return Err(SaveIssue::InvalidFile);
        };
        appearance["theme"] = value(theme.as_config_value());

        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Err(SaveIssue::WriteFailed);
        };
        fs::create_dir_all(parent).map_err(|_| SaveIssue::WriteFailed)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .write_all(document.to_string().as_bytes())
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .persist(path)
            .map_err(|_| SaveIssue::WriteFailed)?;
        self.document = document;
        Ok(())
    }

    pub(crate) fn save_knowledge_favorites(
        &mut self,
        favorites: &[String],
    ) -> Result<(), SaveIssue> {
        let Some(path) = self.path.as_deref() else {
            return Err(SaveIssue::NoLocation);
        };
        if let Some(issue) = self.load_issue {
            return Err(issue);
        }
        let mut unique = std::collections::HashSet::new();
        if favorites.iter().any(|id| {
            id.is_empty() || crate::find_knowledge_card(id).is_none() || !unique.insert(id)
        }) {
            return Err(SaveIssue::InvalidFile);
        }

        let mut document = match read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES) {
            Ok(contents) => contents
                .parse::<DocumentMut>()
                .map_err(|_| SaveIssue::InvalidFile)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.document.clone(),
            Err(_) => return Err(SaveIssue::UnreadableFile),
        };
        if document
            .get("knowledge")
            .is_some_and(|item| !item.is_table())
        {
            return Err(SaveIssue::InvalidFile);
        }
        if document.get("knowledge").is_none() {
            document["knowledge"] = Item::Table(Table::new());
        }
        let Some(knowledge) = document.get_mut("knowledge").and_then(Item::as_table_mut) else {
            return Err(SaveIssue::InvalidFile);
        };
        let mut values = Array::new();
        for id in favorites {
            values.push(id.as_str());
        }
        knowledge.insert("favorites", Item::Value(Value::Array(values)));

        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Err(SaveIssue::WriteFailed);
        };
        fs::create_dir_all(parent).map_err(|_| SaveIssue::WriteFailed)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .write_all(document.to_string().as_bytes())
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .persist(path)
            .map_err(|_| SaveIssue::WriteFailed)?;
        self.document = document;
        Ok(())
    }

    pub(crate) fn save_tool_preferences(
        &mut self,
        preferences: &ToolPreferences,
    ) -> Result<(), SaveIssue> {
        let Some(path) = self.path.as_deref() else {
            return Err(SaveIssue::NoLocation);
        };
        if let Some(issue) = self.load_issue {
            return Err(issue);
        }
        if preferences.recent_tool_ids.len() > MAX_RECENT_TOOLS
            || !valid_unique_tool_ids(&preferences.favorite_tool_ids)
            || !valid_unique_tool_ids(&preferences.recent_tool_ids)
        {
            return Err(SaveIssue::InvalidFile);
        }

        let mut document = match read_bounded_regular_file(path, MAX_SETTINGS_FILE_BYTES) {
            Ok(contents) => contents
                .parse::<DocumentMut>()
                .map_err(|_| SaveIssue::InvalidFile)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.document.clone(),
            Err(_) => return Err(SaveIssue::UnreadableFile),
        };
        if document
            .get("discovery")
            .is_some_and(|item| !item.is_table())
        {
            return Err(SaveIssue::InvalidFile);
        }
        if document.get("discovery").is_none() {
            document["discovery"] = Item::Table(Table::new());
        }
        let Some(discovery) = document.get_mut("discovery").and_then(Item::as_table_mut) else {
            return Err(SaveIssue::InvalidFile);
        };
        discovery.insert(
            "favorite_tools",
            Item::Value(string_array(&preferences.favorite_tool_ids)),
        );
        discovery.insert(
            "recent_tools",
            Item::Value(string_array(&preferences.recent_tool_ids)),
        );
        discovery.insert("record_recent", value(preferences.record_recent));

        let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Err(SaveIssue::WriteFailed);
        };
        fs::create_dir_all(parent).map_err(|_| SaveIssue::WriteFailed)?;

        let mut temporary = NamedTempFile::new_in(parent).map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .write_all(document.to_string().as_bytes())
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| SaveIssue::WriteFailed)?;
        temporary
            .persist(path)
            .map_err(|_| SaveIssue::WriteFailed)?;
        self.document = document;
        Ok(())
    }
}

fn string_array(values: &[String]) -> Value {
    let mut array = Array::new();
    for value in values {
        array.push(value.as_str());
    }
    Value::Array(array)
}

fn valid_unique_tool_ids(ids: &[String]) -> bool {
    let mut unique = std::collections::HashSet::new();
    ids.iter()
        .all(|id| crate::find_tool(id).is_some() && unique.insert(id))
}

pub(crate) fn current_custom_theme_path() -> Option<PathBuf> {
    SettingsStore::load_current().custom_theme_path
}

#[allow(dead_code)] // Includes platforms used by the path resolver's cross-platform tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostPlatform {
    Windows,
    Linux,
    MacOs,
    Other,
}

impl HostPlatform {
    fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Self::Other
        }
    }
}

#[derive(Default)]
struct ConfigDirectories {
    app_data: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl ConfigDirectories {
    fn from_environment() -> Self {
        Self {
            app_data: std::env::var_os("APPDATA").map(PathBuf::from),
            local_app_data: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }
}

struct ResolvedSettingsPath {
    path: PathBuf,
    custom_theme_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeLoadError {
    ReadFailed,
    Invalid(ThemeParseError),
}

impl ThemeLoadError {
    pub(crate) const fn user_message(self) -> &'static str {
        match self {
            Self::ReadFailed => "custom theme file could not be read",
            Self::Invalid(error) => error.user_message(),
        }
    }
}

pub(crate) struct SettingsSaveResult {
    pub(crate) change: SettingsChange,
    pub(crate) result: Result<(), SaveIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SettingsChange {
    Theme(ThemeId),
    Motion(MotionPreferences),
    KnowledgeFavorites(Vec<String>),
    ToolPreferences(ToolPreferences),
}

pub(crate) struct SettingsWriter {
    sender: SyncSender<SettingsChange>,
    results: Receiver<SettingsSaveResult>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsWriter {
    pub(crate) fn start(mut store: SettingsStore) -> Self {
        let (sender, requests) = mpsc::sync_channel::<SettingsChange>(1);
        let (result_sender, results) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("nyra-settings-writer".into())
            .spawn(move || {
                while let Ok(change) = requests.recv() {
                    let result = match &change {
                        SettingsChange::Theme(theme) => store.save_theme(*theme),
                        SettingsChange::Motion(preferences) => {
                            store.save_motion_preferences(*preferences)
                        }
                        SettingsChange::KnowledgeFavorites(favorites) => {
                            store.save_knowledge_favorites(favorites)
                        }
                        SettingsChange::ToolPreferences(preferences) => {
                            store.save_tool_preferences(preferences)
                        }
                    };
                    let _ = result_sender.send(SettingsSaveResult { change, result });
                }
            })
            .ok();
        Self {
            sender,
            results,
            worker,
        }
    }

    pub(crate) fn try_save(
        &self,
        change: SettingsChange,
    ) -> Result<(), TrySendError<SettingsChange>> {
        self.sender.try_send(change)
    }

    pub(crate) fn try_result(&self) -> Option<SettingsSaveResult> {
        self.results.try_recv().ok()
    }

    pub(crate) fn finish(self, pending_changes: impl IntoIterator<Item = SettingsChange>) {
        let Self {
            sender,
            results: _,
            worker,
        } = self;
        for change in pending_changes {
            if sender.send(change).is_err() {
                break;
            }
        }
        drop(sender);
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }
}

fn resolve_settings_path(
    executable: &Path,
    platform: HostPlatform,
    directories: &ConfigDirectories,
) -> Option<ResolvedSettingsPath> {
    let executable_dir = executable.parent()?;
    if !matches!(marker_status(executable_dir), Ok(false)) {
        return Some(ResolvedSettingsPath {
            path: portable_config_path(executable_dir),
            custom_theme_path: executable_dir.join("themes").join("custom.toml"),
        });
    }

    let workspace_root = executable_dir.parent()?;
    if !matches!(marker_status(workspace_root), Ok(false)) {
        return Some(ResolvedSettingsPath {
            path: portable_config_path(workspace_root),
            custom_theme_path: workspace_root.join("themes").join("custom.toml"),
        });
    }

    let config_root = match platform {
        HostPlatform::Windows => directories
            .app_data
            .as_ref()
            .filter(|path| path.is_absolute())
            .or_else(|| {
                directories
                    .local_app_data
                    .as_ref()
                    .filter(|path| path.is_absolute())
            })
            .map(|path| path.join("NYRA")),
        HostPlatform::Linux => directories
            .xdg_config_home
            .as_ref()
            .filter(|path| path.is_absolute())
            .cloned()
            .or_else(|| {
                directories
                    .home
                    .as_ref()
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".config"))
            })
            .map(|path| path.join("nyra")),
        HostPlatform::MacOs => directories
            .home
            .as_ref()
            .filter(|path| path.is_absolute())
            .map(|path| {
                path.join("Library")
                    .join("Application Support")
                    .join("NYRA")
            }),
        HostPlatform::Other => None,
    }?;

    Some(ResolvedSettingsPath {
        path: config_root.join("config.toml"),
        custom_theme_path: config_root.join("themes").join("custom.toml"),
    })
}

fn portable_config_path(root: &Path) -> PathBuf {
    root.join("config").join("config.toml")
}

fn read_bounded_regular_file(path: &Path, maximum_bytes: u64) -> io::Result<String> {
    let path_metadata = fs::metadata(path)?;
    if !path_metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration source is not a regular file",
        ));
    }

    let mut options = fs::OpenOptions::new();
    options.read(true);
    configure_nonblocking_file_open(&mut options);
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration source is not a regular file",
        ));
    }

    let mut source = String::new();
    file.take(maximum_bytes.saturating_add(1))
        .read_to_string(&mut source)?;
    if source.len() as u64 > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "configuration source exceeds the size limit",
        ));
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

fn marker_status(root: &Path) -> Result<bool, io::Error> {
    match fs::metadata(root.join("portable.flag")) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn changing_theme_preserves_other_settings_and_reloads() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "# retained by NYRA\n[appearance]\ntheme = \"neo-tokyo\"\n\n[search]\nfuzzy = true\n",
        )
        .unwrap();

        let mut settings = SettingsStore::load_from_path(&path);
        assert_eq!(settings.selected_theme(), ThemeId::NeoTokyo);

        settings.save_theme(ThemeId::AcidGrid).unwrap();

        let reloaded = SettingsStore::load_from_path(&path);
        assert_eq!(reloaded.selected_theme(), ThemeId::AcidGrid);
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# retained by NYRA"));
        assert!(contents.contains("[search]\nfuzzy = true"));
    }

    #[test]
    fn knowledge_favorites_preserve_other_settings_and_ignore_unknown_ids() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "# keep this comment\n[appearance]\ntheme = \"neo-tokyo\"\n[search]\nfuzzy = true\n[knowledge]\nfavorites = [\"unknown.card\"]\n",
        )
        .unwrap();
        let mut settings = SettingsStore::load_from_path(&path);
        assert!(settings.knowledge_favorites().is_empty());

        settings
            .save_knowledge_favorites(&[
                "windows.dns-lookup".into(),
                "linux.tcp-connectivity".into(),
            ])
            .unwrap();

        let reloaded = SettingsStore::load_from_path(&path);
        assert_eq!(
            reloaded.knowledge_favorites(),
            ["windows.dns-lookup", "linux.tcp-connectivity"]
        );
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("# keep this comment"));
        assert!(contents.contains("[search]\nfuzzy = true"));
    }

    #[test]
    fn legacy_tool_config_defaults_recent_tracking_to_true() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[discovery]\nfavorite_tools = [\"network.tcp\"]\nrecent_tools = [\"system.disk\"]\n",
        )
        .unwrap();

        let preferences = SettingsStore::load_from_path(&path).tool_preferences();

        assert!(preferences.record_recent);
        assert!(ToolPreferences::default().record_recent);
    }

    #[test]
    fn recent_tracking_setting_persists_and_preserves_other_values() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\ntheme = \"neo-tokyo\"\n[search]\nfuzzy = false\n[discovery]\nrecord_recent = false\nfavorite_tools = [\"network.tcp\"]\nrecent_tools = [\"system.disk\"]\ncustom_setting = \"keep\"\n",
        )
        .unwrap();
        let mut settings = SettingsStore::load_from_path(&path);
        let mut preferences = settings.tool_preferences();
        preferences.favorite_tool_ids.push("network.dns".into());
        assert!(!preferences.record_recent);

        settings.save_tool_preferences(&preferences).unwrap();

        let reloaded = SettingsStore::load_from_path(&path);
        assert_eq!(reloaded.tool_preferences(), preferences);
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("record_recent = false"));
        assert!(contents.contains("custom_setting = \"keep\""));
        assert!(contents.contains("[search]\nfuzzy = false"));
        assert_eq!(reloaded.selected_theme(), ThemeId::NeoTokyo);
    }

    #[test]
    fn tool_preferences_validate_ids_deduplicate_and_preserve_other_settings() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\ntheme = \"neo-tokyo\"\n[search]\nfuzzy = false\n[discovery]\nfavorite_tools = [\"network.tcp\", \"unknown.tool\", \"network.tcp\"]\nrecent_tools = [\"system.disk\", \"network.tcp\", \"system.disk\"]\n",
        )
        .unwrap();
        let mut settings = SettingsStore::load_from_path(&path);
        assert_eq!(
            settings.tool_preferences(),
            ToolPreferences {
                favorite_tool_ids: vec!["network.tcp".into()],
                recent_tool_ids: vec!["system.disk".into(), "network.tcp".into()],
                record_recent: true,
            }
        );

        let preferences = ToolPreferences {
            favorite_tool_ids: vec!["network.dns".into()],
            recent_tool_ids: vec!["file.sha256".into(), "network.tcp".into()],
            record_recent: true,
        };
        settings.save_tool_preferences(&preferences).unwrap();

        let reloaded = SettingsStore::load_from_path(&path);
        assert_eq!(reloaded.tool_preferences(), preferences);
        assert_eq!(reloaded.selected_theme(), ThemeId::NeoTokyo);
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains("[search]\nfuzzy = false")
        );
    }

    #[test]
    fn no_color_appearance_setting_is_read_from_toml() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(&path, "[appearance]\nno_color = true\n").unwrap();

        assert!(SettingsStore::load_from_path(&path).no_color());
    }

    #[test]
    fn motion_defaults_to_animated_locally_and_static_over_ssh() {
        let settings = SettingsStore::memory();
        let local = settings.motion_preferences(false);
        let ssh = settings.motion_preferences(true);

        assert!(local.is_animated());
        assert_eq!(local.animation_speed, AnimationSpeed::Normal);
        assert!(!ssh.is_animated());
        assert_eq!(ssh.animation_speed, AnimationSpeed::Normal);
    }

    #[test]
    fn explicit_motion_preferences_can_disable_or_reduce_animation() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\nanimations = true\nreduced_motion = true\n",
        )
        .unwrap();
        assert!(
            !SettingsStore::load_from_path(&path)
                .motion_preferences(false)
                .is_animated()
        );

        fs::write(
            &path,
            "[appearance]\nanimations = false\nreduced_motion = false\n",
        )
        .unwrap();
        assert!(
            !SettingsStore::load_from_path(&path)
                .motion_preferences(false)
                .is_animated()
        );

        fs::write(
            &path,
            "[appearance]\nanimations = true\nreduced_motion = false\n",
        )
        .unwrap();
        let explicit = SettingsStore::load_from_path(&path);
        assert!(explicit.motion_preferences(false).is_animated());
        assert!(explicit.motion_preferences(true).is_animated());
    }

    #[test]
    fn invalid_motion_types_fall_back_to_session_defaults() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\nanimations = \"sometimes\"\nreduced_motion = \"maybe\"\nanimation_speed = \"warp\"\n",
        )
        .unwrap();
        let settings = SettingsStore::load_from_path(&path);

        let local = settings.motion_preferences(false);
        let ssh = settings.motion_preferences(true);
        assert!(local.is_animated());
        assert!(!ssh.is_animated());
        assert_eq!(local.animation_speed, AnimationSpeed::Normal);
        assert_eq!(ssh.animation_speed, AnimationSpeed::Normal);
    }

    #[test]
    fn motion_preferences_save_reload_and_preserve_config_edits() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\ntheme = \"neo-tokyo\"\n[search]\nfuzzy = true\n",
        )
        .unwrap();
        let mut settings = SettingsStore::load_from_path(&path);
        fs::write(
            &path,
            "# edited while NYRA was open\n[appearance]\ntheme = \"acid-grid\"\nno_color = true\n[search]\nfuzzy = false\n",
        )
        .unwrap();
        let preferences = MotionPreferences {
            animations: true,
            reduced_motion: false,
            animation_speed: AnimationSpeed::Fast,
        };

        settings.save_motion_preferences(preferences).unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# edited while NYRA was open"));
        assert!(saved.contains("theme = \"acid-grid\""));
        assert!(saved.contains("no_color = true"));
        assert!(saved.contains("fuzzy = false"));
        assert_eq!(
            SettingsStore::load_from_path(&path).motion_preferences(false),
            preferences
        );
    }

    #[test]
    fn theme_save_keeps_config_edits_made_after_startup() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "[appearance]\ntheme = \"neo-tokyo\"\n[search]\nfuzzy = true\n",
        )
        .unwrap();
        let mut settings = SettingsStore::load_from_path(&path);
        fs::write(
            &path,
            "# edited while NYRA was open\n[appearance]\ntheme = \"neo-tokyo\"\nanimations = false\n[search]\nfuzzy = false\n",
        )
        .unwrap();

        settings.save_theme(ThemeId::AcidGrid).unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# edited while NYRA was open"));
        assert!(saved.contains("animations = false"));
        assert!(saved.contains("fuzzy = false"));
        assert_eq!(
            SettingsStore::load_from_path(&path).selected_theme(),
            ThemeId::AcidGrid
        );
    }

    #[test]
    fn portable_workspace_path_precedes_the_user_config_directory() {
        let workspace = tempfile::tempdir().unwrap();
        let binary_dir = workspace.path().join("windows");
        fs::create_dir_all(&binary_dir).unwrap();
        fs::write(workspace.path().join("portable.flag"), "").unwrap();
        let executable = binary_dir.join("nyra.exe");
        let directories = ConfigDirectories {
            app_data: Some(workspace.path().join("host-config")),
            local_app_data: None,
            xdg_config_home: None,
            home: None,
        };

        let resolved = resolve_settings_path(&executable, HostPlatform::Windows, &directories)
            .expect("portable workspace has a settings path");

        assert_eq!(
            resolved.path,
            workspace.path().join("config").join("config.toml")
        );
        assert_eq!(
            resolved.custom_theme_path,
            workspace.path().join("themes").join("custom.toml")
        );
    }

    #[test]
    fn custom_theme_loads_from_the_resolved_theme_directory() {
        let workspace = tempfile::tempdir().unwrap();
        let config_path = workspace.path().join("config").join("config.toml");
        let theme_path = workspace.path().join("themes").join("custom.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::create_dir_all(theme_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "[appearance]\ntheme = \"custom\"\n").unwrap();
        fs::write(&theme_path, include_str!("../examples/custom-theme.toml")).unwrap();
        let settings = SettingsStore::load_from_resolved(ResolvedSettingsPath {
            path: config_path,
            custom_theme_path: theme_path,
        });

        assert_eq!(settings.selected_theme(), ThemeId::Custom);
        assert_eq!(
            settings.load_custom_theme().unwrap().unwrap().name(),
            "Midnight Circuit"
        );
    }

    #[test]
    fn missing_custom_theme_is_reported_as_unavailable_without_failing_settings() {
        let workspace = tempfile::tempdir().unwrap();
        let settings = SettingsStore::load_from_resolved(ResolvedSettingsPath {
            path: workspace.path().join("config.toml"),
            custom_theme_path: workspace.path().join("themes").join("custom.toml"),
        });

        assert_eq!(settings.selected_theme(), ThemeId::CyberCyan);
        assert!(settings.load_custom_theme().unwrap().is_none());
    }

    #[test]
    fn settings_writer_flushes_motion_and_other_preferences_before_shutdown() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config").join("config.toml");
        let writer = SettingsWriter::start(SettingsStore::load_from_path(&path));

        writer.finish([
            SettingsChange::Theme(ThemeId::CrimsonNoir),
            SettingsChange::Motion(MotionPreferences {
                animations: false,
                reduced_motion: true,
                animation_speed: AnimationSpeed::Slow,
            }),
            SettingsChange::KnowledgeFavorites(vec!["windows.dns-lookup".into()]),
            SettingsChange::ToolPreferences(ToolPreferences {
                favorite_tool_ids: vec!["network.tcp".into()],
                recent_tool_ids: vec!["system.disk".into()],
                record_recent: true,
            }),
        ]);

        let settings = SettingsStore::load_from_path(&path);
        assert_eq!(settings.selected_theme(), ThemeId::CrimsonNoir);
        assert_eq!(
            settings.motion_preferences(false),
            MotionPreferences {
                animations: false,
                reduced_motion: true,
                animation_speed: AnimationSpeed::Slow,
            }
        );
        assert_eq!(settings.knowledge_favorites(), ["windows.dns-lookup"]);
        assert_eq!(
            settings.tool_preferences(),
            ToolPreferences {
                favorite_tool_ids: vec!["network.tcp".into()],
                recent_tool_ids: vec!["system.disk".into()],
                record_recent: true,
            }
        );
    }

    #[test]
    fn invalid_toml_uses_default_theme_without_overwriting_the_file() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        let invalid = b"[appearance\ntheme = \"neo-tokyo\"\n";
        fs::write(&path, invalid).unwrap();
        let mut settings = SettingsStore::load_from_path(&path);

        assert_eq!(settings.selected_theme(), ThemeId::CyberCyan);
        assert_eq!(settings.startup_issue(), Some(SaveIssue::InvalidFile));
        assert_eq!(
            settings.save_theme(ThemeId::AcidGrid),
            Err(SaveIssue::InvalidFile)
        );
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }

    #[test]
    fn oversized_settings_file_is_rejected_without_overwriting_it() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        let original = vec![b' '; (MAX_SETTINGS_FILE_BYTES + 1) as usize];
        fs::write(&path, &original).unwrap();
        let mut settings = SettingsStore::load_from_path(&path);

        assert_eq!(settings.startup_issue(), Some(SaveIssue::UnreadableFile));
        assert_eq!(
            settings.save_theme(ThemeId::AcidGrid),
            Err(SaveIssue::UnreadableFile)
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn oversized_custom_theme_is_rejected_during_load() {
        let workspace = tempfile::tempdir().unwrap();
        let theme_path = workspace.path().join("themes").join("custom.toml");
        fs::create_dir_all(theme_path.parent().unwrap()).unwrap();
        fs::write(
            &theme_path,
            vec![b' '; (MAX_CUSTOM_THEME_FILE_BYTES + 1) as usize],
        )
        .unwrap();
        let settings = SettingsStore::load_from_resolved(ResolvedSettingsPath {
            path: workspace.path().join("config.toml"),
            custom_theme_path: theme_path,
        });

        assert_eq!(
            settings.load_custom_theme(),
            Err(ThemeLoadError::ReadFailed)
        );
    }

    #[cfg(unix)]
    #[test]
    fn settings_and_custom_theme_fifos_are_rejected_without_blocking() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt, sync::mpsc, time::Duration};

        let workspace = tempfile::tempdir().unwrap();
        let settings_path = workspace.path().join("config-fifo");
        let theme_path = workspace.path().join("custom-fifo");
        for path in [&settings_path, &theme_path] {
            let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        }

        let (sender, receiver) = mpsc::channel();
        let path = settings_path.clone();
        std::thread::spawn(move || {
            let issue = SettingsStore::load_from_path(&path).startup_issue();
            let _ = sender.send(issue);
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Some(SaveIssue::UnreadableFile)
        );

        let (sender, receiver) = mpsc::channel();
        let resolved = ResolvedSettingsPath {
            path: workspace.path().join("missing-config.toml"),
            custom_theme_path: theme_path.clone(),
        };
        std::thread::spawn(move || {
            let settings = SettingsStore::load_from_resolved(resolved);
            let result = settings.load_custom_theme().map(|_| ());
            let _ = sender.send(result);
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(ThemeLoadError::ReadFailed)
        );
    }

    #[test]
    fn unreadable_portable_config_never_falls_back_to_host_config() {
        let workspace = tempfile::tempdir().unwrap();
        let binary_dir = workspace.path().join("windows");
        fs::create_dir_all(&binary_dir).unwrap();
        fs::write(workspace.path().join("portable.flag"), "").unwrap();
        fs::write(workspace.path().join("config"), "blocks config directory").unwrap();

        let host_config_root = workspace.path().join("host-config");
        let host_nyra_dir = host_config_root.join("NYRA");
        fs::create_dir_all(&host_nyra_dir).unwrap();
        let host_config = host_nyra_dir.join("config.toml");
        fs::write(&host_config, "[appearance]\ntheme = \"neo-tokyo\"\n").unwrap();
        let directories = ConfigDirectories {
            app_data: Some(host_config_root),
            local_app_data: None,
            xdg_config_home: None,
            home: None,
        };
        let executable = binary_dir.join("nyra.exe");
        let resolved = resolve_settings_path(&executable, HostPlatform::Windows, &directories)
            .expect("portable workspace has a settings path");
        let mut settings = SettingsStore::load_from_path(&resolved.path);

        assert!(matches!(
            settings.save_theme(ThemeId::AcidGrid),
            Err(SaveIssue::UnreadableFile | SaveIssue::WriteFailed)
        ));
        assert_eq!(
            fs::read_to_string(host_config).unwrap(),
            "[appearance]\ntheme = \"neo-tokyo\"\n"
        );
    }

    #[test]
    fn standard_config_paths_follow_platform_environment_contracts() {
        let workspace = tempfile::tempdir().unwrap();
        let executable = workspace.path().join("bin").join("nyra");
        let directories = ConfigDirectories {
            app_data: Some(workspace.path().join("appdata")),
            local_app_data: Some(workspace.path().join("localappdata")),
            xdg_config_home: Some(workspace.path().join("xdg")),
            home: Some(workspace.path().join("home")),
        };

        let windows = resolve_settings_path(&executable, HostPlatform::Windows, &directories)
            .expect("APPDATA gives Windows a settings path");
        assert_eq!(
            windows.path,
            workspace
                .path()
                .join("appdata")
                .join("NYRA")
                .join("config.toml")
        );

        let linux = resolve_settings_path(&executable, HostPlatform::Linux, &directories)
            .expect("XDG_CONFIG_HOME gives Linux a settings path");
        assert_eq!(
            linux.path,
            workspace
                .path()
                .join("xdg")
                .join("nyra")
                .join("config.toml")
        );
    }
}
