mod app;
mod gh;
mod ui;

use anyhow::Result;
use app::{App, AppEvent, View};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table, Tabs},
    Frame, Terminal,
};
use std::{io, time::{Duration, Instant}};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel(32);
    let tick_rate = Duration::from_millis(250);

    // Event handler task
    let event_tx = tx.clone();
    tokio::spawn(async move {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll failed") {
                if let Event::Key(key) = event::read().expect("read failed") {
                    if event_tx.send(AppEvent::Key(key)).await.is_err() {
                        break;
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if event_tx.send(AppEvent::Tick).await.is_err() {
                    break;
                }
                last_tick = Instant::now();
            }
        }
    });

    // Initial data fetch
    let fetch_tx = tx.clone();
    tokio::spawn(async move {
        match gh::list_issues(None).await {
            Ok(issues) => { let _ = fetch_tx.send(AppEvent::IssuesLoaded(issues)).await; },
            Err(e) => { let _ = fetch_tx.send(AppEvent::Error(e.to_string())).await; },
        }
    });

    let mut app = App::new();
    app.loading = true;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Tick => {}
                AppEvent::Key(key) => {
                    if app.error.is_some() {
                        app.error = None;
                        continue;
                    }
                    if app.show_help {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => app.show_help = false,
                            _ => {}
                        }
                    } else if app.show_create_form {
                        match key.code {
                            KeyCode::Esc => app.show_create_form = false,
                            KeyCode::Tab => {
                                app.form_focus = match app.form_focus {
                                    app::FormFocus::Title => app::FormFocus::Body,
                                    app::FormFocus::Body => app::FormFocus::Title,
                                };
                            }
                            KeyCode::Enter => {
                                if !app.form_title.is_empty() {
                                    let title = app.form_title.clone();
                                    let body = app.form_body.clone();
                                    let context = app.context.clone();
                                    let view = app.current_view;
                                    let res_tx = tx.clone();
                                    
                                    app.show_create_form = false;
                                    app.loading = true;

                                    tokio::spawn(async move {
                                        let res = match view {
                                            View::Issues => gh::create_issue(&title, &body, &[], context.as_deref()).await,
                                            View::PullRequests => gh::create_pr(&title, &body, "main", context.as_deref()).await,
                                            _ => Ok(()),
                                        };
                                        match res {
                                            Ok(_) => {
                                                let _ = res_tx.send(AppEvent::Success(format!("{:?} Created!", view))).await;
                                            }
                                            Err(e) => { let _ = res_tx.send(AppEvent::Error(e.to_string())).await; },
                                        }
                                    });
                                }
                            }
                            KeyCode::Char(c) => {
                                match app.form_focus {
                                    app::FormFocus::Title => app.form_title.push(c),
                                    app::FormFocus::Body => app.form_body.push(c),
                                }
                            }
                            KeyCode::Backspace => {
                                match app.form_focus {
                                    app::FormFocus::Title => { app.form_title.pop(); },
                                    app::FormFocus::Body => { app.form_body.pop(); },
                                }
                            }
                            _ => {}
                        }
                    } else if app.input_mode == app::InputMode::Editing {
                        match key.code {
                            KeyCode::Enter => {
                                app.context = if app.input.is_empty() { None } else { Some(app.input.clone()) };
                                app.input_mode = app::InputMode::Normal;
                                app.loading = true;
                                fetch_data_for_view(&app, tx.clone());
                            }
                            KeyCode::Char(c) => {
                                app.input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.input.pop();
                            }
                            KeyCode::Esc => {
                                app.input_mode = app::InputMode::Normal;
                            }
                            _ => {}
                        }
                    } else if app.show_detail {
                        match key.code {
                            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q') => app.show_detail = false,
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('?') => app.show_help = true,
                            KeyCode::Char('n') => {
                                if app.current_view != View::Projects {
                                    app.show_create_form = true;
                                    app.form_title = String::new();
                                    app.form_body = String::new();
                                    app.form_focus = app::FormFocus::Title;
                                }
                            }
                            KeyCode::Char('c') => {
                                app.input_mode = app::InputMode::Editing;
                                app.input = app.context.clone().unwrap_or_default();
                            }
                            KeyCode::Enter => {
                                if !app.loading {
                                    app.show_detail = true;
                                }
                            }
                            KeyCode::Tab => {
                                app.next_view();
                                fetch_data_for_view(&app, tx.clone());
                            }
                            KeyCode::Char('j') | KeyCode::Down => app.select_next(),
                            KeyCode::Char('k') | KeyCode::Up => app.select_previous(),
                            KeyCode::Char('r') => {
                                app.loading = true;
                                fetch_data_for_view(&app, tx.clone());
                            }
                            _ => {}
                        }
                    }
                }
                AppEvent::IssuesLoaded(issues) => {
                    app.issues = issues;
                    app.loading = false;
                    if !app.issues.is_empty() && app.issues_state.selected().is_none() {
                        app.issues_state.select(Some(0));
                    }
                }
                AppEvent::PRsLoaded(prs) => {
                    app.prs = prs;
                    app.loading = false;
                    if !app.prs.is_empty() && app.prs_state.selected().is_none() {
                        app.prs_state.select(Some(0));
                    }
                }
                AppEvent::ProjectsLoaded(projects) => {
                    app.projects = projects;
                    app.loading = false;
                    if !app.projects.is_empty() && app.projects_state.selected().is_none() {
                        app.projects_state.select(Some(0));
                    }
                }
                AppEvent::Success(_msg) => {
                    app.loading = false;
                    // Refresh data after success
                    fetch_data_for_view(&app, tx.clone());
                }
                AppEvent::Error(e) => {
                    app.error = Some(e);
                    app.loading = false;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn fetch_data_for_view(app: &App, tx: mpsc::Sender<AppEvent>) {
    let tx = tx.clone();
    let context = app.context.clone();
    match app.current_view {
        View::Issues => {
            tokio::spawn(async move {
                match gh::list_issues(context.as_deref()).await {
                    Ok(issues) => { let _ = tx.send(AppEvent::IssuesLoaded(issues)).await; },
                    Err(e) => { let _ = tx.send(AppEvent::Error(e.to_string())).await; },
                }
            });
        }
        View::PullRequests => {
            tokio::spawn(async move {
                match gh::list_prs(context.as_deref()).await {
                    Ok(prs) => { let _ = tx.send(AppEvent::PRsLoaded(prs)).await; },
                    Err(e) => { let _ = tx.send(AppEvent::Error(e.to_string())).await; },
                }
            });
        }
        View::Projects => {
            tokio::spawn(async move {
                match gh::list_projects(context.as_deref()).await {
                    Ok(projects) => { let _ = tx.send(AppEvent::ProjectsLoaded(projects)).await; },
                    Err(e) => { let _ = tx.send(AppEvent::Error(e.to_string())).await; },
                }
            });
        }
    }
}

use ratatui::widgets::Clear;

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(if app.input_mode == app::InputMode::Editing { 3 } else { 0 }),
        ].as_ref())
        .split(size);

    let area = main_chunks[0];

    if app.show_detail {
        render_detail(f, app, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(area);

        let context_str = app.context.as_deref().unwrap_or("Local Repo");
        let titles = vec!["Issues", "Pull Requests", "Projects"];
        let tabs = Tabs::new(titles)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(format!("Ghub TUI - Context: {}", context_str)))
            .select(match app.current_view {
                View::Issues => 0,
                View::PullRequests => 1,
                View::Projects => 2,
            })
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(tabs, chunks[0]);

        match app.current_view {
            View::Issues => render_issues(f, app, chunks[1]),
            View::PullRequests => render_prs(f, app, chunks[1]),
            View::Projects => render_projects(f, app, chunks[1]),
        }
    }

    if app.input_mode == app::InputMode::Editing {
        let input = Paragraph::new(app.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Enter Context (Org or Repo)"));
        f.render_widget(input, main_chunks[1]);
    }

    if app.show_help {
        render_help(f, size);
    }

    if app.show_create_form {
        render_create_form(f, app, size);
    }

    if app.loading {
        render_loading(f, size);
    }

    if app.error.is_some() {
        render_error(f, app, size);
    }
}

