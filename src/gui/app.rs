use iced::widget::{button, column, container, pick_list, row, text, text_editor};
use iced::{Element, Length, Task, Theme};
use std::time::Instant;

use crate::config::models::{ServerConfigEntry, SslMode};
use crate::config::UserConfigService;
use crate::database::DbExecutor;

use super::components::editor::SqlEditorState;
use super::components::results::{QueryResultData, QueryResultTable};
use super::components::server_manager::ServerFormState;
use super::components::sidebar::ConnectionSidebarState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    QueryExecutor,
    ServerManagement,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(ActiveTab),
    ThemeSelected(Theme),
    CodeEdited(text_editor::Action),
    ExecuteQuery,
    QueryResultReceived(Result<QueryResultData, String>),
    SelectServer(ServerConfigEntry),

    // Server Form Inputs
    ServerNameInput(String),
    ServerHostInput(String),
    ServerPortInput(String),
    ServerUsernameInput(String),
    ServerPasswordInput(String),
    ServerDatabaseInput(String),
    ServerSslModeSelected(SslMode),

    // Server Form Actions
    NewServerClicked,
    EditServerClicked(ServerConfigEntry),
    TestConnectionClicked,
    TestConnectionResultReceived(Result<String, String>),
    SaveServerClicked,
    DeleteServerClicked,
}

pub struct MustelApp {
    active_tab: ActiveTab,
    theme: Theme,
    sidebar: ConnectionSidebarState,
    editor: SqlEditorState,
    results_table: QueryResultTable,
    server_form: ServerFormState,
    config_service: UserConfigService,
    tokio_handle: tokio::runtime::Handle,
}

impl Default for MustelApp {
    fn default() -> Self {
        let config_service = UserConfigService::new();
        Self {
            active_tab: ActiveTab::QueryExecutor,
            theme: Theme::Dark,
            sidebar: ConnectionSidebarState::new(),
            editor: SqlEditorState::new(),
            results_table: QueryResultTable::new(),
            server_form: ServerFormState::new(),
            config_service,
            tokio_handle: tokio::runtime::Handle::current(),
        }
    }
}

impl MustelApp {
    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn refresh_sidebar(&mut self) {
        if let Ok(servers) = self.config_service.get_servers() {
            self.sidebar.reload_servers(servers);
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Task::none()
            }
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
            Message::ServerNameInput(val) => {
                self.server_form.name_input = val;
                Task::none()
            }
            Message::ServerHostInput(val) => {
                self.server_form.host_input = val;
                Task::none()
            }
            Message::ServerPortInput(val) => {
                self.server_form.port_input = val;
                Task::none()
            }
            Message::ServerUsernameInput(val) => {
                self.server_form.username_input = val;
                Task::none()
            }
            Message::ServerPasswordInput(val) => {
                self.server_form.password_input = val;
                Task::none()
            }
            Message::ServerDatabaseInput(val) => {
                self.server_form.database_input = val;
                Task::none()
            }
            Message::ServerSslModeSelected(ssl) => {
                self.server_form.ssl_mode = ssl;
                Task::none()
            }
            Message::NewServerClicked => {
                self.server_form.clear();
                Task::none()
            }
            Message::EditServerClicked(server) => {
                self.server_form.load_server(&server);
                Task::none()
            }
            Message::TestConnectionClicked => {
                match self.server_form.to_server_entry() {
                    Ok(server) => {
                        self.server_form.is_testing = true;
                        self.server_form.test_status = None;
                        let handle = self.tokio_handle.clone();

                        Task::perform(
                            async move {
                                let res = handle
                                    .spawn(async move {
                                        let target_db = server
                                            .databases
                                            .as_ref()
                                            .and_then(|dbs| dbs.first())
                                            .cloned()
                                            .unwrap_or_else(|| "postgres".to_string());

                                        match DbExecutor::connect(&server, Some(&target_db), server.password.as_deref()).await {
                                            Ok(_) => Ok(format!("Conexão bem sucedida com {}:{}!", server.host, server.port)),
                                            Err(e) => Err(format!("{}", e)),
                                        }
                                    })
                                    .await;

                                match res {
                                    Ok(test_res) => test_res,
                                    Err(e) => Err(format!("Erro interno ao testar conexão: {}", e)),
                                }
                            },
                            Message::TestConnectionResultReceived,
                        )
                    }
                    Err(err) => {
                        self.server_form.test_status = Some(Err(err));
                        Task::none()
                    }
                }
            }
            Message::TestConnectionResultReceived(result) => {
                self.server_form.is_testing = false;
                self.server_form.test_status = Some(result);
                Task::none()
            }
            Message::SaveServerClicked => {
                match self.server_form.to_server_entry() {
                    Ok(server) => {
                        if let Err(e) = self.config_service.add_or_update_server(server.clone()) {
                            self.server_form.feedback_message = Some(format!("Erro ao salvar: {}", e));
                        } else {
                            if let Some(ref pwd) = server.password {
                                let _ = self.config_service.set_encrypted_password(&server.name, pwd.clone());
                            }
                            self.server_form.feedback_message = Some(format!("Servidor '{}' salvo com sucesso!", server.name));
                            self.server_form.selected_name = Some(server.name);
                            self.refresh_sidebar();
                        }
                    }
                    Err(err) => {
                        self.server_form.feedback_message = Some(format!("Erro de validação: {}", err));
                    }
                }
                Task::none()
            }
            Message::DeleteServerClicked => {
                if let Some(ref name) = self.server_form.selected_name.clone() {
                    let _ = self.config_service.remove_server(name);
                    self.server_form.clear();
                    self.server_form.feedback_message = Some(format!("Servidor '{}' removido com sucesso.", name));
                    self.refresh_sidebar();
                }
                Task::none()
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let title = text("Mustel Database Toolkit").size(24);

        // Header Tab Buttons
        let query_tab_btn = if self.active_tab == ActiveTab::QueryExecutor {
            button(text("🔍 Executar Queries")).style(button::primary)
        } else {
            button(text("🔍 Executar Queries")).style(button::secondary)
        }
        .on_press(Message::TabSelected(ActiveTab::QueryExecutor));

        let server_tab_btn = if self.active_tab == ActiveTab::ServerManagement {
            button(text("⚙️ Gerenciar Servidores")).style(button::primary)
        } else {
            button(text("⚙️ Gerenciar Servidores")).style(button::secondary)
        }
        .on_press(Message::TabSelected(ActiveTab::ServerManagement));

        let nav_tabs = row![query_tab_btn, server_tab_btn].spacing(10);

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
            nav_tabs,
            container(theme_picker).align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let body: Element<'a, Message> = match self.active_tab {
            ActiveTab::QueryExecutor => {
                let execute_btn = button(text("▶ Executar Query (F5)"))
                    .on_press(Message::ExecuteQuery);

                let main_content = column![
                    self.editor.view(),
                    execute_btn,
                    self.results_table.view(),
                ]
                .spacing(15)
                .width(Length::Fill);

                row![
                    self.sidebar.view(),
                    main_content,
                ]
                .spacing(20)
                .into()
            }
            ActiveTab::ServerManagement => {
                self.server_form.view(&self.sidebar.server_list)
            }
        };

        container(
            column![
                header,
                body,
            ]
            .spacing(20)
        )
        .padding(15)
        .into()
    }
}
