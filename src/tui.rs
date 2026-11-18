use crate::{
    InputError, RunError, RunResult, TOOLS, ToolCategory, ToolInput, ToolMetadata, execute,
    parse_tool_input,
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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    cell::Cell,
    io,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
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
}

#[derive(Clone, Copy)]
struct Theme {
    name: &'static str,
    short_label: &'static str,
    background: Color,
    primary: Color,
    accent: Color,
    secondary: Color,
}

const THEMES: &[Theme] = &[
    Theme {
        name: "Cyber Cyan",
        short_label: "Cyan",
        background: Color::Rgb(7, 27, 34),
        primary: Color::Rgb(0, 229, 255),
        accent: Color::Rgb(255, 57, 198),
        secondary: Color::Rgb(10, 51, 66),
    },
    Theme {
        name: "Neo Tokyo",
        short_label: "Neo",
        background: Color::Rgb(26, 8, 44),
        primary: Color::Rgb(168, 85, 247),
        accent: Color::Rgb(255, 53, 167),
        secondary: Color::Rgb(255, 148, 46),
    },
    Theme {
        name: "Acid Grid",
        short_label: "Acid",
        background: Color::Rgb(11, 23, 18),
        primary: Color::Rgb(185, 255, 0),
        accent: Color::Rgb(0, 229, 229),
        secondary: Color::Rgb(245, 230, 52),
    },
    Theme {
        name: "Crimson Noir",
        short_label: "Crimson",
        background: Color::Rgb(25, 8, 16),
        primary: Color::Rgb(255, 40, 93),
        accent: Color::Rgb(187, 107, 217),
        secondary: Color::Rgb(88, 32, 58),
    },
];

const CATEGORIES: &[Option<ToolCategory>] = &[
    None,
    Some(ToolCategory::Network),
    Some(ToolCategory::System),
    Some(ToolCategory::Security),
    Some(ToolCategory::File),
];
const MAX_SEARCH_CHARS: usize = 128;

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
    Quit,
}

fn pop_last_grapheme(value: &mut String) {
    if let Some((start, _)) = value.grapheme_indices(true).next_back() {
        value.truncate(start);
    }
}

struct App {
    screen: Screen,
    query: String,
    selection: usize,
    category_index: usize,
    selected_tool_id: Option<&'static str>,
    input_values: Vec<String>,
    active_field: usize,
    theme_index: usize,
    detail_scroll: u16,
    detail_scroll_limit: Cell<u16>,
    result_scroll: u16,
    status: String,
    completion: Completion,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Tools,
            query: String::new(),
            selection: 0,
            category_index: 0,
            selected_tool_id: None,
            input_values: Vec::new(),
            active_field: 0,
            theme_index: 0,
            detail_scroll: 0,
            detail_scroll_limit: Cell::new(0),
            result_scroll: 0,
            status: String::new(),
            completion: Completion::None,
        }
    }

    fn visible_tools(&self) -> Vec<&'static ToolMetadata> {
        let query = self.query.trim().to_lowercase();
        let mut tools = TOOLS
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

    fn current_tool(&self) -> Option<&'static ToolMetadata> {
        self.selected_tool_id.and_then(crate::find_tool)
    }

    fn theme(&self) -> Theme {
        THEMES[self.theme_index]
    }

    fn current_category(&self) -> Option<ToolCategory> {
        CATEGORIES[self.category_index]
    }

    fn set_tool_selection(&mut self, selection: usize) {
        self.selection = selection;
        self.detail_scroll = 0;
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind != KeyEventKind::Press {
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return if self.screen == Screen::Running {
                Action::Cancel
            } else {
                Action::Quit
            };
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
            self.theme_index = (self.theme_index + 1) % THEMES.len();
            self.status = format!("Theme: {}", self.theme().name);
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
        }
    }

    fn handle_tools_key(&mut self, key: KeyEvent) -> Action {
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
            KeyCode::Esc => Action::Quit,
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
                    "Type to search. Enter opens a tool; Esc clears search or exits.".into();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> Action {
        let field_count = self.input_values.len();
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::Tools;
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
        Block::default().style(Style::default().bg(app.theme().background)),
        frame.area(),
    );
    match app.screen {
        Screen::Tools => render_tools(frame, app),
        Screen::Form => render_form(frame, app),
        Screen::Running => render_running(frame, app),
        Screen::Result => render_result(frame, app),
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
    render_header(frame, header_area, theme, false);
    frame.render_widget(
        Paragraph::new(format!("Search tools: {}", app.query))
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
            ListItem::new(format!(
                "{}  [{} · {}]",
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
                    .fg(theme.primary)
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
                .border_style(Style::default().fg(theme.secondary)),
        ),
        detail_area,
    );
    render_footer(
        frame,
        footer_area,
        "Tab category · ↑/↓ select · Enter open · Esc back · ^T theme · Ctrl+C quit",
        &app.status,
        theme,
    );
}

fn render_categories(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
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
                    .fg(theme.primary)
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
    render_header(frame, layout[0], theme, false);
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
    render_header(frame, layout[0], theme, true);
    frame.render_widget(
        Paragraph::new(format!(
            "Running {}\n{}",
            app.current_tool().map_or("diagnostic", |tool| tool.name),
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
    );
}

fn render_result(frame: &mut Frame<'_>, app: &App) {
    let theme = app.theme();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    render_header(frame, layout[0], theme, false);
    let result = match &app.completion {
        Completion::None => "No result is available.".to_owned(),
        Completion::Error(error) => format!("Execution error\n{error}"),
        Completion::Result(result) => {
            let mut lines = vec![
                format!("Execution: {}", result.execution_status_label()),
                format!("Tool: {}", result.tool_id),
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
        "↑/↓ scroll · PgUp/PgDn page · Esc edit · Ctrl+T theme · Ctrl+C quit",
        &app.status,
        theme,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, theme: Theme, running: bool) {
    frame.render_widget(
        Paragraph::new(format!(
            "NYRA v{} · {} · offline-first core · sound off · {} · {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            if running { "busy" } else { "idle" },
            theme.short_label
        ))
        .style(
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.secondary)),
        ),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, help: &str, status: &str, theme: Theme) {
    let text = if status.is_empty() {
        help.to_owned()
    } else {
        format!("{help}\n{status}")
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(theme.accent))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Keyboard")
                    .border_style(Style::default().fg(theme.secondary)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
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
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (completion_tx, completion_rx) = mpsc::channel();
    let mut app = App::new();
    let mut cancel_sender: Option<oneshot::Sender<()>> = None;
    let mut worker: Option<JoinHandle<()>> = None;

    loop {
        terminal.draw(|frame| render(frame, &app))?;
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
                        app.status = "Cancellation requested; waiting for the tool to stop.".into();
                    }
                }
                Action::Start => match app.start_input() {
                    Ok(input) => {
                        app.screen = Screen::Running;
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
    }

    if let Some(cancel) = cancel_sender.take() {
        let _ = cancel.send(());
    }
    if let Some(worker) = worker.take() {
        let _ = worker.join();
    }
    Ok(())
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
    fn curated_thai_query_opens_the_tcp_form() {
        let mut app = App::new();
        for character in "เช็คพอร์ต 443".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.selected_tool_id, Some("network.tcp"));
        assert_eq!(app.screen, Screen::Form);
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
    fn theme_shortcut_cycles_the_four_data_driven_palettes() {
        let mut app = App::new();
        for expected in 1..THEMES.len() {
            app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
            assert_eq!(app.theme_index, expected);
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.theme_index, 0);
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
        assert!(text.contains("Next checks: network.dns"));
        assert!(text.contains("JSON:"));
    }
}
