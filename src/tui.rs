use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs};

use crate::age::AgeThreshold;
use crate::cleanup::Cleaner;
use crate::display::{age_label, bytes, dotted_bar, percent, status_label};
use crate::fuzzy::matching_indices;
use crate::scanner::{DependencyFolder, ScanOptions, ScanSummary, scan_roots};

pub fn run(
    scan: ScanSummary,
    threshold: AgeThreshold,
    initial_filter: String,
    cleaner: &dyn Cleaner,
) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = App::new(scan, threshold, initial_filter).run(&mut terminal, cleaner);
    restore_terminal(&mut terminal)?;
    result
}

struct App {
    scan: ScanSummary,
    threshold: AgeThreshold,
    cursor: usize,
    filter: String,
    roots: Vec<PathBuf>,
    root_cursor: usize,
    root_input: String,
    selected: HashSet<PathBuf>,
    mode: Mode,
    tab: Tab,
    message: String,
    tick: usize,
}

struct Theme;

impl Theme {
    const RED: Color = Color::Rgb(190, 92, 86);
    const GREEN: Color = Color::Rgb(128, 168, 126);
    const AMBER: Color = Color::Rgb(196, 155, 92);
    const MINT: Color = Color::Rgb(119, 181, 168);
    const TEXT: Color = Color::Rgb(210, 211, 204);
    const MUTED: Color = Color::Rgb(116, 124, 121);
    const DARK: Color = Color::Rgb(48, 52, 55);
    const BORDER: Color = Color::Rgb(87, 116, 96);
}

fn metric_style() -> Style {
    Style::default().fg(Theme::GREEN)
}

fn tui_bar(value: u64, max: u64, width: usize) -> Span<'static> {
    if width == 0 {
        return Span::raw("");
    }

    let ratio = if max == 0 {
        0.0
    } else {
        value as f64 / max as f64
    };
    let color = if ratio >= 0.75 {
        Theme::GREEN
    } else if ratio > 0.0 {
        Theme::MINT
    } else {
        Theme::MUTED
    };

    Span::styled(
        dotted_bar(value, max, width, ':', '.'),
        Style::default().fg(color),
    )
}

fn savings_meter_line(
    key: usize,
    label: &str,
    value: u64,
    max: u64,
    is_active: bool,
) -> Line<'static> {
    let (preset, value_text, percent_text) = savings_meter_columns(key, label, value, max);
    let style = if is_active {
        Style::default()
            .fg(Theme::MINT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::MUTED)
    };

    Line::from(vec![
        Span::styled(preset, style),
        Span::raw("    "),
        Span::styled(value_text, style),
        Span::raw("    "),
        Span::styled(percent_text, style),
        Span::raw("    "),
        tui_bar(value, max, 32),
    ])
}

