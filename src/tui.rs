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
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::age::AgeThreshold;
use crate::cleanup::Cleaner;
use crate::display::{age_label, bytes, percent, status_label};
use crate::fuzzy::matching_indices;
use crate::scanner::{DependencyFolder, ScanSummary};

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
    selected: HashSet<PathBuf>,
    mode: Mode,
    message: String,
    tick: usize,
}

struct Theme;

impl Theme {
    const BORDER: Color = Color::Rgb(91, 121, 102);
    const CYAN: Color = Color::Rgb(111, 214, 210);
    const GREEN: Color = Color::Rgb(139, 205, 135);
    const AMBER: Color = Color::Rgb(216, 174, 108);
    const MUTED: Color = Color::Rgb(101, 108, 116);
    const TEXT: Color = Color::Rgb(214, 220, 222);
    const DARK: Color = Color::Rgb(32, 38, 42);
}

fn metric_style() -> Style {
    Style::default().fg(Theme::GREEN)
}

fn tui_bar(value: u64, max: u64, width: usize) -> Span<'static> {
    if width == 0 {
        return Span::raw("");
    }

    if max == 0 {
        return Span::styled("░".repeat(width), Style::default().fg(Theme::DARK));
    }

    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    let color = if filled > (width * 2 / 3) {
        Theme::GREEN
    } else if filled > (width / 3) {
        Theme::CYAN
    } else {
        Theme::AMBER
    };

    Span::styled(
        format!("{}{}", "█".repeat(filled), "░".repeat(empty)),
        Style::default().fg(color),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    Search,
    Review,
    Done,
}

impl App {
    fn new(scan: ScanSummary, threshold: AgeThreshold, filter: String) -> Self {
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
            selected,
            mode: Mode::Browse,
            message: "space select  a ready  A all  / search  enter review  1-4 preset  q quit"
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
                    KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
                    KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
                    KeyCode::Char(' ') => self.toggle_current(),
                    KeyCode::Char('a') => self.select_ready_visible(),
                    KeyCode::Char('A') => self.select_all_visible(),
                    KeyCode::Char('n') => self.selected.clear(),
                    KeyCode::Char('/') => {
                        self.mode = Mode::Search;
                        self.message =
                            "type to fuzzy search  backspace: edit  esc/enter: apply".to_string();
                    }
                    KeyCode::Enter => {
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
                        self.message =
                            "space select  a ready  A all  / search  enter review  1-4 preset  q quit"
                                .to_string();
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
                Mode::Review => match key.code {
                    KeyCode::Esc => {
                        self.mode = Mode::Browse;
                        self.message =
                            "space select  a ready  A all  / search  enter review  1-4 preset  q quit"
                                .to_string();
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
                Constraint::Length(8),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        frame.render_widget(self.summary_widget(), layout[0]);
        frame.render_widget(self.table_widget(), layout[1]);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(self.spinner(), Style::default().fg(Theme::CYAN)),
                Span::raw(" "),
                Span::styled(self.message.as_str(), self.footer_style()),
            ]))
            .block(Self::panel_block("keys")),
            layout[2],
        );
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
                        .fg(Theme::CYAN)
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
            let style = if *preset == self.threshold {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}:{:>4} ", idx + 1, preset), style),
                Span::styled(format!("{:>12} ", bytes(total)), style),
                Span::styled(format!("{:>5} ", percent(total, visible_total)), style),
                tui_bar(total, visible_total, 32),
            ]));
        }

        Paragraph::new(lines).block(Self::panel_block("savings"))
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
                    .fg(Theme::CYAN)
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
                .bg(Theme::CYAN)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(Theme::TEXT)
                .bg(Theme::DARK)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(Theme::CYAN)
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
        let visible_len = self.visible_indices().len();
        if visible_len == 0 {
            self.cursor = 0;
            return;
        }
        let len = visible_len as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, len - 1) as usize;
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
            Style::default().fg(Theme::AMBER)
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::App;
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
}