fn render_error(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    if let Some(ref err) = app.error {
        let area = render_pop_up(area, 60, 40);
        f.render_widget(Clear, area);
        
        let p = Paragraph::new(format!("Error: {}\n\nPress any key to close", err))
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }
}

fn render_pop_up(area: ratatui::layout::Rect, percent_x: u16, percent_y: u16) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let area = render_pop_up(area, 60, 60);
    f.render_widget(Clear, area);
    let help_text = vec![
        Line::from("Ghub TUI Shortcuts"),
        Line::from(""),
        Line::from(vec![Span::styled("Tab: ", Style::default().fg(Color::Yellow)), Span::raw("Switch Views (Issues/PRs/Projects)")]),
        Line::from(vec![Span::styled("j/k: ", Style::default().fg(Color::Yellow)), Span::raw("Navigation")]),
        Line::from(vec![Span::styled("Enter: ", Style::default().fg(Color::Yellow)), Span::raw("View Details")]),
        Line::from(vec![Span::styled("c: ", Style::default().fg(Color::Yellow)), Span::raw("Change Context (Org/Repo)")]),
        Line::from(vec![Span::styled("n: ", Style::default().fg(Color::Yellow)), Span::raw("New Issue/PR")]),
        Line::from(vec![Span::styled("r: ", Style::default().fg(Color::Yellow)), Span::raw("Refresh Data")]),
        Line::from(vec![Span::styled("?: ", Style::default().fg(Color::Yellow)), Span::raw("Show Help")]),
        Line::from(vec![Span::styled("q/Esc: ", Style::default().fg(Color::Yellow)), Span::raw("Quit/Close")]),
    ];

    let p = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(p, area);
}

