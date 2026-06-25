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
use crate::display::{age_label, bar, bytes};
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
            message: "space: select  /: search  enter: review  1-4: age preset  q: quit"
                .to_string(),
        }
    }

    fn run(
        mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        cleaner: &dyn Cleaner,
    ) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

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
                    KeyCode::Char('a') => self.select_all_visible(),
                    KeyCode::Char('n') => self.selected.clear(),
                    KeyCode::Char('/') => {
                        self.mode = Mode::Search;
                        self.message =
                            "type to fuzzy search  backspace: edit  esc/enter: apply".to_string();
                    }
                    KeyCode::Enter => {
                        self.mode = Mode::Review;
                        self.message =
                            "enter: move selected folders to trash  esc: back  q: quit".to_string();
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
                            "space: select  /: search  enter: review  1-4: age preset  q: quit"
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
                            "space: select  /: search  enter: review  1-4: age preset  q: quit"
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
            Paragraph::new(self.message.as_str()).block(Block::default().borders(Borders::ALL)),
            layout[2],
        );
    }

    fn summary_widget(&self) -> Paragraph<'_> {
        let selected_bytes = self.selected_bytes();
        let visible = self.visible_indices();
        let mut lines = vec![
            Line::from(vec![
                Span::styled("nukeD", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::raw(format!("folders: {}", self.scan.folders.len())),
                Span::raw("  "),
                Span::raw(format!("total: {}", bytes(self.scan.total_size()))),
                Span::raw("  "),
                Span::raw(format!("selected: {}", bytes(selected_bytes))),
                Span::raw("  "),
                Span::raw(format!("visible: {}", visible.len())),
            ]),
            Line::from(vec![
                Span::raw("filter: "),
                Span::styled(
                    if self.filter.is_empty() {
                        "none".to_string()
                    } else {
                        self.filter.clone()
                    },
                    if self.mode == Mode::Search {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ]),
        ];

        for (idx, preset) in AgeThreshold::presets().iter().enumerate() {
            let total = self.scan.total_for(*preset);
            let style = if *preset == self.threshold {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}:{} ", idx + 1, preset), style),
                Span::raw(format!("{:>10} ", bytes(total))),
                Span::raw(bar(total, self.scan.total_size(), 32)),
            ]));
        }

        Paragraph::new(lines).block(Block::default().title("Savings").borders(Borders::ALL))
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
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ],
        )
        .header(
            Row::new(["", "kind", "size", "age", "project", "dependency"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .title("Dependency folders")
                .borders(Borders::ALL),
        )
    }

    fn row_for<'a>(&self, row_idx: usize, folder: &'a DependencyFolder) -> Row<'a> {
        let marker = if self.selected.contains(&folder.path) {
            "[x]"
        } else {
            "[ ]"
        };
        let style = if row_idx == self.cursor {
            Style::default().bg(Color::DarkGray)
        } else if folder.is_older_than(self.threshold) {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        Row::new(vec![
            Cell::from(marker),
            Cell::from(folder.kind.label()),
            Cell::from(bytes(folder.size_bytes)),
            Cell::from(age_label(folder.age)),
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
        if !folder.is_older_than(self.threshold) {
            self.message = format!("{} is newer than {}", folder.path.display(), self.threshold);
            return;
        }
        let path = folder.path.clone();
        if !self.selected.insert(path.clone()) {
            self.selected.remove(&path);
        }
    }

    fn select_all_visible(&mut self) {
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
    }

    fn set_threshold(&mut self, threshold: AgeThreshold) {
        self.threshold = threshold;
        self.select_all_visible();
    }

    fn selected_bytes(&self) -> u64 {
        self.scan
            .folders
            .iter()
            .filter(|folder| self.selected.contains(&folder.path))
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
