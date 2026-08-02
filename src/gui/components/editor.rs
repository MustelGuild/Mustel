use iced::widget::{column, container, row, text, text_editor};
use iced::{Element, Length};
use crate::cli::commands::query::analyzer::{SqlQueryAnalyzer, QueryType};
use super::super::app::Message;

pub struct SqlEditorState {
    pub content: text_editor::Content,
    pub is_destructive: bool,
    pub query_type: String,
}

impl Default for SqlEditorState {
    fn default() -> Self {
        Self {
            content: text_editor::Content::with_text("SELECT 1 as id, 'PostgreSQL' as name;"),
            is_destructive: false,
            query_type: "ReadOnly".to_string(),
        }
    }
}

impl SqlEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_content(&mut self, action: text_editor::Action) {
        self.content.perform(action);
        let text_str = self.content.text();
        if text_str.trim().is_empty() {
            self.is_destructive = false;
            self.query_type = "Empty".to_string();
            return;
        }

        match SqlQueryAnalyzer::analyze(&text_str) {
            QueryType::ReadOnly => {
                self.is_destructive = false;
                self.query_type = "ReadOnly".to_string();
            }
            QueryType::Destructive(reason) => {
                self.is_destructive = true;
                self.query_type = reason;
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let status_badge = if self.is_destructive {
            text(format!("⚠️ ALERTA: Query Destrutiva ({})!", self.query_type))
                .style(text::danger)
        } else {
            text(format!("Status: {}", self.query_type))
                .style(text::success)
        };

        let header = row![
            text("Editor SQL").size(20),
            status_badge,
        ]
        .spacing(15);

        let editor_widget = text_editor(&self.content)
            .on_action(Message::CodeEdited)
            .height(Length::Fixed(180.0));

        container(
            column![
                header,
                editor_widget,
            ]
            .spacing(10)
        )
        .into()
    }
}
