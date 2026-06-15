use ratatui::style::{Color, Modifier, Style};

pub fn title() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn subtitle() -> Style {
    Style::default().fg(Color::Gray)
}

pub fn selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn normal() -> Style {
    Style::default().fg(Color::White)
}

pub fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn success() -> Style {
    Style::default().fg(Color::Green)
}

pub fn warning() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn error() -> Style {
    Style::default().fg(Color::Red)
}

pub fn panel() -> Style {
    Style::default().fg(Color::Gray)
}
