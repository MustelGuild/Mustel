use iced::widget::{button, column, container, text};
use iced::{Element, Length};
use crate::config::models::ServerConfigEntry;
use crate::config::UserConfigService;
use super::super::app::Message;

pub struct ConnectionSidebarState {
    pub selected_server: Option<ServerConfigEntry>,
    pub server_list: Vec<ServerConfigEntry>,
}

impl Default for ConnectionSidebarState {
    fn default() -> Self {
        let config_service = UserConfigService::new();
        let configured_servers = config_service.get_servers().unwrap_or_default();

        let server_list = if configured_servers.is_empty() {
            vec![ServerConfigEntry {
                name: "localhost".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                username: "postgres".to_string(),
                password: None,
                encrypted_password: None,
                databases: Some(vec!["postgres".to_string()]),
                fetch_all_databases: None,
                exclude_patterns: None,
                ssl_mode: None,
                timeout: Some(30),
                command_timeout: Some(300),
                max_parallelism: None,
            }]
        } else {
            configured_servers
        };

        let selected_server = server_list.first().cloned();

        Self {
            selected_server,
            server_list,
        }
    }
}

impl ConnectionSidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reload_servers(&mut self, servers: Vec<ServerConfigEntry>) {
        let current_selected_name = self.selected_server.as_ref().map(|s| s.name.clone());
        self.server_list = servers;
        if let Some(ref name) = current_selected_name {
            self.selected_server = self.server_list.iter().find(|s| s.name == *name).cloned();
        }
        if self.selected_server.is_none() {
            self.selected_server = self.server_list.first().cloned();
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let title = text("Conexões").size(20);

        let mut server_buttons = column![title].spacing(8);

        server_buttons = server_buttons.push(text("Servidores Configurados:"));

        for server in &self.server_list {
            let is_selected = self.selected_server.as_ref().map(|s| &s.name) == Some(&server.name);
            let btn_label = format!("🗄️ {}", server.name);
            let btn = if is_selected {
                button(text(btn_label)).style(button::primary)
            } else {
                button(text(btn_label)).style(button::secondary)
            };
            let btn = btn.on_press(Message::SelectServer(server.clone()));
            server_buttons = server_buttons.push(btn);
        }

        let status_text = match self.selected_server {
            Some(ref current) => text(format!("Ativo: {} ({}:{})", current.name, current.host, current.port)).style(text::success),
            None => text("Status: Nenhum servidor selecionado"),
        };

        server_buttons = server_buttons.push(status_text);

        container(server_buttons)
            .width(Length::Fixed(220.0))
            .into()
    }
}