fn savings_meter_columns(
    key: usize,
    label: &str,
    value: u64,
    max: u64,
) -> (String, String, String) {
    (
        format!("{key}: {label:<3}"),
        format!("{:>12}", bytes(value)),
        format!("{:>4}", percent(value, max)),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    Search,
    RootInput,
    Review,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Scan,
    Folders,
    Review,
    Help,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Scan, Self::Folders, Self::Review, Self::Help];

    const fn title(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Folders => "folders",
            Self::Review => "review",
            Self::Help => "help",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Scan => 0,
            Self::Folders => 1,
            Self::Review => 2,
            Self::Help => 3,
        }
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl App {
    fn new(scan: ScanSummary, threshold: AgeThreshold, filter: String) -> Self {
        let roots = scan.roots.clone();
        let selected = scan
            .selected_for(threshold)
            .into_iter()
            .map(|folder| folder.path.clone())
            .collect();
        Self {
            scan,
            threshold,
            cursor: 0,
            filter,
            roots,
            root_cursor: 0,
            root_input: String::new(),
            selected,
            mode: Mode::Browse,
            tab: Tab::Folders,
            message: "r scan  tab switch  space select  / search  enter review  ? help  q quit"
                .to_string(),
            tick: 0,
        }
    }

    fn run(
        mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        cleaner: &dyn Cleaner,
    ) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            self.tick = self.tick.wrapping_add(1);

            if self.mode == Mode::Done {
                if event::poll(Duration::from_millis(250))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            break;
                        }
                    }
                }
                continue;
            }

            if !event::poll(Duration::from_millis(250))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match self.mode {
                Mode::Browse => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.next_tab(),
                    KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.previous_tab(),
                    KeyCode::Char('?') => {
                        self.tab = Tab::Help;
                        self.message = "tab switch  r scan  q quit".to_string();
                    }
                    KeyCode::Char('r') => self.rescan(),
                    KeyCode::Char('+') if self.tab == Tab::Scan => {
                        self.mode = Mode::RootInput;
                        self.root_input.clear();
                        self.message = "type root path  enter add+scan  esc cancel".to_string();
                    }
                    KeyCode::Char('d') if self.tab == Tab::Scan => self.remove_current_root(),
                    KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
                    KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
                    KeyCode::Char(' ') if self.tab == Tab::Folders => self.toggle_current(),
                    KeyCode::Char('a') if self.tab == Tab::Folders => self.select_ready_visible(),
                    KeyCode::Char('A') if self.tab == Tab::Folders => self.select_all_visible(),
                    KeyCode::Char('n') if self.tab == Tab::Folders => self.selected.clear(),
                    KeyCode::Char('/') => {
                        self.mode = Mode::Search;
                        self.message =
                            "type to fuzzy search  backspace: edit  esc/enter: apply".to_string();
                    }
                    KeyCode::Enter => {
                        self.tab = Tab::Review;
                        self.mode = Mode::Review;
                        self.message = self.review_message();
                    }
                    KeyCode::Char('1') => self.set_threshold(AgeThreshold::days(7)),
                    KeyCode::Char('2') => self.set_threshold(AgeThreshold::days(30)),
                    KeyCode::Char('3') => self.set_threshold(AgeThreshold::days(90)),
                    KeyCode::Char('4') => self.set_threshold(AgeThreshold::days(365)),
                    _ => {}
                },
                Mode::Search => match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.mode = Mode::Browse;
                        self.clamp_cursor();
                        self.message = self.browse_message();
                    }
                    KeyCode::Backspace => {
                        self.filter.pop();
                        self.clamp_cursor();
                    }
                    KeyCode::Char(ch) => {
                        self.filter.push(ch);
                        self.clamp_cursor();
                    }
                    _ => {}
                },
                Mode::RootInput => match key.code {
                    KeyCode::Esc => {
                        self.mode = Mode::Browse;
                        self.root_input.clear();
                        self.message = self.browse_message();
                    }
                    KeyCode::Enter => self.add_root_from_input(),
                    KeyCode::Backspace => {
                        self.root_input.pop();
                    }
                    KeyCode::Char(ch) => {
                        self.root_input.push(ch);
                    }
                    _ => {}
                },
                Mode::Review => match key.code {
                    KeyCode::Esc => {
                        self.mode = Mode::Browse;
                        self.message = self.browse_message();
                    }
                    KeyCode::Char('q') => break,
                    KeyCode::Enter => self.trash_selected(cleaner),
                    _ => {}
                },
                Mode::Done => {}
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(8),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        frame.render_widget(self.tabs_widget(), layout[0]);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(64), Constraint::Length(32)])
            .split(layout[1]);

        frame.render_widget(self.summary_widget(), top[0]);
        frame.render_widget(self.radar_widget(), top[1]);
        match self.tab {
            Tab::Scan => frame.render_widget(self.scan_widget(), layout[2]),
            Tab::Folders => frame.render_widget(self.table_widget(), layout[2]),
            Tab::Review => frame.render_widget(self.review_widget(), layout[2]),
            Tab::Help => frame.render_widget(self.help_widget(), layout[2]),
        }
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(self.spinner(), Style::default().fg(Theme::MINT)),
                Span::raw(" "),
                Span::styled(self.message.as_str(), self.footer_style()),
            ]))
            .block(Self::panel_block("keys")),
            layout[3],
        );
    }

    fn tabs_widget(&self) -> Tabs<'static> {
        let titles = Tab::ALL
            .into_iter()
            .map(|tab| Line::from(format!(" {} ", tab.title())))
            .collect::<Vec<_>>();

        Tabs::new(titles)
            .select(self.tab.index())
            .style(Style::default().fg(Theme::MUTED))
            .highlight_style(
                Style::default()
                    .fg(Theme::MINT)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::styled("│", Style::default().fg(Theme::BORDER)))
            .block(Self::panel_block("nukeD"))
    }

    fn summary_widget(&self) -> Paragraph<'_> {
        let selected_bytes = self.selected_bytes();
        let visible = self.visible_indices();
        let visible_total = self.total_for_visible(None);
        let selected_newer = self.selected_newer_count();
        let eligible_visible = visible
            .iter()
            .filter(|idx| self.scan.folders[**idx].is_older_than(self.threshold))
            .count();
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "nukeD",
                    Style::default()
                        .fg(Theme::MINT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("detected {}", self.scan.folders.len()),
                    metric_style(),
                ),
                Span::raw("  "),
                Span::styled(format!("visible {}", visible.len()), metric_style()),
                Span::raw("  "),
                Span::styled(format!("ready {}", eligible_visible), metric_style()),
                Span::raw("  "),
                Span::styled(
                    format!("selected {}", bytes(selected_bytes)),
                    metric_style(),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("manual {}", selected_newer),
                    if selected_newer > 0 {
                        Style::default()
                            .fg(Theme::AMBER)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        metric_style()
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled("filter ", Style::default().fg(Theme::MUTED)),
                Span::styled(
                    if self.filter.is_empty() {
                        "none".to_string()
                    } else {
                        self.filter.clone()
                    },
                    if self.mode == Mode::Search {
                        Style::default()
                            .fg(Theme::AMBER)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Theme::TEXT)
                    },
                ),
                Span::styled("  total ", Style::default().fg(Theme::MUTED)),
                Span::styled(bytes(visible_total), metric_style()),
            ]),
        ];

        for (idx, preset) in AgeThreshold::presets().iter().enumerate() {
            let total = self.total_for_visible(Some(*preset));
            lines.push(savings_meter_line(
                idx + 1,
                &preset.label(),
                total,
                visible_total,
                *preset == self.threshold,
            ));
        }

        Paragraph::new(lines).block(Self::panel_block("savings"))
    }

    fn radar_widget(&self) -> Paragraph<'_> {
        let pulse_width = 18;
        let pulse = self.tick % pulse_width;
        let mut scanline = String::with_capacity(pulse_width);
        for idx in 0..pulse_width {
            scanline.push(if idx == pulse {
                ':'
            } else if idx.abs_diff(pulse) <= 2 {
                '.'
            } else {
                ' '
            });
        }

        let selected = self.selected.len();
        let reclaiming = bytes(self.selected_bytes());
        let visible = self.visible_indices().len();

        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("roots ", Style::default().fg(Theme::MUTED)),
                Span::styled(self.roots.len().to_string(), metric_style()),
                Span::styled("  visible ", Style::default().fg(Theme::MUTED)),
                Span::styled(visible.to_string(), metric_style()),
            ]),
            Line::from(vec![Span::styled(
                format!("[{scanline}]"),
                Style::default().fg(Theme::MINT),
            )]),
            Line::from(vec![
                Span::styled("ready ", Style::default().fg(Theme::MUTED)),
                tui_bar(
                    self.total_for_visible(Some(self.threshold)),
                    self.total_for_visible(None),
                    18,
                ),
            ]),
            Line::from(vec![
                Span::styled("sel ", Style::default().fg(Theme::MUTED)),
                Span::styled(selected.to_string(), metric_style()),
            ]),
            Line::from(vec![
                Span::styled("size ", Style::default().fg(Theme::MUTED)),
                Span::styled(reclaiming, metric_style()),
            ]),
        ])
        .block(Self::panel_block("radar"))
    }

    fn scan_widget(&self) -> Table<'_> {
        let mut rows = Vec::new();
        if self.mode == Mode::RootInput {
            rows.push(
                Row::new(vec![
                    Cell::from("+"),
                    Cell::from("new"),
                    Cell::from(if self.root_input.is_empty() {
                        "type a root path".to_string()
                    } else {
                        self.root_input.clone()
                    }),
                ])
                .style(
                    Style::default()
                        .fg(Theme::AMBER)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        }

        rows.extend(self.roots.iter().enumerate().map(|(idx, root)| {
            let style = if idx == self.root_cursor {
                Style::default()
                    .fg(Theme::TEXT)
                    .bg(Theme::DARK)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };
            Row::new(vec![
                Cell::from(if idx == self.root_cursor { ">" } else { " " }),
                Cell::from("active"),
                Cell::from(root.display().to_string()),
            ])
            .style(style)
        }));

        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(8),
                Constraint::Percentage(90),
            ],
        )
        .header(
            Row::new(["", "state", "root"]).style(
                Style::default()
                    .fg(Theme::MINT)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Self::panel_block("scan roots  r rescan  + add  d remove"))
    }

    fn review_widget(&self) -> Paragraph<'_> {
        let lines = vec![
            Line::from(vec![
                Span::styled("selected ", Style::default().fg(Theme::MUTED)),
                Span::styled(self.selected.len().to_string(), metric_style()),
                Span::styled("  reclaim ", Style::default().fg(Theme::MUTED)),
                Span::styled(bytes(self.selected_bytes()), metric_style()),
                Span::styled("  manual/newer ", Style::default().fg(Theme::MUTED)),
                Span::styled(
                    self.selected_newer_count().to_string(),
                    if self.selected_newer_count() > 0 {
                        Style::default()
                            .fg(Theme::AMBER)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        metric_style()
                    },
                ),
            ]),
            Line::from(""),
            Line::from("enter moves selected dependency folders to the OS trash"),
            Line::from("esc returns to folders without changing anything"),
            Line::from("q quits"),
            Line::from(""),
            Line::from(self.review_message()),
        ];

        Paragraph::new(lines).block(Self::panel_block("review"))
    }

    fn help_widget(&self) -> Paragraph<'_> {
        Paragraph::new(vec![
            Line::from("tab/right/l: next tab    shift-tab/left/h: previous tab"),
            Line::from("r: rescan current roots  /: fuzzy search  1-4: age presets"),
            Line::from("space: toggle row        a: select ready  A: select all visible"),
            Line::from("n: clear selection       enter: review    q: quit"),
            Line::from("scan tab: + add root     d: remove root   j/k: move"),
            Line::from("search and root input: enter apply, esc cancel"),
        ])
        .block(Self::panel_block("help"))
    }

    fn table_widget(&self) -> Table<'_> {
        let visible = self.visible_indices();
        let rows = visible
            .into_iter()
            .enumerate()
            .map(|(row_idx, folder_idx)| self.row_for(row_idx, &self.scan.folders[folder_idx]));

        Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(7),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ],
        )
        .header(
            Row::new(["", "kind", "size", "age", "status", "project", "dependency"]).style(
                Style::default()
                    .fg(Theme::MINT)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Self::panel_block("dependencies"))
    }

    fn row_for<'a>(&self, row_idx: usize, folder: &'a DependencyFolder) -> Row<'a> {
        let marker = if self.selected.contains(&folder.path) {
            "[x]"
        } else {
            "[ ]"
        };
        let is_current = row_idx == self.cursor;
        let is_selected = self.selected.contains(&folder.path);
        let is_eligible = folder.is_older_than(self.threshold);
        let style = if is_current && is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Theme::MINT)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(Theme::TEXT)
                .bg(Theme::DARK)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(Theme::MINT)
                .add_modifier(Modifier::BOLD)
        } else if is_eligible {
            Style::default().fg(Theme::TEXT)
        } else {
            Style::default().fg(Theme::MUTED)
        };

        Row::new(vec![
            Cell::from(marker),
            Cell::from(folder.kind.label()),
            Cell::from(bytes(folder.size_bytes)),
            Cell::from(age_label(folder.age)),
            Cell::from(status_label(is_eligible)),
            Cell::from(folder.project_path.display().to_string()),
            Cell::from(folder.path.display().to_string()),
        ])
        .style(style)
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.tab == Tab::Scan {
            if self.roots.is_empty() {
                self.root_cursor = 0;
                return;
            }
            let len = self.roots.len() as isize;
            self.root_cursor = (self.root_cursor as isize + delta).clamp(0, len - 1) as usize;
        } else {
            let visible_len = self.visible_indices().len();
            if visible_len == 0 {
                self.cursor = 0;
                return;
            }
            let len = visible_len as isize;
            self.cursor = (self.cursor as isize + delta).clamp(0, len - 1) as usize;
        }
    }

    fn toggle_current(&mut self) {
        let Some(folder) = self.current_folder() else {
            return;
        };
        let path = folder.path.clone();
        let is_eligible = folder.is_older_than(self.threshold);
        let path_display = folder.path.display().to_string();
        if !self.selected.insert(path.clone()) {
            self.selected.remove(&path);
            self.message = format!("unselected {path_display}");
        } else if is_eligible {
            self.message = format!("selected {path_display}");
        } else {
            self.message = format!("selected newer item manually: {path_display}");
        }
    }

    fn select_ready_visible(&mut self) {
        let visible = self.visible_indices();
        self.selected = self
            .scan
            .folders
            .iter()
            .enumerate()
            .filter(|(idx, _)| visible.contains(idx))
            .map(|(_, folder)| folder)
            .filter(|folder| folder.is_older_than(self.threshold))
            .map(|folder| folder.path.clone())
            .collect();
        self.message = format!("selected {} ready visible item(s)", self.selected.len());
    }

    fn select_all_visible(&mut self) {
        let visible = self.visible_indices();
        self.selected = self
            .scan
            .folders
            .iter()
            .enumerate()
            .filter(|(idx, _)| visible.contains(idx))
            .map(|(_, folder)| folder.path.clone())
            .collect();
        let selected_newer = self.selected_newer_count();
        self.message = format!(
            "selected all {} visible item(s); {} newer/manual",
            self.selected.len(),
            selected_newer
        );
    }

    fn set_threshold(&mut self, threshold: AgeThreshold) {
        self.threshold = threshold;
        self.select_ready_visible();
    }

    fn browse_message(&self) -> String {
        match self.tab {
            Tab::Scan => "r scan  + add root  d remove root  tab switch  / filter  q quit",
            Tab::Folders => {
                "r scan  space select  a ready  A all  / search  enter review  ? help  q quit"
            }
            Tab::Review => "enter trash selected  esc back  tab switch  q quit",
            Tab::Help => "tab switch  r scan  q quit",
        }
        .to_string()
    }

    fn next_tab(&mut self) {
        self.tab = self.tab.next();
        self.mode = Mode::Browse;
        self.message = self.browse_message();
    }

    fn previous_tab(&mut self) {
        self.tab = self.tab.previous();
        self.mode = Mode::Browse;
        self.message = self.browse_message();
    }

    fn rescan(&mut self) {
        if self.roots.is_empty() {
            self.message = "add at least one root before scanning".to_string();
            return;
        }

        self.message = "scanning roots".to_string();
        match scan_roots(&self.roots, ScanOptions::default()) {
            Ok(scan) => {
                self.scan = scan;
                self.selected.clear();
                self.cursor = 0;
                self.clamp_cursor();
                self.message = format!("scanned {} folder(s)", self.scan.folders.len());
            }
            Err(err) => {
                self.message = format!("scan failed: {err}");
            }
        }
    }

    fn add_root_from_input(&mut self) {
        let raw = self.root_input.trim();
        if raw.is_empty() {
            self.message = "root path is empty".to_string();
            return;
        }

        let root = PathBuf::from(expand_home(raw));
        if !self.roots.contains(&root) {
            self.roots.push(root);
            self.root_cursor = self.roots.len().saturating_sub(1);
        }
        self.root_input.clear();
        self.mode = Mode::Browse;
        self.rescan();
    }

    fn remove_current_root(&mut self) {
        if self.roots.len() <= 1 {
            self.message = "keep at least one scan root".to_string();
            return;
        }

        if self.root_cursor < self.roots.len() {
            let removed = self.roots.remove(self.root_cursor);
            if self.root_cursor >= self.roots.len() {
                self.root_cursor = self.roots.len().saturating_sub(1);
            }
            self.rescan();
            self.message = format!(
                "removed root {}; scanned {} folder(s)",
                removed.display(),
                self.scan.folders.len()
            );
        }
    }

    fn selected_bytes(&self) -> u64 {
        self.scan
            .folders
            .iter()
            .filter(|folder| self.selected.contains(&folder.path))
            .map(|folder| folder.size_bytes)
            .sum()
    }

    fn selected_newer_count(&self) -> usize {
        self.scan
            .folders
            .iter()
            .filter(|folder| {
                self.selected.contains(&folder.path) && !folder.is_older_than(self.threshold)
            })
            .count()
    }

    fn review_message(&self) -> String {
        let selected_count = self.selected.len();
        let newer_count = self.selected_newer_count();
        if selected_count == 0 {
            "nothing selected  esc back  q quit".to_string()
        } else if newer_count > 0 {
            format!(
                "enter trash {} selected; warning {} newer/manual  esc back  q quit",
                selected_count, newer_count
            )
        } else {
            format!(
                "enter trash {} selected item(s)  esc back  q quit",
                selected_count
            )
        }
    }

    fn footer_style(&self) -> Style {
        if self.selected_newer_count() > 0 || self.message.contains("warning") {
            Style::default().fg(Theme::RED)
        } else {
            Style::default().fg(Theme::TEXT)
        }
    }

    fn spinner(&self) -> &'static str {
        match self.tick % 4 {
            0 => "-",
            1 => "\\",
            2 => "|",
            _ => "/",
        }
    }

    fn panel_block(title: &'static str) -> Block<'static> {
        Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER))
    }

    fn total_for_visible(&self, threshold: Option<AgeThreshold>) -> u64 {
        self.visible_indices()
            .into_iter()
            .map(|idx| &self.scan.folders[idx])
            .filter(|folder| threshold.is_none_or(|threshold| folder.is_older_than(threshold)))
            .map(|folder| folder.size_bytes)
            .sum()
    }

    fn visible_indices(&self) -> Vec<usize> {
        matching_indices(&self.scan.folders, &self.filter)
    }

    fn current_folder(&self) -> Option<&DependencyFolder> {
        let visible = self.visible_indices();
        let folder_idx = visible.get(self.cursor)?;
        self.scan.folders.get(*folder_idx)
    }

    fn clamp_cursor(&mut self) {
        let visible_len = self.visible_indices().len();
        if visible_len == 0 {
            self.cursor = 0;
        } else if self.cursor >= visible_len {
            self.cursor = visible_len - 1;
        }
    }

    fn trash_selected(&mut self, cleaner: &dyn Cleaner) {
        let selected: Vec<PathBuf> = self.selected.iter().cloned().collect();
        if selected.is_empty() {
            self.message = "nothing selected".to_string();
            return;
        }

        let mut moved = 0usize;
        let mut errors = Vec::new();
        for path in selected {
            match cleaner.trash(&path) {
                Ok(()) => moved += 1,
                Err(err) => errors.push(format!("{}: {err}", path.display())),
            }
        }

        self.mode = Mode::Done;
        self.message = if errors.is_empty() {
            format!("moved {moved} folder(s) to trash. press any key to exit")
        } else {
            format!(
                "moved {moved}; {} failed. first error: {}. press any key to exit",
                errors.len(),
                errors[0]
            )
        };
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn expand_home(raw: &str) -> String {
    if raw == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| raw.to_string());
    }

    if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }

    raw.to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::{App, savings_meter_columns};
    use crate::age::AgeThreshold;
    use crate::scanner::{DependencyFolder, DependencyKind, ScanSummary};

    fn folder(name: &str, age_days: u64) -> DependencyFolder {
        let path = PathBuf::from(format!("/tmp/{name}/node_modules"));
        DependencyFolder {
            path: path.clone(),
            project_path: path.parent().unwrap().to_path_buf(),
            kind: DependencyKind::Node,
            size_bytes: 100,
            project_modified: SystemTime::UNIX_EPOCH,
            age: Duration::from_secs(age_days * 86_400),
        }
    }

    fn app() -> App {
        App::new(
            ScanSummary {
                roots: vec![PathBuf::from("/tmp")],
                folders: vec![folder("ready", 10), folder("newer", 1)],
            },
            AgeThreshold::days(7),
            String::new(),
        )
    }

    #[test]
    fn manual_toggle_selects_ready_row() {
        let mut app = app();
        app.selected.clear();
        app.cursor = 0;

        app.toggle_current();

        assert!(
            app.selected
                .contains(&PathBuf::from("/tmp/ready/node_modules"))
        );
    }

    #[test]
    fn manual_toggle_selects_newer_row() {
        let mut app = app();
        app.selected.clear();
        app.cursor = 1;

        app.toggle_current();

        assert!(
            app.selected
                .contains(&PathBuf::from("/tmp/newer/node_modules"))
        );
        assert!(app.message.contains("newer item"));
    }

    #[test]
    fn select_ready_visible_skips_newer_rows() {
        let mut app = app();
        app.selected.clear();

        app.select_ready_visible();

        assert!(
            app.selected
                .contains(&PathBuf::from("/tmp/ready/node_modules"))
        );
        assert!(
            !app.selected
                .contains(&PathBuf::from("/tmp/newer/node_modules"))
        );
    }

    #[test]
    fn select_all_visible_includes_newer_rows() {
        let mut app = app();
        app.selected.clear();

        app.select_all_visible();

        assert!(
            app.selected
                .contains(&PathBuf::from("/tmp/ready/node_modules"))
        );
        assert!(
            app.selected
                .contains(&PathBuf::from("/tmp/newer/node_modules"))
        );
    }

    #[test]
    fn rescan_uses_current_roots_and_clears_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("app");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::create_dir(project.join("node_modules")).unwrap();

        let mut app = App::new(
            ScanSummary {
                roots: vec![tmp.path().to_path_buf()],
                folders: Vec::new(),
            },
            AgeThreshold::days(7),
            String::new(),
        );
        app.selected
            .insert(PathBuf::from("/tmp/stale/node_modules"));

        app.rescan();

        assert_eq!(app.scan.folders.len(), 1);
        assert!(app.selected.is_empty());
        assert!(app.message.contains("scanned 1"));
    }

    #[test]
    fn remove_current_root_keeps_at_least_one_root() {
        let mut app = app();

        app.remove_current_root();

        assert_eq!(app.roots.len(), 1);
        assert!(app.message.contains("keep at least one"));
    }

    #[test]
    fn savings_meter_columns_stay_fixed_width() {
        let rows = [
            savings_meter_columns(1, "7d", 581_890_000, 581_890_000),
            savings_meter_columns(2, "30d", 186_990_000, 581_890_000),
            savings_meter_columns(3, "90d", 0, 581_890_000),
            savings_meter_columns(4, "1y", 0, 581_890_000),
        ];

        assert!(
            rows.iter()
                .all(|(preset, _, _)| preset.chars().count() == 6)
        );
        assert!(rows.iter().all(|(_, value, _)| value.chars().count() == 12));
        assert!(
            rows.iter()
                .all(|(_, _, percent)| percent.chars().count() == 4)
        );
    }
}
