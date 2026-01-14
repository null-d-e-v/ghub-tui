use ratatui::widgets::TableState;
use crate::gh::{Issue, PullRequest, Project};
use crate::ui::markdown::MarkdownRenderer;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum View {
    Issues,
    PullRequests,
    Projects,
}

pub enum AppEvent {
    Tick,
    Key(crossterm::event::KeyEvent),
    IssuesLoaded(Vec<Issue>),
    PRsLoaded(Vec<PullRequest>),
    ProjectsLoaded(Vec<Project>),
    Success(String),
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FormFocus {
    Title,
    Body,
}

pub struct App {
    pub current_view: View,
    pub input_mode: InputMode,
    pub input: String,
    pub issues: Vec<Issue>,
    pub prs: Vec<PullRequest>,
    pub projects: Vec<Project>,
    pub issues_state: TableState,
    pub prs_state: TableState,
    pub projects_state: TableState,
    pub loading: bool,
    pub show_detail: bool,
    pub context: Option<String>,
    pub error: Option<String>,
    pub renderer: MarkdownRenderer,
    pub show_help: bool,
    pub show_create_form: bool,
    pub form_focus: FormFocus,
    pub form_title: String,
    pub form_body: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_view: View::Issues,
            input_mode: InputMode::Normal,
            input: String::new(),
            issues: Vec::new(),
            prs: Vec::new(),
            projects: Vec::new(),
            issues_state: TableState::default(),
            prs_state: TableState::default(),
            projects_state: TableState::default(),
            loading: false,
            show_detail: false,
            context: None,
            error: None,
            renderer: MarkdownRenderer::new(),
            show_help: false,
            show_create_form: false,
            form_focus: FormFocus::Title,
            form_title: String::new(),
            form_body: String::new(),
            should_quit: false,
        }
    }

    pub fn next_view(&mut self) {
        self.current_view = match self.current_view {
            View::Issues => View::PullRequests,
            View::PullRequests => View::Projects,
            View::Projects => View::Issues,
        };
    }

    pub fn select_next(&mut self) {
        match self.current_view {
            View::Issues => {
                let i = match self.issues_state.selected() {
                    Some(i) => {
                        if i >= self.issues.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.issues_state.select(Some(i));
            }
            View::PullRequests => {
                let i = match self.prs_state.selected() {
                    Some(i) => {
                        if i >= self.prs.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.prs_state.select(Some(i));
            }
            View::Projects => {
                let i = match self.projects_state.selected() {
                    Some(i) => {
                        if i >= self.projects.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.projects_state.select(Some(i));
            }
        }
    }

    pub fn select_previous(&mut self) {
        match self.current_view {
            View::Issues => {
                let i = match self.issues_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.issues.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.issues_state.select(Some(i));
            }
            View::PullRequests => {
                let i = match self.prs_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.prs.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.prs_state.select(Some(i));
            }
            View::Projects => {
                let i = match self.projects_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.projects.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.projects_state.select(Some(i));
            }
        }
    }
}
