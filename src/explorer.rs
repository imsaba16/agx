use crate::config::AntigravityPaths;
use crate::db::Database;
use crate::models::ConversationSummary;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::fs;
use std::io::stdout;
use std::path::PathBuf;

pub struct ExplorerApp<'a> {
    pub paths: &'a AntigravityPaths,
    pub conversations: Vec<ConversationSummary>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub list_state: ListState,
    pub filter_query: String,
    pub in_filter_mode: bool,
    pub status_message: String,
}

impl<'a> ExplorerApp<'a> {
    pub fn new(paths: &'a AntigravityPaths, conversations: Vec<ConversationSummary>) -> Self {
        let indices: Vec<usize> = (0..conversations.len()).collect();
        let mut list_state = ListState::default();
        if !indices.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            paths,
            conversations,
            filtered_indices: indices,
            selected_index: 0,
            list_state,
            filter_query: String::new(),
            in_filter_mode: false,
            status_message: "Ready".to_string(),
        }
    }

    pub fn apply_filter(&mut self) {
        let q = self.filter_query.to_lowercase();
        self.filtered_indices = self
            .conversations
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if q.is_empty() {
                    true
                } else {
                    c.title.to_lowercase().contains(&q)
                        || c.conversation_id.to_lowercase().contains(&q)
                        || c.workspace_uris.to_lowercase().contains(&q)
                }
            })
            .map(|(idx, _)| idx)
            .collect();

        self.selected_index = 0;
        if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    pub fn next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let next_idx = (self.selected_index + 1) % self.filtered_indices.len();
        self.selected_index = next_idx;
        self.list_state.select(Some(next_idx));
    }

    pub fn previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let prev_idx = if self.selected_index == 0 {
            self.filtered_indices.len() - 1
        } else {
            self.selected_index - 1
        };
        self.selected_index = prev_idx;
        self.list_state.select(Some(prev_idx));
    }

    pub fn current_selected_summary(&self) -> Option<&ConversationSummary> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.conversations.get(idx))
    }

    pub fn load_active_preview(&self) -> String {
        if let Some(conv) = self.current_selected_summary() {
            let brain_dir = self.paths.conversation_brain(&conv.conversation_id);
            if brain_dir.exists() {
                let plan_path = brain_dir.join("implementation_plan.md");
                if plan_path.exists() {
                    if let Ok(content) = fs::read_to_string(&plan_path) {
                        return content;
                    }
                }
                let walkthrough_path = brain_dir.join("walkthrough.md");
                if walkthrough_path.exists() {
                    if let Ok(content) = fs::read_to_string(&walkthrough_path) {
                        return content;
                    }
                }
            }
            format!(
                "# {}\n\nNo implementation_plan.md found for this session.\n\nConversation ID: {}\nModified: {}\nSteps: {}",
                conv.title, conv.conversation_id, conv.last_modified_time, conv.step_count
            )
        } else {
            "No conversation selected.".to_string()
        }
    }
}

pub fn run_explorer(paths: &AntigravityPaths) -> Result<()> {
    let db = Database::new(&paths.summaries_db());
    let convs = if paths.summaries_db().exists() {
        db.list_conversations()?
    } else {
        Vec::new()
    };

    if convs.is_empty() {
        println!("No conversations found in Antigravity storage.");
        return Ok(());
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = ExplorerApp::new(paths, convs);

    loop {
        terminal.draw(|f| {
            let size = f.area();

            // Vertical layout: Header (3), Main (rest), Footer (3)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(size);

            // 1. Header
            let header_text = vec![Line::from(vec![
                Span::styled("agx ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("─ Split-Pane Session Explorer & Markdown Inspector", Style::default().fg(Color::DarkGray)),
            ])];
            let header = Paragraph::new(header_text)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(header, chunks[0]);

            // 2. Main split (Left: Sessions list 40%, Right: Markdown preview 60%)
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[1]);

            // Left: List
            let items: Vec<ListItem> = app
                .filtered_indices
                .iter()
                .map(|&orig_idx| {
                    let conv = &app.conversations[orig_idx];
                    let date = if conv.last_modified_time.len() >= 10 {
                        &conv.last_modified_time[..10]
                    } else {
                        &conv.last_modified_time
                    };
                    let title = if conv.title.trim().is_empty() {
                        "Untitled Session"
                    } else {
                        &conv.title
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(format!("[{}] ", date), Style::default().fg(Color::Cyan)),
                            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled(format!("  {} steps", conv.step_count), Style::default().fg(Color::DarkGray)),
                        ]),
                    ])
                })
                .collect();

            let list_title = if app.in_filter_mode {
                format!("Sessions ({}) [Filter: {}_]", app.filtered_indices.len(), app.filter_query)
            } else if !app.filter_query.is_empty() {
                format!("Sessions ({}) [Filtered: {}]", app.filtered_indices.len(), app.filter_query)
            } else {
                format!("Sessions ({})", app.filtered_indices.len())
            };

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(list_title)
                        .borders(Borders::ALL)
                        .border_style(if app.in_filter_mode {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::Cyan)
                        }),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("❯ ");

            f.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

            // Right: Preview
            let preview_content = app.load_active_preview();
            let preview_title = app
                .current_selected_summary()
                .map(|c| format!("Preview: {}", if c.title.trim().is_empty() { "Session" } else { &c.title }))
                .unwrap_or_else(|| "Preview".to_string());

            let preview_widget = Paragraph::new(preview_content)
                .block(Block::default().title(preview_title).borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
                .wrap(Wrap { trim: false });
            f.render_widget(preview_widget, main_chunks[1]);

            // 3. Footer / Status
            let controls = if app.in_filter_mode {
                Line::from(vec![
                    Span::styled("FILTER MODE: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::raw("Type search query, Enter/Esc to confirm"),
                ])
            } else {
                Line::from(vec![
                    Span::styled("[j/k / ↑/↓] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Nav  "),
                    Span::styled("[/] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Filter  "),
                    Span::styled("[x] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Extract Plan  "),
                    Span::styled("[q] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Quit  "),
                    Span::styled(format!("│ {}", app.status_message), Style::default().fg(Color::Green)),
                ])
            };

            let footer = Paragraph::new(controls)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(footer, chunks[2]);
        })?;

        // Handle inputs
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.in_filter_mode {
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc => {
                                app.in_filter_mode = false;
                            }
                            KeyCode::Backspace => {
                                app.filter_query.pop();
                                app.apply_filter();
                            }
                            KeyCode::Char(c) => {
                                app.filter_query.push(c);
                                app.apply_filter();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next();
                                app.status_message = "Ready".to_string();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous();
                                app.status_message = "Ready".to_string();
                            }
                            KeyCode::Char('/') => {
                                app.in_filter_mode = true;
                                app.status_message = "Filter mode active".to_string();
                            }
                            KeyCode::Char('x') => {
                                if let Some(conv) = app.current_selected_summary() {
                                    let brain_dir = app.paths.conversation_brain(&conv.conversation_id);
                                    let plan_file = brain_dir.join("implementation_plan.md");
                                    if plan_file.exists() {
                                        let out_name = format!("{}_plan.md", &conv.conversation_id[..8]);
                                        let out_path = PathBuf::from(&out_name);
                                        if fs::copy(&plan_file, &out_path).is_ok() {
                                            app.status_message = format!("✔ Saved to {:?}", out_name);
                                        } else {
                                            app.status_message = "✖ Failed to copy plan".to_string();
                                        }
                                    } else {
                                        app.status_message = "✖ No plan file exists".to_string();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
