use iced::widget::{button, column, container, pick_list, row, text, text_editor};
use iced::{Element, Length, Task, Theme};
use std::time::Instant;

use crate::config::models::ServerConfigEntry;
use crate::database::DbExecutor;

use super::components::editor::SqlEditorState;
use super::components::results::{QueryResultData, QueryResultTable};
use super::components::sidebar::ConnectionSidebarState;

#[derive(Debug, Clone)]
pub enum Message {
    ThemeSelected(Theme),
    CodeEdited(text_editor::Action),
    ExecuteQuery,
    QueryResultReceived(Result<QueryResultData, String>),
    SelectServer(ServerConfigEntry),
}

pub struct MustelApp {
    theme: Theme,
    sidebar: ConnectionSidebarState,
    editor: SqlEditorState,
    results_table: QueryResultTable,
    tokio_handle: tokio::runtime::Handle,
}

impl Default for MustelApp {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            sidebar: ConnectionSidebarState::new(),
            editor: SqlEditorState::new(),
            results_table: QueryResultTable::new(),
            tokio_handle: tokio::runtime::Handle::current(),
        }
    }
}

impl MustelApp {
    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeSelected(theme) => {
                self.theme = theme;
                Task::none()
            }
            Message::CodeEdited(action) => {
                self.editor.update_content(action);
                Task::none()
            }
            Message::SelectServer(server) => {
                self.sidebar.selected_server = Some(server);
                Task::none()
            }
            Message::ExecuteQuery => {
                self.results_table.is_loading = true;
                let query_sql = self.editor.content.text();
                let server_opt = self.sidebar.selected_server.clone();
                let handle = self.tokio_handle.clone();

                Task::perform(
                    async move {
                        let res = handle
                            .spawn(async move {
                                let Some(server) = server_opt else {
                                    return Err("Nenhum servidor PostgreSQL configurado ou selecionado.".to_string());
                                };

                                let target_db = server
                                    .databases
                                    .as_ref()
                                    .and_then(|dbs| dbs.first())
                                    .cloned()
                                    .unwrap_or_else(|| "postgres".to_string());

                                let start = Instant::now();

                                match DbExecutor::connect(&server, Some(&target_db), server.password.as_deref()).await {
                                    Ok(client) => match client.query(&query_sql, &[]).await {
                                        Ok(rows) => {
                                            let elapsed = start.elapsed().as_millis();
                                            Ok(QueryResultData::from_postgres_rows(&rows, elapsed))
                                        }
                                        Err(e) => Err(format!("Erro ao executar query no PostgreSQL: {}", e)),
                                    },
                                    Err(e) => Err(format!("Falha na conexão com {}:{} (banco '{}'): {}", server.host, server.port, target_db, e)),
                                }
                            })
                            .await;

                        match res {
                            Ok(query_res) => query_res,
                            Err(e) => Err(format!("Erro de execução assíncrona: {}", e)),
                        }
                    },
                    Message::QueryResultReceived,
                )
            }
            Message::QueryResultReceived(result) => {
                self.results_table.is_loading = false;
                match result {
                    Ok(data) => {
                        self.results_table.data = Some(data);
                        self.results_table.error_message = None;
                    }
                    Err(err) => {
                        self.results_table.error_message = Some(err);
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let title = text("Mustel Database Toolkit").size(24);

        let themes = vec![
            Theme::Dark,
            Theme::Light,
            Theme::TokyoNight,
            Theme::Dracula,
            Theme::GruvboxDark,
            Theme::GruvboxLight,
            Theme::SolarizedDark,
            Theme::SolarizedLight,
        ];

        let theme_picker = pick_list(
            themes,
            Some(self.theme.clone()),
            Message::ThemeSelected,
        );

        let header = row![
            title,
            container(theme_picker).align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let execute_btn = button(text("▶ Executar Query (F5)"))
            .on_press(Message::ExecuteQuery);

        let main_content = column![
            self.editor.view(),
            execute_btn,
            self.results_table.view(),
        ]
        .spacing(15)
        .width(Length::Fill);

        let layout = row![
            self.sidebar.view(),
            main_content,
        ]
        .spacing(20);

        container(
            column![
                header,
                layout,
            ]
            .spacing(20)
        )
        .padding(15)
        .into()
    }
}
