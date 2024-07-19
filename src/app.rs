use std::error;
use std::ops::Div;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;
use status_bar::{StatusBar, StatusBarState};
use tabular::Tabular;

use crate::command::{Commands, CommandRegistery};
use crate::keybind::{Action, Keybind};
use crate::sql::SqlBackend;
use crate::theme::Styler;

pub mod status_bar;
pub mod tabular;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

pub struct App {
    pub tabular: Tabular,
    pub status_bar: StatusBar,
    pub sql: SqlBackend,
    exec_table: CommandRegistery,
    keybindings: Keybind,
    running: bool,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum AppState {
    Tabular,
    Detail,
    Command,
    Error,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum AppAction {
    StatusBarStats,
    StatusBarCommand(String),
    StatausBarError(String),
    TabularTableView,
    TabularDetailView,
    TabularSwitchView,
    SqlQuery(String),
    SqlBackendTable,
    TabularGoto(usize),
    TabularGotoFirst,
    TabularGotoLast,
    TabularGotoRandom,
    TabularGoUp(usize),
    TabularGoUpHalfPage,
    TabularGoUpFullPage,
    TabularGoDown(usize),
    TabularGoDownHalfPage,
    TabularGoDownFullPage,
    DetailScrollUp,
    DetailScrollDown,
    TabularReset,
    TabularSelect(String),
    TabularOrder(String),
    TabularFilter(String),
    Help,
    Quit,
}

impl App {
    pub fn new(
        tabular: Tabular,
        status_bar: StatusBar,
        sql: SqlBackend,
        exec_table: CommandRegistery,
        key_bind: Keybind,
    ) -> Self {
        Self {
            tabular,
            status_bar,
            sql,
            exec_table,
            keybindings: key_bind,
            running: true,
        }
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn quit(&mut self) -> AppResult<()> {
        self.running = false;
        Ok(())
    }

    pub fn infer_state(&self) -> AppState {
        match self.status_bar.state {
            StatusBarState::Normal => {
                if self.tabular.scroll().is_some() {
                    AppState::Detail
                } else {
                    AppState::Tabular
                }
            }
            StatusBarState::Error(_) => AppState::Error,
            StatusBarState::Command(_) => AppState::Command,
        }
    }

    pub fn draw<Theme: Styler>(&mut self, frame: &mut Frame) -> AppResult<()> {
        let layout =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(frame.size());

        // Draw table / item
        self.tabular.render::<Theme>(frame, layout[0])?;
        self.status_bar
            .render::<Theme>(frame, layout[1], &self.tabular)
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> AppResult<()> {
        let state = self.infer_state();
        let key_code = key_event.code;
        match (state, key_code) {
            (AppState::Command | AppState::Error, KeyCode::Esc) => self.status_bar.stats(),

            (AppState::Command, KeyCode::Enter) => {
                if let Some(command) = self.status_bar.commit_prompt() {
                    let (s1, s2) = command.split_once(' ').unwrap_or((command.as_ref(), ""));
                    if let Some(parse_fn) = self.exec_table.get(s1) {
                        match parse_fn(s2).and_then(|action| self.invoke(action)) {
                            Ok(_) => self.status_bar.stats(),
                            Err(error) => self.status_bar.error(error),
                        }
                    } else {
                        self.status_bar.error("Command not found")
                    }
                } else {
                    self.status_bar
                        .error("Invalid state; consider restarting Tabiew")
                }
            }

            (AppState::Command, _) => self.status_bar.input(key_event),

            (AppState::Tabular | AppState::Detail | AppState::Error, KeyCode::Char(':')) => {
                self.status_bar.command("")
            }

            _ => self
                .keybindings
                .get_action(state, key_event)
                .cloned()
                .map(|action| self.invoke(action))
                .unwrap_or(Ok(())),
        }
    }
    fn invoke(&mut self, action: Action) -> AppResult<()> {
        match action {
            AppAction::StatusBarStats => self.status_bar.stats(),

            AppAction::StatusBarCommand(prefix) => self.status_bar.command(prefix),

            AppAction::StatausBarError(msg) => self.status_bar.error(msg),

            AppAction::TabularTableView => self.tabular.table_view(),

            AppAction::TabularDetailView => self.tabular.detail_view(),

            AppAction::TabularSwitchView => self.tabular.switch_view(),

            AppAction::SqlQuery(query) => self.tabular.set_data_frame(self.sql.execute(&query)?),

            AppAction::SqlBackendTable => self.tabular.set_data_frame(self.sql.table_df()),

            AppAction::TabularGoto(line) => self.tabular.select(line),

            AppAction::TabularGotoFirst => self.tabular.select_first(),

            AppAction::TabularGotoLast => self.tabular.select_last(),

            AppAction::TabularGotoRandom => self.tabular.select_random(),

            AppAction::TabularGoUp(lines) => self.tabular.select_up(lines),

            AppAction::TabularGoUpHalfPage => {
                self.tabular.select_up(self.tabular.page_len().div(2))
            }

            AppAction::TabularGoUpFullPage => self.tabular.select_up(self.tabular.page_len()),

            AppAction::TabularGoDown(lines) => self.tabular.select_down(lines),

            AppAction::TabularGoDownHalfPage => {
                self.tabular.select_down(self.tabular.page_len().div(2))
            }

            AppAction::TabularGoDownFullPage => self.tabular.select_down(self.tabular.page_len()),

            AppAction::DetailScrollUp => self.tabular.scroll_up(),

            AppAction::DetailScrollDown => self.tabular.scroll_down(),

            AppAction::TabularReset => if let Some(data_frame) = self.sql.default_df() {
                self.tabular.set_data_frame(data_frame)
            } else {
                Err("Default data frame not found".into())
            },

            AppAction::TabularSelect(select) => {
                let mut back = SqlBackend::new();
                back.register("df", self.tabular.data_frame().clone(), "".into());
                self.tabular
                    .set_data_frame(back.execute(&format!("SELECT {} FROM df", select))?)
            }

            AppAction::TabularOrder(order) => {
                let mut back = SqlBackend::new();
                back.register("df", self.tabular.data_frame().clone(), "".into());
                self.tabular
                    .set_data_frame(back.execute(&format!("SELECT * FROM df ORDER BY {}", order))?)
            }

            AppAction::TabularFilter(filter) => {
                let mut back = SqlBackend::new();
                back.register("df", self.tabular.data_frame().clone(), "".into());
                self.tabular
                    .set_data_frame(back.execute(&format!("SELECT * FROM df where {}", filter))?)
            }
            AppAction::Help => self
                .tabular
                .set_data_frame(Commands::default().into_data_frame()),

            AppAction::Quit => self.quit(),
        }
    }
}
