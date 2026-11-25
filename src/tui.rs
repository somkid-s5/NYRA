use crate::{
    CATALOG, CsvReportDestination, HtmlReportDestination, InputError, JsonReportDestination,
    KNOWLEDGE_CARDS, KnowledgeCard, ReportExportError, RunError, RunResult, ToolCategory,
    ToolInput, ToolMetadata, execute, parse_tool_input, rank_knowledge_cards,
    settings::{
        AnimationSpeed, MotionPreferences, SettingsChange, SettingsSaveResult, SettingsStore,
        SettingsWriter, ToolPreferences,
    },
    theme::{BUILTIN_THEMES, Theme, ThemeId},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    cell::Cell,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::{runtime::Builder, sync::oneshot};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Tools,
    Form,
    Running,
    Result,
    ReportExportFormat,
    ReportExportPath,
    Knowledge,
    SavedTools,
    Appearance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavedToolsView {
    Favorites,
    Recent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReportFormat {
    Json,
    Html,
    Csv,
}

impl ReportFormat {
    const fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Html => "HTML",
            Self::Csv => "CSV",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Json => ".json",
            Self::Html => ".html",
            Self::Csv => ".csv",
        }
    }
}

const CATEGORIES: &[Option<ToolCategory>] = &[
    None,
    Some(ToolCategory::Network),
    Some(ToolCategory::System),
    Some(ToolCategory::Security),
    Some(ToolCategory::File),
];
const MAX_SEARCH_CHARS: usize = 128;
const RUNNING_INDICATOR_INTERVAL: Duration = Duration::from_millis(150);
const RUNNING_INDICATOR_SLOW_INTERVAL: Duration = Duration::from_millis(300);
const RUNNING_INDICATOR_FAST_INTERVAL: Duration = Duration::from_millis(75);
const RUNNING_INDICATOR_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
const STARTUP_ANIMATION_FRAMES: [&str; 3] = ["|", "/", "-"];
const MAX_REPORT_PATH_BYTES: usize = 4096;

#[derive(Default)]
enum Completion {
    #[default]
    None,
    Result(Box<RunResult>),
    Error(String),
}

enum Action {
    None,
    Start,
    Cancel,
    Copy(String),
    ExportReport { format: ReportFormat, path: PathBuf },
    Quit,
}

fn pop_last_grapheme(value: &mut String) {
    if let Some((start, _)) = value.grapheme_indices(true).next_back() {
        value.truncate(start);
    }
}

fn visible_query(prefix: &str, query: &str, panel_width: u16) -> String {
    let available_width = usize::from(panel_width.saturating_sub(2));
    let prefix_width = UnicodeWidthStr::width(prefix);
    let query_width = UnicodeWidthStr::width(query);
    if prefix_width >= available_width || query_width <= available_width - prefix_width {
        return format!("{prefix}{query}");
    }

    let suffix_width = available_width.saturating_sub(prefix_width + 1);
    let mut suffix = Vec::new();
    let mut used_width = 0;
    for grapheme in query.graphemes(true).rev() {
        let width = UnicodeWidthStr::width(grapheme);
        if used_width + width > suffix_width {
            break;
        }
        suffix.push(grapheme);
        used_width += width;
    }
    suffix.reverse();
    format!("{prefix}…{}", suffix.concat())
}

struct App {
    screen: Screen,
    query: String,
    selection: usize,
    category_index: usize,
    selected_tool_id: Option<&'static str>,
    input_values: Vec<String>,
    active_field: usize,
    themes: Vec<Theme>,
    theme_index: usize,
    appearance_selection: usize,
    appearance_return_screen: Screen,
    fallback_theme: Option<Theme>,
    no_color: bool,
    pending_theme: Option<ThemeId>,
    detail_scroll: u16,
    detail_scroll_limit: Cell<u16>,
    result_scroll: u16,
    report_formats: Vec<ReportFormat>,
    report_format_index: usize,
    report_path_format: Option<ReportFormat>,
    report_path: String,
    knowledge_query: String,
    knowledge_selection: usize,
    knowledge_scroll: u16,
    knowledge_scroll_limit: Cell<u16>,
    knowledge_return_screen: Screen,
    knowledge_favorites: Vec<String>,
    pending_knowledge_favorites: Option<Vec<String>>,
    knowledge_favorites_only: bool,
    favorite_tool_ids: Vec<String>,
    recent_tool_ids: Vec<String>,
    record_recent: bool,
    pending_tool_preferences: Option<ToolPreferences>,
    saved_tools_view: SavedToolsView,
    saved_selection: usize,
    saved_return_screen: Screen,
    motion_preferences: MotionPreferences,
    pending_motion_preferences: Option<MotionPreferences>,
    running_indicator_frame: usize,
    next_running_indicator_tick: Option<Instant>,
    startup_animation_frame: usize,
    next_startup_animation_tick: Option<Instant>,
    startup_animation_deadline: Option<Instant>,
    form_return_screen: Screen,
    search_return_screen: Option<Screen>,
    status: String,
    completion: Completion,
}

