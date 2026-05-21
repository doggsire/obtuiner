use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPane {
    Search,
    Results,
    Details,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            Self::Search => Self::Results,
            Self::Results => Self::Details,
            Self::Details => Self::Search,
        }
    }
}

#[derive(Debug)]
pub enum CommonAction {
    None,
    Quit,
    Activate,
}

#[derive(Debug)]
pub struct SharedState {
    pub query: String,
    pub selected: usize,
    pub focus: FocusPane,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            query: String::new(),
            selected: 0,
            focus: FocusPane::Search,
        }
    }
}

pub struct LayoutData<'a> {
    pub app_title: &'a str,
    pub left_title: &'a str,
    pub right_title: &'a str,
    pub status_line: &'a str,
    pub items: &'a [String],
    pub details: &'a [String],
}

pub fn handle_common_key(state: &mut SharedState, key: KeyEvent, max_items: usize) -> CommonAction {
    match key.code {
        KeyCode::Char('q') => CommonAction::Quit,
        KeyCode::Tab => {
            state.focus = state.focus.next();
            CommonAction::None
        }
        KeyCode::Char('/') => {
            state.focus = FocusPane::Search;
            CommonAction::None
        }
        KeyCode::Up => {
            if state.focus == FocusPane::Results {
                if state.selected > 0 {
                    state.selected -= 1;
                } else {
                    state.focus = FocusPane::Search;
                }
            }
            CommonAction::None
        }
        KeyCode::Down => {
            if state.focus == FocusPane::Search {
                state.focus = FocusPane::Results;
            } else if state.focus == FocusPane::Results && state.selected + 1 < max_items {
                state.selected += 1;
            }
            CommonAction::None
        }
        KeyCode::Backspace => {
            if state.focus == FocusPane::Search {
                state.query.pop();
                state.selected = 0;
            }
            CommonAction::None
        }
        KeyCode::Char(c) => {
            if state.focus == FocusPane::Search {
                state.query.push(c);
                state.selected = 0;
            }
            CommonAction::None
        }
        KeyCode::Enter => CommonAction::Activate,
        _ => CommonAction::None,
    }
}

pub fn render_layout(frame: &mut Frame, state: &SharedState, data: &LayoutData<'_>) {
    let root = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .split(frame.area());

    let header = Paragraph::new(format!("Search: {}", state.query)).block(
        Block::default()
            .title(format!("{} | / focus search", data.app_title))
            .borders(Borders::ALL)
            .border_style(if state.focus == FocusPane::Search {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    frame.render_widget(header, root[0]);

    let body = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(root[1]);

    let items: Vec<ListItem> = if data.items.is_empty() {
        vec![ListItem::new("No results")]
    } else {
        data.items
            .iter()
            .map(|item| ListItem::new(item.clone()))
            .collect()
    };
    let list = List::new(items)
        .block(
            Block::default()
                .title(data.left_title)
                .borders(Borders::ALL)
                .border_style(if state.focus == FocusPane::Results {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(" > ");

    let mut list_state = ratatui::widgets::ListState::default();
    if !data.items.is_empty() {
        list_state.select(Some(state.selected.min(data.items.len() - 1)));
    }
    frame.render_stateful_widget(list, body[0], &mut list_state);

    let details = Paragraph::new(data.details.join("\n")).block(
        Block::default()
            .title(data.right_title)
            .borders(Borders::ALL)
            .border_style(if state.focus == FocusPane::Details {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            }),
    );
    frame.render_widget(details, body[1]);

    let footer = Paragraph::new(data.status_line).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, root[2]);
}

// ── Confirmation modal ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConfirmModal {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ModalChoice {
    Confirmed,
    Cancelled,
    Pending,
}

pub fn handle_modal_key(key: KeyEvent) -> ModalChoice {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => ModalChoice::Confirmed,
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
            ModalChoice::Cancelled
        }
        _ => ModalChoice::Pending,
    }
}

pub fn render_confirm_modal(frame: &mut Frame, modal: &ConfirmModal) {
    let area = frame.area();
    let width = (area.width as f32 * 0.65) as u16;
    let height = (modal.lines.len() as u16 + 6).min(area.height - 4);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let mut all_lines = modal.lines.clone();
    all_lines.push(String::new());
    all_lines.push("  [Enter / y] Confirm    [Esc / n] Cancel".to_string());

    let widget = Paragraph::new(all_lines.join("\n")).block(
        Block::default()
            .title(modal.title.clone())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(widget, popup);
}

// ── Log / result panel ─────────────────────────────────────────────────────

pub fn render_log_panel(frame: &mut Frame, area: Rect, lines: &[String]) {
    let widget = Paragraph::new(lines.join("\n"))
        .wrap(ratatui::widgets::Wrap { trim: false })
        .block(
            Block::default()
                .title("Output")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(widget, area);
}