fn render_create_form(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let area = render_pop_up(area, 80, 80);
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Instructions
        ].as_ref())
        .split(area);

    let title_style = if let app::FormFocus::Title = app.form_focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let body_style = if let app::FormFocus::Body = app.form_focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let title_input = Paragraph::new(app.form_title.as_str())
        .style(title_style)
        .block(Block::default().borders(Borders::ALL).title("Title"));
    f.render_widget(title_input, chunks[0]);

    let body_input = Paragraph::new(app.form_body.as_str())
        .style(body_style)
        .block(Block::default().borders(Borders::ALL).title("Body (Markdown)"));
    f.render_widget(body_input, chunks[1]);

    let instructions = Paragraph::new("Tab: Switch Focus | Enter: Submit | Esc: Cancel")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(instructions, chunks[2]);

    f.render_widget(Block::default().borders(Borders::ALL).title(format!("New {:?}", app.current_view)), area);
}

fn render_loading(f: &mut Frame, area: ratatui::layout::Rect) {
    let area = render_pop_up(area, 20, 10);
    f.render_widget(Clear, area);
    let p = Paragraph::new("Loading...")
        .block(Block::default().borders(Borders::ALL))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, area);
}

use ratatui::widgets::{Paragraph, Wrap};

fn render_detail(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let (title, body) = match app.current_view {
        View::Issues => {
            if let Some(selected) = app.issues_state.selected() {
                if let Some(issue) = app.issues.get(selected) {
                    (format!("Issue #{} - {}", issue.number, issue.title), issue.body.clone())
                } else {
                    ("No issue selected".to_string(), "".to_string())
                }
            } else {
                ("No issue selected".to_string(), "".to_string())
            }
        }
        View::PullRequests => {
            if let Some(selected) = app.prs_state.selected() {
                if let Some(pr) = app.prs.get(selected) {
                    (format!("PR #{} - {}", pr.number, pr.title), pr.body.clone())
                } else {
                    ("No PR selected".to_string(), "".to_string())
                }
            } else {
                ("No PR selected".to_string(), "".to_string())
            }
        }
        View::Projects => {
            if let Some(selected) = app.projects_state.selected() {
                if let Some(project) = app.projects.get(selected) {
                    (format!("Project - {}", project.title), project.body.clone().unwrap_or_default())
                } else {
                    ("No project selected".to_string(), "".to_string())
                }
            } else {
                ("No project selected".to_string(), "".to_string())
            }
        }
    };

    let lines = app.renderer.render(&body);

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn render_issues(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header_cells = ["#", "Title", "State", "Author", "Updated"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Blue));

    let rows = app.issues.iter().map(|item| {
        let cells = vec![
            Cell::from(item.number.to_string()),
            Cell::from(item.title.clone()),
            Cell::from(item.state.clone()),
            Cell::from(item.author.login.clone()),
            Cell::from(item.updated_at.clone()),
        ];
        Row::new(cells)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(5),
        Constraint::Percentage(50),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(20),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Issues"))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.issues_state);
}

fn render_prs(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header_cells = ["#", "Title", "State", "Author", "M"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Blue));

    let rows = app.prs.iter().map(|item| {
        let cells = vec![
            Cell::from(item.number.to_string()),
            Cell::from(item.title.clone()),
            Cell::from(item.state.clone()),
            Cell::from(item.author.login.clone()),
            Cell::from(item.mergeable.clone()),
        ];
        Row::new(cells)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(5),
        Constraint::Percentage(60),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Pull Requests"))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.prs_state);
}

fn render_projects(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header_cells = ["Title", "Updated"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Blue));

    let rows = app.projects.iter().map(|item| {
        let cells = vec![
            Cell::from(item.title.clone()),
            Cell::from(item.updated_at.clone()),
        ];
        Row::new(cells)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(70),
        Constraint::Percentage(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Projects"))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.projects_state);
}