impl App {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_themes(ThemeId::default(), None)
    }

    fn with_themes(theme_id: ThemeId, custom_theme: Option<Theme>) -> Self {
        let mut themes = BUILTIN_THEMES.to_vec();
        if let Some(custom_theme) = custom_theme {
            themes.push(custom_theme);
        }
        let selected_theme = if theme_id == ThemeId::Custom && themes.len() == BUILTIN_THEMES.len()
        {
            ThemeId::default()
        } else {
            theme_id
        };
        let theme_index = themes
            .iter()
            .position(|theme| theme.id == selected_theme)
            .unwrap_or_default();
        Self {
            screen: Screen::Tools,
            query: String::new(),
            selection: 0,
            category_index: 0,
            selected_tool_id: None,
            input_values: Vec::new(),
            active_field: 0,
            themes,
            theme_index,
            appearance_selection: theme_index,
            appearance_return_screen: Screen::Tools,
            fallback_theme: None,
            no_color: false,
            pending_theme: None,
            detail_scroll: 0,
            detail_scroll_limit: Cell::new(0),
            result_scroll: 0,
            report_formats: Vec::new(),
            report_format_index: 0,
            report_path_format: None,
            report_path: String::new(),
            knowledge_query: String::new(),
            knowledge_selection: 0,
            knowledge_scroll: 0,
            knowledge_scroll_limit: Cell::new(0),
            knowledge_return_screen: Screen::Tools,
            knowledge_favorites: Vec::new(),
            pending_knowledge_favorites: None,
            knowledge_favorites_only: false,
            favorite_tool_ids: Vec::new(),
            recent_tool_ids: Vec::new(),
            record_recent: true,
            pending_tool_preferences: None,
            saved_tools_view: SavedToolsView::Favorites,
            saved_selection: 0,
            saved_return_screen: Screen::Tools,
            motion_preferences: MotionPreferences {
                animations: true,
                reduced_motion: false,
                animation_speed: AnimationSpeed::Normal,
            },
            pending_motion_preferences: None,
            running_indicator_frame: 0,
            next_running_indicator_tick: None,
            startup_animation_frame: 0,
            next_startup_animation_tick: None,
            startup_animation_deadline: None,
            form_return_screen: Screen::Tools,
            search_return_screen: None,
            status: String::new(),
            completion: Completion::None,
        }
    }

    fn with_fallback(
        theme_id: ThemeId,
        custom_theme: Option<Theme>,
        fallback_theme: Option<Theme>,
    ) -> Self {
        let mut app = Self::with_themes(theme_id, custom_theme);
        app.fallback_theme = fallback_theme;
        app
    }

    fn visible_tools(&self) -> Vec<&'static ToolMetadata> {
        let query = self.query.trim().to_lowercase();
        let mut tools = CATALOG
            .iter()
            .filter(|tool| {
                self.current_category()
                    .is_none_or(|category| tool.category == category)
            })
            .filter_map(|tool| score_tool(tool, &query).map(|score| (score, tool)))
            .collect::<Vec<_>>();
        tools.sort_by_key(|(score, _)| *score);
        tools.into_iter().map(|(_, tool)| tool).collect()
    }

    fn visible_knowledge_cards(&self) -> Vec<&'static KnowledgeCard> {
        rank_knowledge_cards(&self.knowledge_query)
            .into_iter()
            .filter(|card| {
                !self.knowledge_favorites_only
                    || self.knowledge_favorites.iter().any(|id| id == card.id)
            })
            .collect()
    }

    fn selected_knowledge_card(&self) -> Option<&'static KnowledgeCard> {
        self.visible_knowledge_cards()
            .get(self.knowledge_selection)
            .copied()
    }

    fn visible_saved_tools(&self) -> Vec<&'static ToolMetadata> {
        let ids = match self.saved_tools_view {
            SavedToolsView::Favorites => &self.favorite_tool_ids,
            SavedToolsView::Recent => &self.recent_tool_ids,
        };
        ids.iter().filter_map(|id| crate::find_tool(id)).collect()
    }

    fn current_tool(&self) -> Option<&'static ToolMetadata> {
        self.selected_tool_id.and_then(crate::find_tool)
    }

    fn start_running_indicator_at(&mut self, now: Instant) {
        self.running_indicator_frame = 0;
        self.next_running_indicator_tick = self
            .motion_preferences
            .is_animated()
            .then_some(now + self.running_indicator_interval());
    }

    fn advance_running_indicator_at(&mut self, now: Instant) {
        if self.screen != Screen::Running {
            self.next_running_indicator_tick = None;
            return;
        }
        if !self.motion_preferences.is_animated() {
            self.next_running_indicator_tick = None;
            return;
        }
        let interval = self.running_indicator_interval();
        let Some(next_tick) = self.next_running_indicator_tick else {
            self.next_running_indicator_tick = Some(now + interval);
            return;
        };
        if now >= next_tick {
            self.running_indicator_frame =
                (self.running_indicator_frame + 1) % RUNNING_INDICATOR_FRAMES.len();
            self.next_running_indicator_tick = Some(now + interval);
        }
    }

    fn running_indicator_interval(&self) -> Duration {
        match self.motion_preferences.animation_speed {
            AnimationSpeed::Slow => RUNNING_INDICATOR_SLOW_INTERVAL,
            AnimationSpeed::Normal => RUNNING_INDICATOR_INTERVAL,
            AnimationSpeed::Fast => RUNNING_INDICATOR_FAST_INTERVAL,
        }
    }

    fn running_indicator_label(&self) -> String {
        if self.motion_preferences.is_animated() {
            format!(
                "Working {}",
                RUNNING_INDICATOR_FRAMES[self.running_indicator_frame]
            )
        } else {
            "Working".into()
        }
    }

    fn start_startup_animation_at(&mut self, now: Instant) {
        if !self.motion_preferences.is_animated() {
            return;
        }
        let interval = self.running_indicator_interval();
        self.startup_animation_frame = 0;
        self.next_startup_animation_tick = Some(now + interval);
        self.startup_animation_deadline = Some(now + interval + interval + interval);
    }

    fn advance_startup_animation_at(&mut self, now: Instant) {
        let (Some(next_tick), Some(deadline)) = (
            self.next_startup_animation_tick,
            self.startup_animation_deadline,
        ) else {
            return;
        };
        if now >= deadline {
            self.next_startup_animation_tick = None;
            self.startup_animation_deadline = None;
            return;
        }
        if now >= next_tick {
            self.startup_animation_frame =
                (self.startup_animation_frame + 1) % STARTUP_ANIMATION_FRAMES.len();
            self.next_startup_animation_tick =
                Some((now + self.running_indicator_interval()).min(deadline));
        }
    }

    fn startup_animation_frame(&self) -> Option<usize> {
        self.next_startup_animation_tick
            .map(|_| self.startup_animation_frame)
    }

    fn skip_startup_animation(&mut self) {
        self.next_startup_animation_tick = None;
        self.startup_animation_deadline = None;
    }

    fn set_motion_preferences(&mut self, preferences: MotionPreferences, now: Instant) {
        if preferences == self.motion_preferences {
            return;
        }
        self.motion_preferences = preferences;
        self.pending_motion_preferences = Some(preferences);
        if self.screen == Screen::Running {
            self.running_indicator_frame = 0;
            self.next_running_indicator_tick = preferences
                .is_animated()
                .then_some(now + self.running_indicator_interval());
        }
        self.status = if preferences.is_animated() {
            format!(
                "Motion enabled at {} speed (saving...).",
                animation_speed_label(preferences.animation_speed)
            )
        } else {
            "Running indicator set to static (saving...).".into()
        };
    }

    fn theme(&self) -> &Theme {
        self.fallback_theme
            .as_ref()
            .unwrap_or(&self.themes[self.theme_index])
    }

    fn show_settings_save_result(&mut self, result: SettingsSaveResult) {
        match result.change {
            SettingsChange::Theme(theme) => {
                if theme != self.theme().id || self.pending_theme.is_some() {
                    return;
                }
                self.status = match result.result {
                    Ok(()) => format!("Theme: {} (saved)", self.theme().name()),
                    Err(issue) => format!(
                        "Theme: {} (session only; {})",
                        self.theme().name(),
                        issue.user_message()
                    ),
                };
            }
            SettingsChange::Motion(preferences) => {
                if preferences != self.motion_preferences
                    || self.pending_motion_preferences.is_some()
                {
                    return;
                }
                self.status = match result.result {
                    Ok(()) => "Motion preferences saved.".into(),
                    Err(issue) => format!(
                        "Motion preferences are session only; {}.",
                        issue.user_message()
                    ),
                };
            }
            SettingsChange::KnowledgeFavorites(favorites) => {
                if favorites != self.knowledge_favorites
                    || self.pending_knowledge_favorites.is_some()
                {
                    return;
                }
                self.status = match result.result {
                    Ok(()) => "Knowledge favorites saved.".into(),
                    Err(issue) => format!(
                        "Knowledge favorites are session only; {}.",
                        issue.user_message()
                    ),
                };
            }
            SettingsChange::ToolPreferences(preferences) => {
                if preferences.favorite_tool_ids != self.favorite_tool_ids
                    || preferences.recent_tool_ids != self.recent_tool_ids
                    || preferences.record_recent != self.record_recent
                    || self.pending_tool_preferences.is_some()
                {
                    return;
                }
                self.status = match result.result {
                    Ok(()) => "Discovery preferences saved.".into(),
                    Err(issue) => format!(
                        "Discovery preferences are session only; {}.",
                        issue.user_message()
                    ),
                };
            }
        }
    }

    fn current_category(&self) -> Option<ToolCategory> {
        CATEGORIES[self.category_index]
    }

    fn set_tool_selection(&mut self, selection: usize) {
        self.selection = selection;
        self.detail_scroll = 0;
    }

    fn open_knowledge(&mut self) {
        self.knowledge_return_screen = self.screen;
        self.knowledge_query = if self.screen == Screen::Tools {
            self.query.clone()
        } else {
            String::new()
        };
        self.knowledge_selection = 0;
        self.knowledge_scroll = 0;
        self.knowledge_favorites_only = false;
        self.screen = Screen::Knowledge;
        self.status = if self.knowledge_query.is_empty() {
            "Local curated command references; NYRA does not run these commands.".into()
        } else {
            "Local curated search; references are displayed, never executed.".into()
        };
    }

    fn open_global_search(&mut self) {
        if self.screen != Screen::Tools {
            self.search_return_screen = Some(self.screen);
        } else {
            self.search_return_screen = None;
        }
        self.screen = Screen::Tools;
        self.query.clear();
        self.category_index = 0;
        self.set_tool_selection(0);
        self.status = "Global tool search. Type a task or curated English alias.".into();
    }

    fn set_knowledge_selection(&mut self, selection: usize) {
        self.knowledge_selection = selection;
        self.knowledge_scroll = 0;
    }

    fn toggle_knowledge_favorite(&mut self) {
        let Some(card) = self.selected_knowledge_card() else {
            self.status = "Select a knowledge card first.".into();
            return;
        };
        if let Some(index) = self
            .knowledge_favorites
            .iter()
            .position(|favorite| favorite == card.id)
        {
            self.knowledge_favorites.remove(index);
            self.status = format!("Removed {} from favorites (saving...).", card.id);
        } else {
            self.knowledge_favorites.push(card.id.to_owned());
            self.knowledge_favorites.sort_by_key(|favorite| {
                KNOWLEDGE_CARDS
                    .iter()
                    .position(|candidate| candidate.id == favorite)
                    .unwrap_or(usize::MAX)
            });
            self.status = format!("Added {} to favorites (saving...).", card.id);
        }
        self.pending_knowledge_favorites = Some(self.knowledge_favorites.clone());
    }

    fn open_saved_tools(&mut self) {
        self.saved_return_screen = self.screen;
        self.saved_tools_view =
            if self.favorite_tool_ids.is_empty() && !self.recent_tool_ids.is_empty() {
                SavedToolsView::Recent
            } else {
                SavedToolsView::Favorites
            };
        self.saved_selection = 0;
        self.screen = Screen::SavedTools;
        self.status = "Only tool IDs are saved; NYRA does not record targets here.".into();
    }

    fn toggle_tool_favorite(&mut self, tool_id: &str) {
        if let Some(index) = self.favorite_tool_ids.iter().position(|id| id == tool_id) {
            self.favorite_tool_ids.remove(index);
            self.status = format!("Removed {tool_id} from tool favorites (saving...).");
        } else if crate::find_tool(tool_id).is_some() {
            self.favorite_tool_ids.push(tool_id.to_owned());
            self.favorite_tool_ids.sort_by_key(|favorite| {
                CATALOG
                    .iter()
                    .position(|tool| tool.id == favorite)
                    .unwrap_or(usize::MAX)
            });
            self.status = format!("Added {tool_id} to tool favorites (saving...).");
        } else {
            self.status = "The selected tool is no longer available.".into();
            return;
        }
        self.pending_tool_preferences = Some(self.current_tool_preferences());
    }

    fn current_tool_preferences(&self) -> ToolPreferences {
        ToolPreferences {
            favorite_tool_ids: self.favorite_tool_ids.clone(),
            recent_tool_ids: self.recent_tool_ids.clone(),
            record_recent: self.record_recent,
        }
    }

    fn toggle_recent_recording(&mut self) {
        self.record_recent = !self.record_recent;
        if !self.record_recent {
            self.recent_tool_ids.clear();
            self.saved_selection = 0;
        }
        self.pending_tool_preferences = Some(self.current_tool_preferences());
        self.clamp_saved_selection();
        self.status = if self.record_recent {
            "Recent history enabled; only tool IDs will be saved (saving...).".into()
        } else {
            "Recent history disabled and existing IDs cleared (saving...).".into()
        };
    }

    fn clamp_saved_selection(&mut self) {
        let len = self.visible_saved_tools().len();
        self.saved_selection = if len == 0 {
            0
        } else {
            self.saved_selection.min(len - 1)
        };
    }

    fn record_recent_tool(&mut self) {
        if !self.record_recent {
            return;
        }
        let Some(tool_id) = self.selected_tool_id else {
            return;
        };
        self.recent_tool_ids.retain(|recent| *recent != tool_id);
        self.recent_tool_ids.insert(0, tool_id.to_owned());
        self.recent_tool_ids.truncate(10);
        self.pending_tool_preferences = Some(self.current_tool_preferences());
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind != KeyEventKind::Press {
            return Action::None;
        }
        self.skip_startup_animation();
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return match self.screen {
                Screen::Running => Action::Cancel,
                Screen::ReportExportFormat | Screen::ReportExportPath => {
                    self.cancel_report_export();
                    Action::None
                }
                _ => Action::Quit,
            };
        }
        if key.code == KeyCode::F(5)
            && !matches!(
                self.screen,
                Screen::Running
                    | Screen::Appearance
                    | Screen::Knowledge
                    | Screen::SavedTools
                    | Screen::ReportExportFormat
                    | Screen::ReportExportPath
            )
        {
            self.open_saved_tools();
            return Action::None;
        }
        if !matches!(
            self.screen,
            Screen::Running
                | Screen::Appearance
                | Screen::Knowledge
                | Screen::ReportExportFormat
                | Screen::ReportExportPath
        ) && key.code == KeyCode::F(6)
        {
            self.open_knowledge();
            return Action::None;
        }
        if !matches!(
            self.screen,
            Screen::Running
                | Screen::Appearance
                | Screen::ReportExportFormat
                | Screen::ReportExportPath
        ) && key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('k')
        {
            self.open_global_search();
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('t')
            && !matches!(
                self.screen,
                Screen::ReportExportFormat | Screen::ReportExportPath
            )
        {
            if self.screen == Screen::Appearance {
                self.appearance_selection = (self.appearance_selection + 1) % self.themes.len();
                self.status = format!(
                    "Previewing: {} · Enter applies",
                    self.themes[self.appearance_selection].name()
                );
            } else {
                if self.fallback_theme.take().is_none() {
                    self.theme_index = (self.theme_index + 1) % self.themes.len();
                }
                self.pending_theme = Some(self.theme().id);
                self.status = format!("Theme: {} (saving...)", self.theme().name());
            }
            return Action::None;
        }
        if key.code == KeyCode::F(9)
            && self.screen != Screen::Running
            && self.screen != Screen::Appearance
            && !matches!(
                self.screen,
                Screen::ReportExportFormat | Screen::ReportExportPath
            )
        {
            self.appearance_return_screen = self.screen;
            self.appearance_selection = self.theme_index;
            self.screen = Screen::Appearance;
            return Action::None;
        }

        match self.screen {
            Screen::Tools => self.handle_tools_key(key),
            Screen::Form => self.handle_form_key(key),
            Screen::Running => match key.code {
                KeyCode::Esc => Action::Cancel,
                _ => Action::None,
            },
            Screen::Result => match key.code {
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.open_report_export();
                    Action::None
                }
                KeyCode::Up => {
                    self.result_scroll = self.result_scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    self.result_scroll = self.result_scroll.saturating_add(1);
                    Action::None
                }
                KeyCode::PageUp => {
                    self.result_scroll = self.result_scroll.saturating_sub(8);
                    Action::None
                }
                KeyCode::PageDown => {
                    self.result_scroll = self.result_scroll.saturating_add(8);
                    Action::None
                }
                KeyCode::Esc => {
                    self.screen = Screen::Form;
                    Action::None
                }
                _ => Action::None,
            },
            Screen::ReportExportFormat => self.handle_report_format_key(key),
            Screen::ReportExportPath => self.handle_report_path_key(key),
            Screen::Knowledge => self.handle_knowledge_key(key),
            Screen::SavedTools => self.handle_saved_tools_key(key),
            Screen::Appearance => self.handle_appearance_key(key),
        }
    }

    fn open_report_export(&mut self) {
        if !matches!(&self.completion, Completion::Result(_)) {
            self.status = "There is no diagnostic result to export.".into();
            return;
        }
        self.report_formats = vec![ReportFormat::Json, ReportFormat::Html, ReportFormat::Csv];
        self.report_format_index = 0;
        self.report_path_format = None;
        self.report_path.clear();
        self.status.clear();
        self.screen = Screen::ReportExportFormat;
    }

    fn handle_report_format_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Up => {
                self.report_format_index = self.report_format_index.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.report_format_index = self
                    .report_format_index
                    .saturating_add(1)
                    .min(self.report_formats.len().saturating_sub(1));
                Action::None
            }
            KeyCode::Enter => {
                let Some(format) = self.report_formats.get(self.report_format_index).copied()
                else {
                    self.status = "No report format is available for this result.".into();
                    self.screen = Screen::Result;
                    return Action::None;
                };
                self.report_path_format = Some(format);
                self.report_path.clear();
                self.status.clear();
                self.screen = Screen::ReportExportPath;
                Action::None
            }
            KeyCode::Esc => {
                self.cancel_report_export();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_report_path_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.cancel_report_export();
                Action::None
            }
            KeyCode::Enter => {
                if self.report_path.trim().is_empty() {
                    self.status = "Enter a destination path for the report.".into();
                    return Action::None;
                }
                let Some(format) = self.report_path_format else {
                    self.cancel_report_export();
                    return Action::None;
                };
                Action::ExportReport {
                    format,
                    path: PathBuf::from(self.report_path.as_str()),
                }
            }
            KeyCode::Backspace => {
                pop_last_grapheme(&mut self.report_path);
                self.status.clear();
                Action::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.report_path.clear();
                self.status.clear();
                Action::None
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                if self.report_path.len() + character.len_utf8() <= MAX_REPORT_PATH_BYTES {
                    self.report_path.push(character);
                    self.status.clear();
                } else {
                    self.status = "Destination path is too long.".into();
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn cancel_report_export(&mut self) {
        self.screen = Screen::Result;
        self.report_formats.clear();
        self.report_format_index = 0;
        self.report_path_format = None;
        self.report_path.clear();
        self.status = "Report export cancelled.".into();
    }

    fn finish_report_export(&mut self, format: ReportFormat, path: &Path) {
        let result = match &self.completion {
            Completion::Result(result) => export_result_report(result, format, path),
            Completion::None | Completion::Error(_) => Err(ReportExportError::UnsupportedResult),
        };
        self.screen = Screen::Result;
        self.report_path.clear();
        self.report_path_format = None;
        self.status = match result {
            Ok(()) => format!("{} report exported to the selected path.", format.label()),
            Err(error) => format!("Report export failed: {error}."),
        };
    }

    fn handle_appearance_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers == KeyModifiers::NONE {
            let preferences = match key.code {
                KeyCode::Char('a') => Some(MotionPreferences {
                    animations: !self.motion_preferences.animations,
                    ..self.motion_preferences
                }),
                KeyCode::Char('r') => Some(MotionPreferences {
                    reduced_motion: !self.motion_preferences.reduced_motion,
                    ..self.motion_preferences
                }),
                KeyCode::Char('s') => Some(MotionPreferences {
                    animation_speed: self.motion_preferences.animation_speed.next(),
                    ..self.motion_preferences
                }),
                _ => None,
            };
            if let Some(preferences) = preferences {
                self.set_motion_preferences(preferences, Instant::now());
                return Action::None;
            }
        }

        match key.code {
            KeyCode::Up | KeyCode::BackTab => {
                self.appearance_selection =
                    (self.appearance_selection + self.themes.len() - 1) % self.themes.len();
                self.status = format!(
                    "Previewing: {} · Enter applies",
                    self.themes[self.appearance_selection].name()
                );
                Action::None
            }
            KeyCode::Down | KeyCode::Tab => {
                self.appearance_selection = (self.appearance_selection + 1) % self.themes.len();
                self.status = format!(
                    "Previewing: {} · Enter applies",
                    self.themes[self.appearance_selection].name()
                );
                Action::None
            }
            KeyCode::Enter => {
                let selected = self.appearance_selection;
                self.screen = self.appearance_return_screen;
                let was_fallback = self.fallback_theme.take().is_some();
                if selected == self.theme_index && !was_fallback {
                    self.status = "Theme unchanged.".into();
                } else {
                    self.theme_index = selected;
                    self.pending_theme = Some(self.theme().id);
                    self.status = format!("Theme: {} (saving...)", self.theme().name());
                }
                Action::None
            }
            KeyCode::Esc => {
                self.screen = self.appearance_return_screen;
                self.status = "Appearance preview canceled; motion changes are kept.".into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_tools_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            let selected_id = self.visible_tools().get(self.selection).map(|tool| tool.id);
            if let Some(tool_id) = selected_id {
                self.toggle_tool_favorite(tool_id);
                self.clamp_saved_selection();
            } else {
                self.status = "Select a tool before saving a favorite.".into();
            }
            return Action::None;
        }
        match key.code {
            KeyCode::Tab => {
                self.category_index = (self.category_index + 1) % CATEGORIES.len();
                self.set_tool_selection(0);
                let category = self
                    .current_category()
                    .map_or_else(|| "All".to_owned(), |value| value.to_string());
                self.status = format!("Category: {category}");
                Action::None
            }
            KeyCode::BackTab => {
                self.category_index =
                    (self.category_index + CATEGORIES.len() - 1) % CATEGORIES.len();
                self.set_tool_selection(0);
                let category = self
                    .current_category()
                    .map_or_else(|| "All".to_owned(), |value| value.to_string());
                self.status = format!("Category: {category}");
                Action::None
            }
            KeyCode::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.set_tool_selection(0);
                Action::None
            }
            KeyCode::Esc => {
                if let Some(return_screen) = self.search_return_screen.take() {
                    self.screen = return_screen;
                    self.status.clear();
                    Action::None
                } else {
                    Action::Quit
                }
            }
            KeyCode::Up => {
                self.set_tool_selection(self.selection.saturating_sub(1));
                Action::None
            }
            KeyCode::Down => {
                let len = self.visible_tools().len();
                let selection = if len > 0 {
                    (self.selection + 1).min(len - 1)
                } else {
                    0
                };
                self.set_tool_selection(selection);
                Action::None
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(4);
                Action::None
            }
            KeyCode::PageDown => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add(4)
                    .min(self.detail_scroll_limit.get());
                Action::None
            }
            KeyCode::Home => {
                self.detail_scroll = 0;
                Action::None
            }
            KeyCode::Backspace => {
                pop_last_grapheme(&mut self.query);
                self.set_tool_selection(0);
                Action::None
            }
            KeyCode::Enter => {
                let tools = self.visible_tools();
                if let Some(tool) = tools.get(self.selection) {
                    self.selected_tool_id = Some(tool.id);
                    self.input_values = tool
                        .input_fields
                        .iter()
                        .map(|field| field.default.unwrap_or_default().to_owned())
                        .collect();
                    self.active_field = 0;
                    self.form_return_screen = Screen::Tools;
                    self.status.clear();
                    self.screen = Screen::Form;
                } else {
                    self.status = "No tools match that search. Try a shorter query.".into();
                }
                Action::None
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.query.chars().count() < MAX_SEARCH_CHARS =>
            {
                self.query.push(character);
                self.set_tool_selection(0);
                Action::None
            }
            KeyCode::F(1) => {
                self.status =
                    "Type to search. Enter opens; F6 cards; Ctrl+K resets search; Esc clears or returns.".into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> Action {
        let field_count = self.input_values.len();
        match key.code {
            KeyCode::Esc => {
                self.screen = self.form_return_screen;
                self.status.clear();
                Action::None
            }
            KeyCode::Tab | KeyCode::Down if field_count > 0 => {
                self.active_field = (self.active_field + 1) % field_count;
                Action::None
            }
            KeyCode::BackTab | KeyCode::Up if field_count > 0 => {
                self.active_field = if self.active_field == 0 {
                    field_count - 1
                } else {
                    self.active_field - 1
                };
                Action::None
            }
            KeyCode::Backspace if field_count > 0 => {
                pop_last_grapheme(&mut self.input_values[self.active_field]);
                Action::None
            }
            KeyCode::Enter if field_count > 0 => {
                if self.active_field + 1 < field_count {
                    self.active_field += 1;
                    Action::None
                } else {
                    Action::Start
                }
            }
            KeyCode::Enter => Action::Start,
            KeyCode::Char(character)
                if field_count > 0
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.input_values[self.active_field].push(character);
                Action::None
            }
            KeyCode::F(1) => {
                self.status = "Tab moves between fields. Enter runs the check. Esc returns.".into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_knowledge_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            self.toggle_knowledge_favorite();
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('y') {
            return self
                .selected_knowledge_card()
                .map(|card| Action::Copy(card.command.to_owned()))
                .unwrap_or_else(|| {
                    self.status = "Select a knowledge card before copying its command.".into();
                    Action::None
                });
        }

        match key.code {
            KeyCode::Esc => {
                self.screen = self.knowledge_return_screen;
                self.status.clear();
                Action::None
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.knowledge_favorites_only = !self.knowledge_favorites_only;
                self.set_knowledge_selection(0);
                self.status = if self.knowledge_favorites_only {
                    "Showing favorite knowledge cards. Tab returns to all cards.".into()
                } else {
                    "Showing all knowledge cards.".into()
                };
                Action::None
            }
            KeyCode::Up => {
                self.set_knowledge_selection(self.knowledge_selection.saturating_sub(1));
                Action::None
            }
            KeyCode::Down => {
                let len = self.visible_knowledge_cards().len();
                self.set_knowledge_selection(if len == 0 {
                    0
                } else {
                    (self.knowledge_selection + 1).min(len - 1)
                });
                Action::None
            }
            KeyCode::PageUp => {
                self.knowledge_scroll = self.knowledge_scroll.saturating_sub(6);
                Action::None
            }
            KeyCode::PageDown => {
                self.knowledge_scroll = self
                    .knowledge_scroll
                    .saturating_add(6)
                    .min(self.knowledge_scroll_limit.get());
                Action::None
            }
            KeyCode::Home => {
                self.knowledge_scroll = 0;
                Action::None
            }
            KeyCode::Backspace => {
                pop_last_grapheme(&mut self.knowledge_query);
                self.set_knowledge_selection(0);
                Action::None
            }
            KeyCode::Enter => {
                if let Some(card) = self.selected_knowledge_card() {
                    self.status = format!(
                        "{} is a reference only; NYRA did not run its command.",
                        card.id
                    );
                } else {
                    self.status = "No cards match this search. Try fewer terms.".into();
                }
                Action::None
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.knowledge_query.chars().count() < MAX_SEARCH_CHARS =>
            {
                self.knowledge_query.push(character);
                self.set_knowledge_selection(0);
                Action::None
            }
            KeyCode::F(1) => {
                self.status =
                    "Type to search cards. Ctrl+Y copies; Ctrl+F favorites; Ctrl+K tool search."
                        .into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_saved_tools_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            self.toggle_recent_recording();
            return Action::None;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            let selected_id = self
                .visible_saved_tools()
                .get(self.saved_selection)
                .map(|tool| tool.id);
            if let Some(tool_id) = selected_id {
                self.toggle_tool_favorite(tool_id);
                self.clamp_saved_selection();
            } else {
                self.status = "Select a tool before saving a favorite.".into();
            }
            return Action::None;
        }

        match key.code {
            KeyCode::Esc => {
                self.screen = self.saved_return_screen;
                self.status.clear();
                Action::None
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.saved_tools_view = match self.saved_tools_view {
                    SavedToolsView::Favorites => SavedToolsView::Recent,
                    SavedToolsView::Recent => SavedToolsView::Favorites,
                };
                self.saved_selection = 0;
                Action::None
            }
            KeyCode::Up => {
                self.saved_selection = self.saved_selection.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                let len = self.visible_saved_tools().len();
                self.saved_selection = if len == 0 {
                    0
                } else {
                    (self.saved_selection + 1).min(len - 1)
                };
                Action::None
            }
            KeyCode::Enter => {
                let selected_id = self
                    .visible_saved_tools()
                    .get(self.saved_selection)
                    .map(|tool| tool.id);
                if let Some(tool_id) = selected_id {
                    let Some(tool) = crate::find_tool(tool_id) else {
                        self.status = "The selected tool is no longer available.".into();
                        return Action::None;
                    };
                    self.selected_tool_id = Some(tool_id);
                    self.input_values = tool
                        .input_fields
                        .iter()
                        .map(|field| field.default.unwrap_or_default().to_owned())
                        .collect();
                    self.active_field = 0;
                    self.form_return_screen = Screen::SavedTools;
                    self.status.clear();
                    self.screen = Screen::Form;
                } else {
                    self.status =
                        "This list is empty. Use Ctrl+F in Tools to save a favorite.".into();
                }
                Action::None
            }
            KeyCode::F(1) => {
                self.status = "Tab switches favorites/recents. Enter opens a tool. Ctrl+R toggles recent history.".into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn start_input(&self) -> Result<ToolInput, InputError> {
        let Some(tool) = self.current_tool() else {
            return Err(InputError {
                field_index: 0,
                message: "Select a tool before running a check",
            });
        };
        parse_tool_input(tool.id, &self.input_values)
    }

    fn show_completion(&mut self, completion: Completion) {
        self.completion = completion;
        self.result_scroll = 0;
        self.screen = Screen::Result;
        self.next_running_indicator_tick = None;
        self.status.clear();
    }
}

fn score_tool(tool: &ToolMetadata, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    let mut fields = vec![
        tool.id.to_lowercase(),
        tool.name.to_lowercase(),
        tool.description.to_lowercase(),
        tool.category.to_string().to_lowercase(),
    ];
    fields.extend(tool.aliases.iter().map(|field| field.to_lowercase()));
    fields.extend(tool.keywords.iter().map(|field| field.to_lowercase()));
    fields.extend(tool.sample_queries.iter().map(|field| field.to_lowercase()));
    let alias_match = tool
        .aliases
        .iter()
        .any(|alias| alias.to_lowercase() == query);
    let exact_match = fields.iter().any(|field| field == query);
    if alias_match || exact_match {
        return Some(0);
    }
    if fields.iter().any(|field| field.contains(query)) {
        return Some(1);
    }
    let terms = query.split_whitespace().collect::<Vec<_>>();
    if terms
        .iter()
        .all(|term| fields.iter().any(|field| field.contains(term)))
    {
        return Some(2);
    }
    if terms.iter().all(|term| {
        fields.iter().any(|field| {
            field
                .split(|character: char| !character.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .any(|word| within_edit_distance(term, word, fuzzy_distance(term)))
        })
    }) {
        return Some(3);
    }
    let mut query_chars = query.chars();
    let mut current = query_chars.next();
    for character in fields.iter().flat_map(|field| field.chars()) {
        if current == Some(character) {
            current = query_chars.next();
            if current.is_none() {
                return Some(3);
            }
        }
    }
    None
}

fn fuzzy_distance(value: &str) -> usize {
    if value.chars().count() <= 4 { 1 } else { 2 }
}

fn within_edit_distance(left: &str, right: &str, maximum: usize) -> bool {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    if left.len().abs_diff(right.len()) > maximum {
        return false;
    }

    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_minimum = current[0];
        for (right_index, right_char) in right.iter().enumerate() {
            let cost = if left_char == right_char { 0 } else { 1 };
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(previous[right_index] + cost);
            row_minimum = row_minimum.min(current[right_index + 1]);
        }
        if row_minimum > maximum {
            return false;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()] <= maximum
}

fn platform_list(platforms: &[crate::OperatingSystem]) -> String {
    if platforms.is_empty() {
        return "none recorded".to_owned();
    }
    platforms
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn wrapped_line_count(text: &str, width: u16) -> usize {
    let width = usize::from(width.max(1));
    text.lines()
        .map(|line| {
            let mut visual_lines = 1;
            let mut current_width = 0;
            for word in line.split_whitespace() {
                let word_width = UnicodeWidthStr::width(word);
                if current_width > 0 {
                    if current_width + 1 + word_width <= width {
                        current_width += 1;
                    } else {
                        visual_lines += 1;
                        current_width = 0;
                    }
                }
                let mut remaining = word_width;
                while remaining > 0 {
                    let available = width - current_width;
                    let consumed = remaining.min(available);
                    current_width += consumed;
                    remaining -= consumed;
                    if remaining > 0 {
                        visual_lines += 1;
                        current_width = 0;
                    }
                }
            }
            visual_lines
        })
        .sum()
}

fn render(frame: &mut Frame<'_>, app: &App) {
    frame.render_widget(
        Block::default().style(Style::default().bg(ui_color(app.no_color, app.theme().background))),
        frame.area(),
    );
    match app.screen {
        Screen::Tools => render_tools(frame, app),
        Screen::Form => render_form(frame, app),
        Screen::Running => render_running(frame, app),
        Screen::Result => render_result(frame, app),
        Screen::ReportExportFormat => render_report_export_format(frame, app),
        Screen::ReportExportPath => render_report_export_path(frame, app),
        Screen::Knowledge => render_knowledge(frame, app),
        Screen::SavedTools => render_saved_tools(frame, app),
        Screen::Appearance => render_appearance(frame, app),
    }
}

fn render_tools(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let (header_area, search_area, category_area, list_area, detail_area, footer_area) =
        if frame.area().width >= 130 {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(7),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(20),
                    Constraint::Percentage(35),
                    Constraint::Percentage(45),
                ])
                .split(layout[2]);
            (
                layout[0],
                layout[1],
                Some(columns[0]),
                columns[1],
                columns[2],
                layout[3],
            )
        } else if frame.area().width >= 100 {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(7),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(layout[2]);
            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Min(6)])
                .split(columns[0]);
            (
                layout[0],
                layout[1],
                Some(left[0]),
                left[1],
                columns[1],
                layout[3],
            )
        } else {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(9),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            (layout[0], layout[1], None, layout[2], layout[3], layout[4])
        };
    render_header_with_startup(
        frame,
        header_area,
        theme,
        false,
        app.no_color,
        app.startup_animation_frame(),
    );
    frame.render_widget(
        Paragraph::new(visible_query(
            "Search tools: ",
            &app.query,
            search_area.width,
        ))
        .block(Block::default().borders(Borders::ALL).title("Discover")),
        search_area,
    );

    if let Some(area) = category_area {
        render_categories(frame, app, area, theme);
    }

    let tools = app.visible_tools();
    let items = tools
        .iter()
        .map(|tool| {
            let favorite = if app.favorite_tool_ids.iter().any(|id| id == tool.id) {
                "* "
            } else {
                "  "
            };
            ListItem::new(format!(
                "{favorite}{}  [{} · {}]",
                tool.name, tool.category, tool.risk
            ))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.selection.min(items.len() - 1)));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Tools"))
            .highlight_style(
                Style::default()
                    .fg(ui_color(app.no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            ),
        list_area,
        &mut state,
    );

    let detail = tools.get(app.selection).map_or_else(
        || "No tool matches this query.".to_owned(),
        |tool| {
            format!(
                "{} · {} · {} · {}\n{}\nPlatforms: {}\nSmoke tested on: {}\nOutput: {}\nStatus: {}\nDependencies: {}\nInputs: {}\nExamples: {}",
                tool.id,
                tool.risk,
                tool.backend,
                tool.platform_status,
                tool.description,
                platform_list(tool.platforms),
                platform_list(tool.smoke_tested_platforms),
                tool.output_schema,
                tool.status_explanation,
                if tool.dependencies.is_empty() {
                    "none".to_owned()
                } else {
                    tool.dependencies.join(", ")
                },
                tool.input_fields
                    .iter()
                    .map(|field| field.label)
                    .collect::<Vec<_>>()
                    .join(", "),
                tool.sample_inputs.join(", ")
            )
        },
    );
    let inner_width = detail_area.width.saturating_sub(2);
    let content_lines = wrapped_line_count(&detail, inner_width);
    let visible_lines = usize::from(detail_area.height.saturating_sub(2));
    let max_scroll = content_lines
        .saturating_sub(visible_lines)
        .min(usize::from(u16::MAX)) as u16;
    app.detail_scroll_limit.set(max_scroll);
    let detail_scroll = app.detail_scroll.min(max_scroll);
    let detail_paragraph = Paragraph::new(detail).wrap(Wrap { trim: true });
    frame.render_widget(
        detail_paragraph.scroll((detail_scroll, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Selected tool · PgUp/PgDn scroll · Home top")
                .border_style(Style::default().fg(ui_color(app.no_color, theme.secondary))),
        ),
        detail_area,
    );
    render_footer(
        frame,
        footer_area,
        "F1 help · F5 saved · F6 cards · Ctrl+K search · F9 · Esc/Ctrl+C quit",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_knowledge(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let (header_area, search_area, list_area, detail_area, footer_area) =
        if frame.area().width >= 120 {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
                .split(layout[2]);
            (layout[0], layout[1], columns[0], columns[1], layout[3])
        } else {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Min(8),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            (layout[0], layout[1], layout[2], layout[3], layout[4])
        };
    render_header(frame, header_area, theme, false, app.no_color);
    frame.render_widget(
        Paragraph::new(visible_query(
            "Search cards: ",
            &app.knowledge_query,
            search_area.width,
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Command Knowledge"),
        ),
        search_area,
    );

    let cards = app.visible_knowledge_cards();
    let mut state = ListState::default();
    if !cards.is_empty() {
        state.select(Some(app.knowledge_selection.min(cards.len() - 1)));
    }
    let items = cards
        .iter()
        .map(|card| {
            let favorite = if app.knowledge_favorites.iter().any(|id| id == card.id) {
                "* "
            } else {
                "  "
            };
            ListItem::new(format!(
                "{favorite}{} · {}",
                card.operating_system, card.title
            ))
        })
        .collect::<Vec<_>>();
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(if app.knowledge_favorites_only {
                        "Favorite cards"
                    } else {
                        "Cards"
                    }),
            )
            .highlight_style(
                Style::default()
                    .fg(ui_color(app.no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            ),
        list_area,
        &mut state,
    );

    let details = app.selected_knowledge_card().map_or_else(
        || "No cards match this query. Try fewer terms, or clear the search.".to_owned(),
        |card| {
            format!(
                "{}\nOperating system: {}\nIntent: {}\nCommand (display only; never executed): {}\nParameters: {}\nExample: {}\nExpected output: {}\nRisk: {}\nSide effects: {}\nRequired privileges: {}\nRelated diagnostics: {}\nFavorite: {}\nExplanation: {}\nKeywords: {}",
                card.id,
                card.operating_system,
                card.intent,
                card.command,
                card.parameters,
                card.example,
                card.expected_output,
                card.risk,
                card.side_effects,
                card.required_privileges,
                card.related_diagnostics.join(", "),
                if app.knowledge_favorites.iter().any(|id| id == card.id) {
                    "yes"
                } else {
                    "no"
                },
                card.explanation,
                card.keywords.join(", ")
            )
        },
    );
    let inner_width = detail_area.width.saturating_sub(2);
    let content_lines = wrapped_line_count(&details, inner_width);
    let visible_lines = usize::from(detail_area.height.saturating_sub(2));
    let max_scroll = content_lines
        .saturating_sub(visible_lines)
        .min(usize::from(u16::MAX)) as u16;
    app.knowledge_scroll_limit.set(max_scroll);
    let scroll = app.knowledge_scroll.min(max_scroll);
    frame.render_widget(
        Paragraph::new(details)
            .wrap(Wrap { trim: true })
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Card details · PgUp/PgDn scroll · Home top")
                    .border_style(Style::default().fg(ui_color(app.no_color, theme.secondary))),
            ),
        detail_area,
    );
    render_footer(
        frame,
        footer_area,
        "Type search · ↑/↓ · Tab favs · Ctrl+Y copy · Ctrl+F save · Esc back",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_saved_tools(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let (header_area, list_area, detail_area, footer_area) = if frame.area().width >= 120 {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(4),
            ])
            .split(frame.area());
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(layout[1]);
        (layout[0], columns[0], columns[1], layout[2])
    } else {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Min(8),
                Constraint::Length(4),
            ])
            .split(frame.area());
        (layout[0], layout[1], layout[2], layout[3])
    };
    render_header(frame, header_area, theme, false, app.no_color);

    let tools = app.visible_saved_tools();
    let items = tools
        .iter()
        .map(|tool| {
            let favorite = if app.favorite_tool_ids.iter().any(|id| id == tool.id) {
                "* "
            } else {
                "  "
            };
            ListItem::new(format!("{favorite}{} · {}", tool.name, tool.category))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(app.saved_selection.min(items.len() - 1)));
    }
    let title = match (app.saved_tools_view, app.record_recent) {
        (SavedToolsView::Favorites, _) => "Tool favorites · Tab recent",
        (SavedToolsView::Recent, false) => "Recent tools · disabled · Tab favorites",
        (SavedToolsView::Recent, true) => "Recent tools · Tab favorites",
    };
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .fg(ui_color(app.no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            ),
        list_area,
        &mut state,
    );
    let details = tools
        .get(app.saved_selection)
        .map_or_else(
            || match (app.saved_tools_view, app.record_recent) {
                (SavedToolsView::Recent, false) => "Recent history is disabled. Press Ctrl+R to enable it; NYRA stores tool IDs only.".to_owned(),
                (SavedToolsView::Recent, true) => "No recent tools yet. A tool appears here when a valid run starts. Press Ctrl+R to disable recent history.".to_owned(),
                (SavedToolsView::Favorites, _) => "No tools are saved in this view. Return to Tools and press Ctrl+F on a selection. Recent tools appear here after they run.".to_owned(),
            },
            |tool| {
                format!(
                    "{}\n{}\nCategory: {} · Risk: {}\nPlatforms: {}\nReadiness: {}\nInputs: {}\nExamples: {}\n\nOnly tool IDs are saved. NYRA does not record targets in favorites or recents.",
                    tool.id,
                    tool.description,
                    tool.category,
                    tool.risk,
                    platform_list(tool.platforms),
                    tool.platform_status,
                    tool.input_fields.iter().map(|field| field.label).collect::<Vec<_>>().join(", "),
                    tool.sample_inputs.join(", ")
                )
            },
        );
    frame.render_widget(
        Paragraph::new(details).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Selected tool")
                .border_style(Style::default().fg(ui_color(app.no_color, theme.secondary))),
        ),
        detail_area,
    );
    render_footer(
        frame,
        footer_area,
        "Tab switch · ↑/↓ · Enter open · Ctrl+F save · Ctrl+R history · Esc back",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_categories(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &Theme) {
    let items = CATEGORIES
        .iter()
        .map(|category| {
            ListItem::new(
                category
                    .as_ref()
                    .map_or_else(|| "All".to_owned(), |value| value.to_string()),
            )
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    state.select(Some(app.category_index));
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Categories"))
            .highlight_style(
                Style::default()
                    .fg(ui_color(app.no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            ),
        area,
        &mut state,
    );
}

fn render_form(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false, app.no_color);
    let mut lines = Vec::new();
    if let Some(tool) = app.current_tool() {
        lines.push(format!(
            "{} · {} · R{}",
            tool.name,
            tool.category,
            match tool.risk {
                crate::RiskLevel::R0 => 0,
                crate::RiskLevel::R1 => 1,
                crate::RiskLevel::R2 => 2,
            }
        ));
        lines.push(tool.description.to_owned());
        lines.push(format!(
            "Backend: {} · Readiness: {}",
            tool.backend, tool.platform_status
        ));
        lines.push(format!("Platforms: {}", platform_list(tool.platforms)));
        lines.push(format!(
            "Smoke tested on: {}",
            platform_list(tool.smoke_tested_platforms)
        ));
        if !tool.sample_inputs.is_empty() {
            lines.push(format!("Examples: {}", tool.sample_inputs.join(" · ")));
        }
        lines.push(tool.help.to_owned());
        lines.push(String::new());
        for (index, field) in tool.input_fields.iter().enumerate() {
            let focus = if index == app.active_field { ">" } else { " " };
            let value = app.input_values.get(index).map_or("", String::as_str);
            let bounds = match (field.minimum, field.maximum) {
                (Some(minimum), Some(maximum)) => format!(" ({minimum}–{maximum})"),
                _ => String::new(),
            };
            lines.push(format!("{focus} {}{bounds}: {value}", field.label));
        }
    }
    if !app.status.is_empty() {
        lines.push(String::new());
        lines.push(format!("Input: {}", app.status));
    }
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Check inputs"))
            .wrap(Wrap { trim: true }),
        layout[1],
    );
    render_footer(
        frame,
        layout[2],
        "Type · Tab/↑/↓ next · Enter next/run · Esc back · ^T theme · Ctrl+C quit",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_running(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, true, app.no_color);
    frame.render_widget(
        Paragraph::new(format!(
            "Running {}\n{}\n{}",
            app.current_tool().map_or("diagnostic", |tool| tool.name),
            app.running_indicator_label(),
            if app.status.is_empty() {
                "Waiting for the shared executor…"
            } else {
                &app.status
            }
        ))
        .block(Block::default().borders(Borders::ALL).title("In progress"))
        .wrap(Wrap { trim: true }),
        layout[1],
    );
    render_footer(
        frame,
        layout[2],
        "Esc or Ctrl+C cancels · Ctrl+T theme",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_result(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false, app.no_color);
    let result = match &app.completion {
        Completion::None => "No result is available.".to_owned(),
        Completion::Error(error) => format!("Execution error\n{error}"),
        Completion::Result(result) => {
            let mut lines = vec![
                format!("Execution: {}", result.execution_status_label()),
                format!("Tool: {}", result.tool_id),
                format!(
                    "Observer: {}",
                    result.observer.as_deref().unwrap_or("unknown")
                ),
                format!("Target: {}", result.target),
                format!("Duration: {} ms", result.duration_ms),
            ];
            if let Some(finding) = result.finding {
                lines.push(format!("Finding: {finding:?}"));
            }
            if let Some(tool) = crate::find_tool(result.tool_id)
                && !tool.suggested_follow_ups.is_empty()
            {
                lines.push(format!(
                    "Next checks: {}",
                    tool.suggested_follow_ups.join(", ")
                ));
            }
            if let Some(error) = result.error {
                lines.push(format!("Error: {error}"));
            }
            if let Ok(details) = serde_json::to_string_pretty(result) {
                lines.push("JSON:".into());
                lines.extend(details.lines().map(str::to_owned));
            }
            lines.join("\n")
        }
    };
    frame.render_widget(
        Paragraph::new(result)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Diagnostic result"),
            )
            .wrap(Wrap { trim: true })
            .scroll((app.result_scroll, 0)),
        layout[1],
    );
    render_footer(
        frame,
        layout[2],
        "↑/↓ scroll · PgUp/PgDn page · Ctrl+E export · Esc edit · Ctrl+C quit",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_report_export_format(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false, app.no_color);

    let mut items = app
        .report_formats
        .iter()
        .map(|format| {
            let description = match format {
                ReportFormat::Json => "Sanitized JSON report",
                ReportFormat::Html => "Sanitized HTML report",
                ReportFormat::Csv => "Sanitized CSV report",
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format.label(),
                    Style::default()
                        .fg(ui_color(app.no_color, theme.primary))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}  ·  {description}", format.extension())),
            ]))
        })
        .collect::<Vec<_>>();
    items.push(ListItem::new(Line::from(
        "Reports redact the target and omit raw diagnostic output.",
    )));
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Export diagnostic result"),
        )
        .highlight_style(
            Style::default()
                .fg(ui_color(app.no_color, theme.focus))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    let mut state = ListState::default();
    state.select(Some(app.report_format_index));
    frame.render_stateful_widget(list, layout[1], &mut state);
    render_footer(
        frame,
        layout[2],
        "↑/↓ choose · Enter continue · Esc cancel",
        &app.status,
        theme,
        app.no_color,
    );
}

fn render_report_export_path(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false, app.no_color);

    let label = app.report_path_format.map_or("Report", ReportFormat::label);
    let path_area_width = layout[1].width.saturating_sub(4);
    let path_text = visible_query("Destination: ", &app.report_path, path_area_width);
    let prompt = Paragraph::new(vec![
        Line::from(format!("Export {label} report")),
        Line::from(""),
        Line::from(path_text),
        Line::from(""),
        Line::from("Enter an explicit path. Existing files are never replaced."),
        Line::from("Relative paths use the current working directory."),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Report destination"),
    )
    .wrap(Wrap { trim: true });
    frame.render_widget(prompt, layout[1]);
    render_footer(
        frame,
        layout[2],
        "Type path · Backspace edits · Ctrl+U clears · Enter exports · Esc cancels",
        &app.status,
        theme,
        app.no_color,
    );
}

fn export_result_report(
    result: &RunResult,
    format: ReportFormat,
    path: &Path,
) -> Result<(), ReportExportError> {
    match format {
        ReportFormat::Json => JsonReportDestination::prepare(path)?.write(result),
        ReportFormat::Html => HtmlReportDestination::prepare(path)?.write(result),
        ReportFormat::Csv => CsvReportDestination::prepare(path)?.write(result),
    }
}

fn render_appearance(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(4),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false, app.no_color);
    let panels = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(layout[1]);
    let items = app
        .themes
        .iter()
        .map(|theme| ListItem::new(theme.name()))
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    state.select(Some(
        app.appearance_selection.min(items.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Theme presets"),
            )
            .highlight_style(
                Style::default()
                    .fg(ui_color(app.no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            ),
        panels[0],
        &mut state,
    );

    let preview = &app.themes[app.appearance_selection.min(app.themes.len() - 1)];
    let colors = preview.colors;
    let chip = |label, foreground, background| {
        Span::styled(label, ui_style(app.no_color, foreground, background))
    };
    let preview_lines = vec![
        Line::from(format!("Preview: {}", preview.name())),
        Line::from(vec![
            chip(" Background ", colors.foreground, colors.background),
            chip(" Surface ", colors.foreground, colors.surface),
            chip(" Text ", colors.background, colors.foreground),
        ]),
        Line::from(vec![
            chip(" Primary ", colors.background, colors.primary),
            chip(" Accent ", colors.background, colors.accent),
            chip(" Focus ", colors.background, colors.focus),
        ]),
        Line::from(vec![
            chip(" Success ", colors.background, colors.success),
            chip(" Warning ", colors.background, colors.warning),
            chip(" Error ", colors.background, colors.error),
        ]),
        Line::from(format!(
            "Motion: {} (a) · Reduced: {} (r) · Speed: {} (s)",
            if app.motion_preferences.animations {
                "On"
            } else {
                "Off"
            },
            if app.motion_preferences.reduced_motion {
                "On"
            } else {
                "Off"
            },
            animation_speed_label(app.motion_preferences.animation_speed)
        )),
    ];
    frame.render_widget(
        Paragraph::new(preview_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Color preview"),
            )
            .wrap(Wrap { trim: true }),
        panels[1],
    );
    render_footer(
        frame,
        layout[2],
        "↑/↓ themes · Enter apply · a animation · r reduce · s speed · Esc close",
        &app.status,
        theme,
        app.no_color,
    );
}

fn animation_speed_label(speed: AnimationSpeed) -> &'static str {
    match speed {
        AnimationSpeed::Slow => "Slow",
        AnimationSpeed::Normal => "Normal",
        AnimationSpeed::Fast => "Fast",
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect, theme: &Theme, running: bool, no_color: bool) {
    render_header_with_startup(frame, area, theme, running, no_color, None);
}

fn render_header_with_startup(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    running: bool,
    no_color: bool,
    startup_frame: Option<usize>,
) {
    let title = startup_frame.map_or_else(
        || {
            format!(
                "NYRA v{} · {} · offline-first core · sound off · {} · {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                if running { "busy" } else { "idle" },
                theme.short_label()
            )
        },
        |frame| {
            format!(
                "NYRA v{} · Starting {} · any key skips",
                env!("CARGO_PKG_VERSION"),
                STARTUP_ANIMATION_FRAMES[frame.min(STARTUP_ANIMATION_FRAMES.len() - 1)]
            )
        },
    );
    frame.render_widget(
        Paragraph::new(title)
            .style(
                Style::default()
                    .fg(ui_color(no_color, theme.primary))
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ui_color(no_color, theme.secondary))),
            ),
        area,
    );
}

fn render_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    help: &str,
    status: &str,
    theme: &Theme,
    no_color: bool,
) {
    let text = if status.is_empty() {
        help.to_owned()
    } else {
        format!("{help}\n{status}")
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(ui_color(no_color, theme.accent)))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Keyboard")
                    .border_style(Style::default().fg(ui_color(no_color, theme.secondary))),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn ui_color(no_color: bool, color: Color) -> Color {
    if no_color { Color::Reset } else { color }
}

fn ui_style(no_color: bool, foreground: Color, background: Color) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(foreground).bg(background)
    }
}

fn send_osc52_copy(value: &str) -> io::Result<()> {
    let encoded = encode_base64(value.as_bytes());
    let mut stdout = io::stdout().lock();
    write!(stdout, "\x1b]52;c;{encoded}\x07")?;
    stdout.flush()
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied();
        let third = chunk.get(2).copied();
        encoded.push(char::from(ALPHABET[usize::from(first >> 2)]));
        encoded.push(char::from(
            ALPHABET[usize::from(((first & 0b11) << 4) | second.unwrap_or_default() >> 4)],
        ));
        if let Some(second) = second {
            encoded.push(char::from(
                ALPHABET[usize::from(((second & 0b1111) << 2) | third.unwrap_or_default() >> 6)],
            ));
        } else {
            encoded.push('=');
        }
        if let Some(third) = third {
            encoded.push(char::from(ALPHABET[usize::from(third & 0b0011_1111)]));
        } else {
            encoded.push('=');
        }
    }
    encoded
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
}

pub fn run() -> io::Result<()> {
    let settings = SettingsStore::load_current();
    let no_color = settings.no_color();
    let motion_preferences = settings.motion_preferences(is_ssh_session());
    let configured_theme = settings.selected_theme();
    let knowledge_favorites = settings.knowledge_favorites();
    let tool_preferences = settings.tool_preferences();
    let (custom_theme, custom_theme_issue) = match settings.load_custom_theme() {
        Ok(theme) => (theme, None),
        Err(issue) => (None, Some(issue.user_message())),
    };
    let custom_theme_issue = if configured_theme == ThemeId::Custom && custom_theme.is_none() {
        Some(custom_theme_issue.unwrap_or("custom theme file was not found"))
    } else {
        None
    };
    let theme_id = if custom_theme_issue.is_some() {
        ThemeId::default()
    } else {
        configured_theme
    };
    let fallback_theme = custom_theme_issue.map(|_| Theme::safe_fallback());
    let startup_issue = settings.startup_issue();
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (completion_tx, completion_rx) = mpsc::channel();
    let writer = SettingsWriter::start(settings);
    let mut app = App::with_fallback(theme_id, custom_theme, fallback_theme);
    app.no_color = no_color;
    app.motion_preferences = motion_preferences;
    app.start_startup_animation_at(Instant::now());
    app.knowledge_favorites = knowledge_favorites;
    app.record_recent = tool_preferences.record_recent;
    app.favorite_tool_ids = tool_preferences.favorite_tool_ids;
    app.recent_tool_ids = if app.record_recent {
        tool_preferences.recent_tool_ids
    } else {
        Vec::new()
    };
    if !app.record_recent {
        app.pending_tool_preferences = Some(app.current_tool_preferences());
    }
    if let Some(issue) = startup_issue {
        app.status = format!(
            "Settings unavailable; using {} ({}).",
            app.theme().name(),
            issue.user_message()
        );
    } else if let Some(issue) = custom_theme_issue {
        app.status = format!(
            "Custom theme unavailable; using {} ({}).",
            app.theme().name(),
            issue
        );
    }
    let mut cancel_sender: Option<oneshot::Sender<()>> = None;
    let mut worker: Option<JoinHandle<()>> = None;

    let loop_result = (|| -> io::Result<()> {
        loop {
            app.advance_startup_animation_at(Instant::now());
            app.advance_running_indicator_at(Instant::now());
            terminal.draw(|frame| render(frame, &app))?;
            while let Some(result) = writer.try_result() {
                app.show_settings_save_result(result);
            }
            if let Ok(completion) = completion_rx.try_recv() {
                app.show_completion(completion);
                cancel_sender = None;
                if let Some(worker) = worker.take() {
                    let _ = worker.join();
                }
            }
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                match app.handle_key(key) {
                    Action::None => {}
                    Action::Quit => break,
                    Action::Cancel => {
                        if let Some(cancel) = cancel_sender.take() {
                            let _ = cancel.send(());
                            app.status =
                                "Cancellation requested; waiting for the tool to stop.".into();
                        }
                    }
                    Action::Copy(command) => {
                        send_osc52_copy(&command)?;
                        app.status =
                            "Copy request sent using OSC 52; terminal support varies.".into();
                    }
                    Action::ExportReport { format, path } => {
                        app.finish_report_export(format, &path);
                    }
                    Action::Start => match app.start_input() {
                        Ok(input) => {
                            app.record_recent_tool();
                            app.screen = Screen::Running;
                            app.start_running_indicator_at(Instant::now());
                            app.status.clear();
                            let tx = completion_tx.clone();
                            let (cancel_tx, cancel_rx) = oneshot::channel();
                            cancel_sender = Some(cancel_tx);
                            worker = Some(thread::spawn(move || {
                                let completion =
                                    match Builder::new_current_thread().enable_all().build() {
                                        Ok(runtime) => {
                                            let cancel = async move {
                                                cancel_rx.await.map_err(|_| RunError::Cancelled)?;
                                                Ok(())
                                            };
                                            let result = runtime.block_on(execute(input, cancel));
                                            runtime.shutdown_background();
                                            Completion::Result(Box::new(result))
                                        }
                                        Err(error) => Completion::Error(error.to_string()),
                                    };
                                let _ = tx.send(completion);
                            }));
                        }
                        Err(InputError {
                            field_index,
                            message,
                        }) => {
                            if !app.input_values.is_empty() {
                                app.active_field = field_index.min(app.input_values.len() - 1);
                            }
                            app.status = message.into();
                        }
                    },
                }
            }
            if let Some(theme) = app.pending_theme {
                match writer.try_save(SettingsChange::Theme(theme)) {
                    Ok(()) => app.pending_theme = None,
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        app.pending_theme = None;
                        app.status = format!(
                            "Theme: {} (session only; settings writer stopped)",
                            app.theme().name()
                        );
                    }
                }
            }
            if let Some(favorites) = app.pending_knowledge_favorites.clone() {
                match writer.try_save(SettingsChange::KnowledgeFavorites(favorites)) {
                    Ok(()) => app.pending_knowledge_favorites = None,
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        app.pending_knowledge_favorites = None;
                        app.status =
                            "Knowledge favorites are session only; settings writer stopped.".into();
                    }
                }
            }
            if let Some(preferences) = app.pending_tool_preferences.clone() {
                match writer.try_save(SettingsChange::ToolPreferences(preferences)) {
                    Ok(()) => app.pending_tool_preferences = None,
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        app.pending_tool_preferences = None;
                        app.status =
                            "Tool favorites and recents are session only; settings writer stopped."
                                .into();
                    }
                }
            }
            if let Some(preferences) = app.pending_motion_preferences {
                match writer.try_save(SettingsChange::Motion(preferences)) {
                    Ok(()) => app.pending_motion_preferences = None,
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        app.pending_motion_preferences = None;
                        app.status = "Motion preferences could not be saved.".into();
                    }
                }
            }
        }
        Ok(())
    })();

    if let Some(cancel) = cancel_sender.take() {
        let _ = cancel.send(());
    }
    if let Some(worker) = worker.take() {
        let _ = worker.join();
    }
    let pending_changes = app
        .pending_theme
        .take()
        .map(SettingsChange::Theme)
        .into_iter()
        .chain(
            app.pending_knowledge_favorites
                .take()
                .map(SettingsChange::KnowledgeFavorites),
        )
        .chain(
            app.pending_tool_preferences
                .take()
                .map(SettingsChange::ToolPreferences),
        )
        .chain(
            app.pending_motion_preferences
                .take()
                .map(SettingsChange::Motion),
        );
    writer.finish(pending_changes);
    loop_result?;
    Ok(())
}

fn is_ssh_session() -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn backspace_removes_a_combined_grapheme_as_one_unit() {
        let mut value = "e\u{301}".to_owned();
        pop_last_grapheme(&mut value);
        assert!(value.is_empty());
    }

    #[test]
    fn long_search_queries_keep_the_visible_tail_at_cell_boundaries() {
        let display = visible_query("Search: ", "abcdefghijklmnopqrstuvwxyz", 12);
        assert_eq!(display, "Search: …z");

        let display = visible_query("Find: ", "abc界def", 14);
        assert!(display.ends_with("界def"));
        assert!(UnicodeWidthStr::width(display.as_str()) <= 12);
    }

    #[test]
    fn home_screen_fits_80_by_24_and_shows_search_and_tool_detail() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Search tools"));
        assert!(text.contains("TCP Port Check"));
        assert!(text.contains("Check whether one host accepts a TCP connection"));
        assert!(text.contains("finding_only"));
        assert!(text.contains("offline-first core"));
        assert!(text.contains("sound off"));
        assert!(text.contains("idle"));
        assert!(text.contains("Ctrl+C quit"));
    }

    #[test]
    fn knowledge_search_opens_from_the_current_tool_query_and_fits_80_by_24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.query = "disk full".into();
        app.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);

        assert_eq!(app.screen, Screen::Knowledge);
        assert_eq!(app.knowledge_query, "disk full");
        assert_eq!(
            app.selected_knowledge_card().map(|card| card.id),
            Some("linux.filesystem-space")
        );
        assert!(text.contains("Command Knowledge"), "{text}");
        assert!(text.contains("Linux · Check filesystem space"), "{text}");
        assert!(text.contains("Command (display only"), "{text}");
        assert!(text.contains("Ctrl+Y copy"), "{text}");
    }

    #[test]
    fn knowledge_screen_returns_to_the_opening_screen_and_filters_favorites() {
        let mut app = App::new();
        app.screen = Screen::Form;
        app.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Knowledge);
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);

        app.open_knowledge();
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert_eq!(app.knowledge_favorites, ["linux.filesystem-space"]);
        assert_eq!(
            app.pending_knowledge_favorites,
            Some(vec!["linux.filesystem-space".into()])
        );
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.visible_knowledge_cards().len(), 1);
        assert_eq!(
            app.selected_knowledge_card().map(|card| card.id),
            Some("linux.filesystem-space")
        );
    }

    #[test]
    fn knowledge_copy_requests_only_the_selected_reference_command() {
        let mut app = App::new();
        app.open_knowledge();
        assert!(matches!(
            app.handle_key(KeyEvent::new(
                KeyCode::Char('y'),
                KeyModifiers::CONTROL,
            )),
            Action::Copy(command) if command == "df -h"
        ));

        assert_eq!(encode_base64(b"df -h"), "ZGYgLWg=");
        assert_eq!(encode_base64(b"abc"), "YWJj");
        assert_eq!(encode_base64(b""), "");
    }

    #[test]
    fn running_indicator_advances_at_a_bounded_cadence_and_stops_on_completion() {
        let start = Instant::now();
        let mut app = App::new();
        app.screen = Screen::Running;
        app.start_running_indicator_at(start);

        assert_eq!(app.running_indicator_label(), "Working |");
        app.advance_running_indicator_at(
            start + RUNNING_INDICATOR_INTERVAL - Duration::from_millis(1),
        );
        assert_eq!(app.running_indicator_label(), "Working |");
        app.advance_running_indicator_at(start + RUNNING_INDICATOR_INTERVAL);
        assert_eq!(app.running_indicator_label(), "Working /");

        app.advance_running_indicator_at(start + Duration::from_secs(10));
        assert_eq!(app.running_indicator_label(), "Working -");

        app.show_completion(Completion::Error("finished".into()));
        app.advance_running_indicator_at(start + Duration::from_secs(20));
        assert_eq!(app.running_indicator_label(), "Working -");
        assert!(app.next_running_indicator_tick.is_none());
    }

    #[test]
    fn startup_animation_respects_motion_preferences_and_stops_within_one_second() {
        let start = Instant::now();
        let mut app = App::new();
        app.motion_preferences.animation_speed = AnimationSpeed::Slow;
        app.start_startup_animation_at(start);
        let interval = app.running_indicator_interval();

        assert_eq!(app.startup_animation_frame(), Some(0));
        app.advance_startup_animation_at(start + interval);
        assert_eq!(app.startup_animation_frame(), Some(1));
        app.advance_startup_animation_at(start + interval + interval);
        assert_eq!(app.startup_animation_frame(), Some(2));
        app.advance_startup_animation_at(start + interval + interval + interval);
        assert_eq!(app.startup_animation_frame(), None);
        assert!(interval + interval + interval < Duration::from_secs(1));

        app.motion_preferences.reduced_motion = true;
        app.start_startup_animation_at(start);
        assert_eq!(app.startup_animation_frame(), None);
    }

    #[test]
    fn any_startup_key_skips_the_effect_and_still_reaches_the_home_screen() {
        let mut app = App::new();
        app.start_startup_animation_at(Instant::now());
        assert!(app.startup_animation_frame().is_some());

        let action = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert!(matches!(action, Action::None));
        assert_eq!(app.selection, 1);
        assert_eq!(app.startup_animation_frame(), None);
    }

    #[test]
    fn startup_animation_is_visible_in_the_80_by_24_header() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.start_startup_animation_at(Instant::now());

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);

        assert!(text.contains("Starting |"), "{text}");
        assert!(text.contains("any key skips"), "{text}");
    }

    #[test]
    fn static_running_screen_keeps_cancellation_status_visible_at_80_by_24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.screen = Screen::Running;
        app.motion_preferences.animations = false;
        app.status = "Cancellation requested; waiting for the tool to stop.".into();
        app.start_running_indicator_at(Instant::now());
        assert!(matches!(
            app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL,)),
            Action::Cancel
        ));

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);

        assert!(text.contains("Working"), "{text}");
        assert!(!text.contains("Working |"), "{text}");
        assert!(
            text.contains("Cancellation requested; waiting for the tool to stop."),
            "{text}"
        );
        assert!(app.next_running_indicator_tick.is_none());
    }

    #[test]
    fn knowledge_no_color_mode_uses_terminal_default_foreground_and_background() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.open_knowledge();
        app.no_color = true;

        terminal.draw(|frame| render(frame, &app)).unwrap();

        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
        );
    }

    #[test]
    fn tool_favorites_open_from_the_saved_screen_and_return_after_opening_a_form() {
        let mut app = App::new();
        app.query = "check port".into();
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert_eq!(app.favorite_tool_ids, ["network.tcp"]);

        app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::SavedTools);
        assert_eq!(app.visible_saved_tools()[0].id, "network.tcp");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
        assert_eq!(app.form_return_screen, Screen::SavedTools);
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::SavedTools);
    }

    #[test]
    fn saved_tool_screen_renders_at_80_by_24_and_recent_lists_keep_only_tool_ids() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.favorite_tool_ids.push("network.tcp".into());
        app.open_saved_tools();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Tool favorites"), "{text}");
        assert!(text.contains("TCP Port Check"), "{text}");
        assert!(text.contains("Only tool IDs are saved"), "{text}");

        for tool in CATALOG {
            app.selected_tool_id = Some(tool.id);
            app.record_recent_tool();
        }
        assert_eq!(app.recent_tool_ids.len(), 10);
        assert_eq!(app.recent_tool_ids[0], CATALOG[CATALOG.len() - 1].id);
        assert_eq!(app.recent_tool_ids[9], CATALOG[CATALOG.len() - 10].id);
    }

    #[test]
    fn saved_tools_can_switch_to_the_recent_list() {
        let mut app = App::new();
        app.favorite_tool_ids.push("network.tcp".into());
        app.recent_tool_ids.push("system.disk".into());
        app.open_saved_tools();
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.saved_tools_view, SavedToolsView::Recent);
        assert_eq!(app.visible_saved_tools()[0].id, "system.disk");
    }

    #[test]
    fn recent_history_can_be_disabled_cleared_and_reenabled() {
        let mut app = App::new();
        app.recent_tool_ids.push("network.tcp".into());
        app.open_saved_tools();
        app.saved_tools_view = SavedToolsView::Recent;

        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

        assert!(!app.record_recent);
        assert!(app.recent_tool_ids.is_empty());
        assert_eq!(
            app.pending_tool_preferences,
            Some(ToolPreferences {
                favorite_tool_ids: Vec::new(),
                recent_tool_ids: Vec::new(),
                record_recent: false,
            })
        );

        app.selected_tool_id = Some("system.disk");
        app.record_recent_tool();
        assert!(app.recent_tool_ids.is_empty());

        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.record_recent);

        app.record_recent_tool();
        assert_eq!(app.recent_tool_ids, ["system.disk"]);
    }

    #[test]
    fn minimal_tool_detail_can_scroll_to_its_hidden_examples() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let initial = rendered_text(&terminal);
        assert!(initial.contains("PgUp/PgDn scroll"));
        assert!(!initial.contains("Examples:"));

        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.detail_scroll, app.detail_scroll_limit.get());
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(rendered_text(&terminal).contains("Examples:"));

        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.detail_scroll, app.detail_scroll_limit.get());
        app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert_eq!(app.detail_scroll, 0);
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn detail_wrapping_counts_combining_and_wide_characters_by_cell_width() {
        assert_eq!(wrapped_line_count("e\u{301}", 1), 1);
        assert_eq!(wrapped_line_count("界界", 2), 2);
        assert_eq!(wrapped_line_count("┌─┐", 3), 1);
    }

    #[test]
    fn scrolled_tool_detail_returns_to_the_top_when_viewport_grows() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert!(app.detail_scroll > 0);

        terminal.backend_mut().resize(150, 36);
        terminal.resize(Rect::new(0, 0, 150, 36)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("network.tcp"), "{text}");
        assert!(text.contains("Examples:"));
    }

    #[test]
    fn tool_explorer_preserves_search_and_context_across_layout_sizes() {
        let backend = TestBackend::new(150, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        for character in "dns lookup".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }

        for (width, height) in [(150, 36), (120, 36), (80, 24)] {
            terminal.backend_mut().resize(width, height);
            terminal.resize(Rect::new(0, 0, width, height)).unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let text = rendered_text(&terminal);
            assert!(text.contains("Search tools: dns lookup"), "{text}");
            assert!(text.contains("DNS Lookup"), "{text}");
            assert!(text.contains("network.dns"), "{text}");
            assert!(text.contains("Ctrl+C quit"), "{text}");
        }
    }

    #[test]
    fn long_execution_error_is_reachable_from_the_80_by_24_result_view() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        let details = (0..24)
            .map(|index| format!("Diagnostic detail {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.screen = Screen::Result;
        app.completion = Completion::Error(format!("{details}\nCRITICAL FINAL DETAIL"));

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let initial = rendered_text(&terminal);
        assert!(initial.contains("Execution error"));
        assert!(initial.contains("PgUp/PgDn page"));
        assert!(!initial.contains("CRITICAL FINAL DETAIL"));

        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(rendered_text(&terminal).contains("CRITICAL FINAL DETAIL"));
    }

    #[test]
    fn compact_tool_explorer_places_categories_and_detail_side_by_side() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Tools"));
        assert!(
            text.lines()
                .any(|line| { line.contains("Categories") && line.contains("Selected tool") })
        );
    }

    #[test]
    fn wide_tool_explorer_shows_categories_tools_and_detail_in_columns() {
        let backend = TestBackend::new(150, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.lines().any(|line| {
            line.contains("Categories") && line.contains("Tools") && line.contains("Selected tool")
        }));
    }

    #[test]
    fn english_task_query_opens_the_matching_tcp_form() {
        let mut app = App::new();
        for character in "check port".chars() {
            assert!(matches!(
                app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
                Action::None
            ));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
        assert_eq!(app.selected_tool_id, Some("network.tcp"));
        assert_eq!(app.input_values, [String::new(), String::new(), "5".into()]);
    }

    #[test]
    fn fuzzy_typo_query_finds_the_tcp_tool() {
        let mut app = App::new();
        for character in "chek port".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.selected_tool_id, Some("network.tcp"));
    }

    #[test]
    fn curated_english_query_opens_the_tcp_form() {
        let mut app = App::new();
        for character in "port connectivity".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.selected_tool_id, Some("network.tcp"));
        assert_eq!(app.screen, Screen::Form);
    }

    #[test]
    fn ctrl_k_opens_global_tool_search_and_returns_to_the_previous_screen() {
        let mut app = App::new();
        app.screen = Screen::Form;
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(app.screen, Screen::Tools);
        assert!(app.query.is_empty());
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
    }

    #[test]
    fn removing_the_selected_tool_favorite_clamps_the_saved_screen_selection() {
        let mut app = App::new();
        app.favorite_tool_ids = vec!["network.tcp".into(), "system.disk".into()];
        app.open_saved_tools();
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.saved_selection, 1);
        assert_eq!(
            app.visible_saved_tools()[app.saved_selection].id,
            "system.disk"
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));

        assert_eq!(app.favorite_tool_ids, ["network.tcp"]);
        assert_eq!(app.saved_selection, 0);
        assert_eq!(
            app.visible_saved_tools()[app.saved_selection].id,
            "network.tcp"
        );
    }

    #[test]
    fn category_tabs_filter_the_visible_tools() {
        let mut app = App::new();
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            app.visible_tools()
                .iter()
                .all(|tool| { tool.category == ToolCategory::Network })
        );
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            app.visible_tools()
                .iter()
                .all(|tool| { tool.category == ToolCategory::System })
        );
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(app.current_category(), Some(ToolCategory::Network));
    }

    #[test]
    fn system_information_opens_a_zero_input_form_that_can_run() {
        let mut app = App::new();
        for character in "system info".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
        assert!(app.input_values.is_empty());
        assert!(matches!(
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Action::Start
        ));
        assert!(app.start_input().is_ok());
    }

    #[test]
    fn tool_form_shows_backend_and_platform_readiness() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.screen = Screen::Form;
        app.selected_tool_id = Some("network.ping");
        app.input_values = vec![String::new(), "4".into(), "5".into()];
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Backend: operating_system"));
        assert!(text.contains("Readiness: needs smoke test"));
        assert!(text.contains("Platforms: Windows, Linux"));
        assert!(text.contains("Smoke tested on: none recorded"));
        assert!(text.contains("Examples: example.com"));
    }

    #[test]
    fn dns_search_opens_the_metadata_driven_default_form() {
        let mut app = App::new();
        for character in "dns lookup".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
        assert_eq!(app.selected_tool_id, Some("network.dns"));
        assert_eq!(
            app.input_values,
            [String::new(), "A,AAAA".into(), "5".into()]
        );
    }

    #[test]
    fn web_diagnostics_search_opens_its_url_form() {
        let mut app = App::new();
        for character in "web diagnostics".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Form);
        assert_eq!(app.selected_tool_id, Some("workflow.web_diagnostics"));
        assert_eq!(app.input_values, [String::new(), "15".into()]);
    }

    #[test]
    fn theme_shortcut_cycles_the_four_data_driven_palettes() {
        let mut app = App::new();
        for expected in 1..app.themes.len() {
            app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
            assert_eq!(app.theme_index, expected);
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.theme_index, 0);
    }

    #[test]
    fn configured_theme_is_selected_and_a_new_choice_is_queued_for_saving() {
        let mut app = App::with_themes(ThemeId::NeoTokyo, None);
        assert_eq!(app.theme().id, ThemeId::NeoTokyo);

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(app.theme().id, ThemeId::AcidGrid);
        assert_eq!(app.pending_theme, Some(ThemeId::AcidGrid));
    }

    #[test]
    fn custom_theme_is_added_to_the_cycle_only_when_loaded() {
        let custom = Theme::parse_custom(
            r##"
version = 1
name = "Test Theme"

[colors]
background = "#071B22"
surface = "#0A3342"
foreground = "#D8F7FF"
muted = "#73A0AD"
primary = "#00E5FF"
accent = "#FF39C6"
secondary = "#0A3342"
border = "#176070"
focus = "#00E5FF"
selection = "#10404B"
success = "#44E28F"
warning = "#F5E634"
error = "#FF5D64"
disabled = "#55707A"
progress = "#00C4FF"
"##,
        )
        .unwrap();
        let mut app = App::with_themes(ThemeId::Custom, Some(custom));
        assert_eq!(app.theme().id, ThemeId::Custom);
        assert_eq!(app.theme().name(), "Test Theme");

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(app.theme().id, ThemeId::CyberCyan);
        assert_eq!(app.pending_theme, Some(ThemeId::CyberCyan));
    }

    #[test]
    fn appearance_screen_previews_then_applies_a_theme_and_returns_to_the_previous_screen() {
        let mut app = App::new();
        app.screen = Screen::Form;

        app.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Appearance);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.theme().id, ThemeId::CyberCyan);
        assert_eq!(app.themes[app.appearance_selection].id, ThemeId::NeoTokyo);

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.screen, Screen::Form);
        assert_eq!(app.theme().id, ThemeId::NeoTokyo);
        assert_eq!(app.pending_theme, Some(ThemeId::NeoTokyo));
    }

    #[test]
    fn canceling_appearance_returns_without_changing_the_theme() {
        let mut app = App::new();
        app.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.screen, Screen::Tools);
        assert_eq!(app.theme().id, ThemeId::CyberCyan);
        assert_eq!(app.pending_theme, None);
    }

    #[test]
    fn appearance_motion_shortcuts_update_preferences_and_queue_persistence() {
        let mut app = App::new();
        app.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!app.motion_preferences.animations);
        assert!(!app.motion_preferences.is_animated());

        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        assert!(app.motion_preferences.reduced_motion);
        assert_eq!(app.motion_preferences.animation_speed, AnimationSpeed::Fast);

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!app.motion_preferences.is_animated());
        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert!(app.motion_preferences.is_animated());
        assert_eq!(app.pending_motion_preferences, Some(app.motion_preferences));
    }

    #[test]
    fn running_indicator_uses_slow_and_fast_cadence_preferences() {
        for (speed, interval) in [
            (AnimationSpeed::Slow, Duration::from_millis(300)),
            (AnimationSpeed::Fast, Duration::from_millis(75)),
        ] {
            let start = Instant::now();
            let mut app = App::new();
            app.screen = Screen::Running;
            app.motion_preferences.animation_speed = speed;
            app.start_running_indicator_at(start);

            app.advance_running_indicator_at(start + interval - Duration::from_millis(1));
            assert_eq!(app.running_indicator_label(), "Working |");
            app.advance_running_indicator_at(start + interval);
            assert_eq!(app.running_indicator_label(), "Working /");
        }
    }

    #[test]
    fn safe_fallback_is_session_only_until_a_theme_is_selected() {
        let mut app = App::with_fallback(ThemeId::CyberCyan, None, Some(Theme::safe_fallback()));
        assert_eq!(app.theme().name(), "Safe Fallback");
        assert_eq!(app.pending_theme, None);

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));

        assert_eq!(app.theme().name(), "Cyber Cyan");
        assert_eq!(app.pending_theme, Some(ThemeId::CyberCyan));
    }

    #[test]
    fn appearance_screen_shows_theme_list_and_color_preview_at_80_by_24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);

        assert!(text.contains("Theme presets"), "{text}");
        assert!(text.contains("Color preview"), "{text}");
        assert!(text.contains("Background"), "{text}");
        assert!(text.contains("Motion: On (a)"), "{text}");
        assert!(text.contains("Reduced: Off (r)"), "{text}");
        assert!(text.contains("Speed: Normal (s)"), "{text}");
        assert!(text.contains("Enter apply"), "{text}");
    }

    #[test]
    fn no_color_mode_uses_terminal_default_foreground_and_background() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.no_color = true;

        terminal.draw(|frame| render(frame, &app)).unwrap();

        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|cell| { cell.fg == Color::Reset && cell.bg == Color::Reset })
        );
    }

    #[test]
    fn result_view_supports_keyboard_scrolling() {
        let mut app = App::new();
        app.screen = Screen::Result;
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.result_scroll, 9);
        app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert_eq!(app.result_scroll, 1);
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.result_scroll, 0);
    }

    #[test]
    fn result_view_shows_execution_status_suggestions_and_full_json() {
        let backend = TestBackend::new(100, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.show_completion(Completion::Result(Box::new(RunResult {
            schema_version: 1,
            tool_id: "network.tcp",
            observer: Some("test-host".into()),
            target: "127.0.0.1:443".into(),
            execution_status: crate::ExecutionStatus::Completed,
            finding: Some(crate::Finding::Open),
            data: None,
            duration_ms: 3,
            error: None,
        })));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Execution: completed"));
        assert!(text.contains("Observer: test-host"));
        assert!(text.contains("Next checks: network.dns"));
        assert!(text.contains("JSON:"));
    }

    #[test]
    fn result_view_opens_a_keyboard_report_export_and_can_cancel_it() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.show_completion(Completion::Result(Box::new(report_test_result())));

        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert_eq!(app.screen, Screen::ReportExportFormat);
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = rendered_text(&terminal);
        assert!(text.contains("Export diagnostic result"), "{text}");
        assert!(text.contains("JSON"), "{text}");
        assert!(text.contains("Enter continue"), "{text}");

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::Result);
        assert_eq!(app.status, "Report export cancelled.");
    }

    #[test]
    fn report_export_accepts_an_explicit_path_and_writes_a_sanitized_json_report() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("diagnostic-report.json");
        let path_text = path.to_string_lossy().into_owned();
        let mut app = App::new();
        app.show_completion(Completion::Result(Box::new(report_test_result())));

        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::ReportExportPath);
        for character in path_text.chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        assert_eq!(app.report_path, path_text);
        let theme_id = app.theme().id;
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
        assert_eq!(app.screen, Screen::ReportExportPath);
        assert_eq!(app.report_path, path_text);
        assert_eq!(app.theme().id, theme_id);
        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let Action::ExportReport {
            format: ReportFormat::Json,
            path: action_path,
        } = action
        else {
            panic!("expected the explicit JSON export action");
        };
        assert_eq!(action_path, path);
        app.finish_report_export(ReportFormat::Json, &action_path);

        let report = std::fs::read_to_string(path).unwrap();
        assert!(report.contains("[redacted]"));
        assert!(!report.contains("10.20.30.40:443"));
        assert_eq!(app.screen, Screen::Result);
        assert!(app.status.contains("JSON report exported"));

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(rendered_text(&terminal).contains("JSON report exported"));
    }

    #[test]
    fn report_export_does_not_overwrite_existing_files() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("keep.html");
        std::fs::write(&path, "keep this report").unwrap();
        let mut app = App::new();
        app.show_completion(Completion::Result(Box::new(report_test_result())));

        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.report_path = path.to_string_lossy().into_owned();
        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let Action::ExportReport {
            format: ReportFormat::Html,
            path: action_path,
        } = action
        else {
            panic!("expected the selected HTML export action");
        };
        app.finish_report_export(ReportFormat::Html, &action_path);

        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep this report");
        assert!(app.status.contains("already exists"));

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert!(rendered_text(&terminal).contains("already exists"));
    }

    fn report_test_result() -> RunResult {
        RunResult {
            schema_version: 1,
            tool_id: "network.tcp",
            observer: Some("internal-observer".into()),
            target: "10.20.30.40:443".into(),
            execution_status: crate::ExecutionStatus::Completed,
            finding: Some(crate::Finding::Open),
            data: None,
            duration_ms: 8,
            error: None,
        }
    }
}
