use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
};
use tokio::sync::mpsc;

use crate::{action::Action, terminal::Frame};

use super::{logger::Logger, search::Search, Component};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum Context {
    #[default]
    Home,
    Search,
}

#[derive(Default)]
pub struct Root {
    search: Search,
    logger: Logger,

    show_logger: bool,
    context: Context,
}

impl Root {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for Root {
    fn init(&mut self, sender: mpsc::UnboundedSender<Action>) -> Result<()> {
        self.search.init(sender.clone())?;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Action {
        match key.code {
            // FIXME: Handle global key events after the context
            KeyCode::Char('l') => Action::ToggleShowLogger,
            KeyCode::Char('q') => Action::Quit,
            _ => match self.context {
                Context::Home => match key.code {
                    KeyCode::Char('s') => Action::EnterContext(Context::Search),
                    _ => Action::Noop,
                },
                Context::Search => self.search.handle_key_events(key),
            },
        }
    }

    fn dispatch(&mut self, action: Action) -> Option<Action> {
        // global actions
        if let Some(_action) = match action {
            Action::ToggleShowLogger => {
                self.show_logger = !self.show_logger;
                None
            }
            Action::EnterContext(ref context) => {
                self.context = context.to_owned();
                None
            }
            _ => None,
        } {
            return Some(_action);
        }

        // all other actions are passed on to the current component
        if let Some(_action) = match self.context {
            Context::Home => None,
            Context::Search => self.search.dispatch(action),
        } {
            return Some(_action);
        }

        None
    }

    fn render(&mut self, frame: &mut Frame<'_>, size: Rect) {
        let size = if self.show_logger {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(size);
            self.logger.render(frame, chunks[1]);
            chunks[0]
        } else {
            size
        };

        match self.context {
            Context::Home => frame.render_widget(Paragraph::new("Hello World!"), size),
            Context::Search => self.search.render(frame, size),
        }
    }
}
