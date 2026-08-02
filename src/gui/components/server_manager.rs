use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Element, Length};

use crate::config::models::{ServerConfigEntry, SslMode};
use super::super::app::Message;

#[derive(Debug, Clone)]
pub struct ServerFormState {
    pub selected_name: Option<String>,
    pub name_input: String,
    pub host_input: String,
    pub port_input: String,
    pub username_input: String,
    pub password_input: String,
    pub database_input: String,
    pub ssl_mode: SslMode,
    pub test_status: Option<Result<String, String>>,
    pub is_testing: bool,
    pub feedback_message: Option<String>,
}

impl Default for ServerFormState {
    fn default() -> Self {
        Self {
            selected_name: None,
            name_input: "".to_string(),
            host_input: "localhost".to_string(),
            port_input: "5432".to_string(),
            username_input: "postgres".to_string(),
            password_input: "".to_string(),
            database_input: "postgres".to_string(),
            ssl_mode: SslMode::Prefer,
            test_status: None,
            is_testing: false,
            feedback_message: None,
        }
    }
}

impl ServerFormState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_server(&mut self, server: &ServerConfigEntry) {
        self.selected_name = Some(server.name.clone());
        self.name_input = server.name.clone();
        self.host_input = server.host.clone();
        self.port_input = server.port.to_string();
        self.username_input = server.username.clone();
        self.password_input = server.password.clone().unwrap_or_default();
        self.database_input = server
            .databases
            .as_ref()
            .and_then(|dbs| dbs.first())
            .cloned()
            .unwrap_or_else(|| "postgres".to_string());
        self.ssl_mode = server.ssl_mode.clone().unwrap_or(SslMode::Prefer);
        self.test_status = None;
        self.feedback_message = None;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn to_server_entry(&self) -> Result<ServerConfigEntry, String> {
        if self.name_input.trim().is_empty() {
            return Err("O nome da conexão não pode ser vazio.".to_string());
        }
        if self.host_input.trim().is_empty() {
            return Err("O host não pode ser vazio.".to_string());
        }
        let port: u16 = self
            .port_input
            .trim()
            .parse()
            .map_err(|_| "Porta inválida (deve ser um número de 1 a 65535).".to_string())?;

        let password = if self.password_input.is_empty() {
            None
        } else {
            Some(self.password_input.clone())
        };

        let database = if self.database_input.trim().is_empty() {
            "postgres".to_string()
        } else {
            self.database_input.trim().to_string()
        };

        Ok(ServerConfigEntry {
            name: self.name_input.trim().to_string(),
            host: self.host_input.trim().to_string(),
            port,
            username: self.username_input.trim().to_string(),
            password,
            encrypted_password: None,
            databases: Some(vec![database]),
            fetch_all_databases: Some(false),
            exclude_patterns: Some(vec!["template*".to_string()]),
            ssl_mode: Some(self.ssl_mode.clone()),
            timeout: Some(30),
            command_timeout: Some(300),
            max_parallelism: Some(4),
        })
    }

    pub fn view<'a>(&'a self, servers: &'a [ServerConfigEntry]) -> Element<'a, Message> {
        let title = text("Gerenciamento de Servidores PostgreSQL").size(22);

        // Sidebar list of saved servers
        let mut server_list_col = column![text("Servidores Salvos:").size(16)].spacing(8);

        let new_btn = button(text("➕ Novo Servidor"))
            .on_press(Message::NewServerClicked);
        server_list_col = server_list_col.push(new_btn);

        for server in servers {
            let is_selected = self.selected_name.as_ref() == Some(&server.name);
            let label = format!("🗄️ {} ({}:{})", server.name, server.host, server.port);
            let btn = if is_selected {
                button(text(label)).style(button::primary)
            } else {
                button(text(label)).style(button::secondary)
            };
            let btn = btn.on_press(Message::EditServerClicked(server.clone()));
            server_list_col = server_list_col.push(btn);
        }

        let left_panel = container(server_list_col)
            .width(Length::Fixed(220.0));

        // Form Fields
        let form_title = if self.selected_name.is_some() {
            text("Editar Servidor").size(18)
        } else {
            text("Novo Servidor").size(18)
        };

        let ssl_modes = vec![
            SslMode::Disable,
            SslMode::Allow,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ];

        let ssl_picker = pick_list(
            ssl_modes,
            Some(self.ssl_mode.clone()),
            Message::ServerSslModeSelected,
        );

        let form_fields = column![
            form_title,
            row![text("Nome da Conexão:").width(Length::Fixed(150.0)), text_input("Ex: localhost", &self.name_input).on_input(Message::ServerNameInput)].spacing(10),
            row![text("Host / IP:").width(Length::Fixed(150.0)), text_input("Ex: 127.0.0.1", &self.host_input).on_input(Message::ServerHostInput)].spacing(10),
            row![text("Porta:").width(Length::Fixed(150.0)), text_input("5432", &self.port_input).on_input(Message::ServerPortInput)].spacing(10),
            row![text("Usuário:").width(Length::Fixed(150.0)), text_input("postgres", &self.username_input).on_input(Message::ServerUsernameInput)].spacing(10),
            row![text("Senha:").width(Length::Fixed(150.0)), text_input("••••••••", &self.password_input).secure(true).on_input(Message::ServerPasswordInput)].spacing(10),
            row![text("Banco Padrão:").width(Length::Fixed(150.0)), text_input("postgres", &self.database_input).on_input(Message::ServerDatabaseInput)].spacing(10),
            row![text("Modo SSL:").width(Length::Fixed(150.0)), ssl_picker].spacing(10),
        ]
        .spacing(12);

        // Action Buttons
        let test_btn = if self.is_testing {
            button(text("🔌 Testando..."))
        } else {
            button(text("🔌 Testar Conexão")).on_press(Message::TestConnectionClicked)
        };

        let save_btn = button(text("💾 Salvar Servidor")).on_press(Message::SaveServerClicked);

        let mut action_row = row![test_btn, save_btn].spacing(12);

        if self.selected_name.is_some() {
            let delete_btn = button(text("🗑️ Excluir")).style(button::danger).on_press(Message::DeleteServerClicked);
            action_row = action_row.push(delete_btn);
        }

        // Status / Feedback Badges
        let mut status_column = column![].spacing(8);

        if let Some(ref feedback) = self.feedback_message {
            status_column = status_column.push(text(feedback).style(text::success));
        }

        if let Some(ref res) = self.test_status {
            match res {
                Ok(msg) => {
                    status_column = status_column.push(text(format!("✔ Teste OK: {}", msg)).style(text::success));
                }
                Err(err) => {
                    status_column = status_column.push(text(format!("✖ Teste Falhou: {}", err)).style(text::danger));
                }
            }
        }

        let right_panel = container(
            column![
                form_fields,
                action_row,
                status_column,
            ]
            .spacing(18)
        )
        .width(Length::Fill);

        let content = row![
            left_panel,
            right_panel,
        ]
        .spacing(20);

        container(
            column![
                title,
                content,
            ]
            .spacing(20)
        )
        .into()
    }
}
