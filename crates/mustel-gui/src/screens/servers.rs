//! Tela de servidores cadastrados (T42): CRUD no `mustel.jsonc`, senha no cofre e teste de conexão.
//!
//! O formulário edita uma **cópia** do servidor; quem grava é o núcleo (`db_servers::{add, update}`),
//! então as validações e as mensagens são as mesmas do CLI. Editar um servidor existente usa
//! `update` (novo no T42), que preserva a senha do cofre — o caminho do CLI (`rm` + `add`) apagaria.

use mustel_core::config::ServerConfigEntry;

use crate::request::Request;

/// O que o formulário está fazendo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Form {
    /// Servidor sendo editado (`None` = cadastro novo).
    original: Option<String>,
    /// Campos em edição.
    draft: ServerConfigEntry,
    /// Bancos como texto (a lista vira `Vec` ao gravar).
    databases: String,
    /// Padrões de exclusão como texto.
    exclude_patterns: String,
    /// Senha a guardar junto (opcional).
    password: String,
    /// De qual servidor os valores foram copiados (só informativo, para o combo mostrar a origem).
    template: Option<String>,
}

impl Form {
    /// Formulário vazio para um cadastro novo.
    fn new() -> Self {
        Self {
            original: None,
            draft: ServerConfigEntry::default(),
            databases: String::new(),
            exclude_patterns: "template*,postgres".to_string(),
            password: String::new(),
            template: None,
        }
    }

    /// Formulário preenchido a partir de um servidor existente.
    fn edit(server: &ServerConfigEntry) -> Self {
        Self {
            original: Some(server.name.clone()),
            draft: server.clone(),
            databases: server.databases.join(", "),
            exclude_patterns: server.exclude_patterns.join(","),
            password: String::new(),
            template: None,
        }
    }

    /// Copia os valores de `server` para o formulário, como ponto de partida de um cadastro novo.
    ///
    /// Ficam de fora, de propósito:
    ///
    /// - o **nome**, que é a identidade do servidor novo — copiá-lo só criaria um conflito;
    /// - a **senha**, que mora no cofre com a chave do nome; copiar daria ao servidor novo a senha do
    ///   antigo sem ninguém perceber. Quem quiser a mesma senha digita no campo (ou usa o CLI).
    ///
    /// Todo o resto é copiado: host, porta, usuário, bancos, descoberta automática, padrões de
    /// exclusão, SSL, timeout e paralelismo. Depois é só editar o que quiser.
    fn copy_from(&mut self, server: &ServerConfigEntry) {
        self.draft.host = server.host.clone();
        self.draft.port = server.port;
        self.draft.username = server.username.clone();
        self.draft.databases = server.databases.clone();
        self.draft.fetch_all_databases = server.fetch_all_databases;
        self.draft.exclude_patterns = server.exclude_patterns.clone();
        self.draft.ssl_mode = server.ssl_mode.clone();
        self.draft.timeout = server.timeout;
        self.draft.max_parallelism = server.max_parallelism;

        self.databases = server.databases.join(", ");
        self.exclude_patterns = server.exclude_patterns.join(",");
        self.template = Some(server.name.clone());
    }

    /// O servidor como o núcleo espera, já com as listas convertidas.
    fn to_entry(&self) -> ServerConfigEntry {
        ServerConfigEntry {
            databases: split(&self.databases),
            exclude_patterns: split(&self.exclude_patterns),
            ..self.draft.clone()
        }
    }
}

/// Estado da tela de servidores.
#[derive(Debug, Default)]
pub struct State {
    /// Servidores do cadastro, como o núcleo devolveu.
    servers: Vec<ServerConfigEntry>,
    /// Formulário aberto, quando houver.
    form: Option<Form>,
    /// Servidor marcado para remoção (confirmação em dois toques).
    confirming_remove: Option<String>,
    /// Último recado da tela.
    message: Option<String>,
    /// Erro do último pedido.
    error: Option<String>,
    /// Senha a guardar depois que o servidor for gravado (o `Save` manda os dois pedidos).
    pending_password: Option<String>,
}

impl State {
    /// Estado com a lista já carregada.
    pub fn new(servers: Vec<ServerConfigEntry>) -> Self {
        Self {
            servers,
            ..Self::default()
        }
    }

    /// Os servidores do cadastro (a tela de consulta também escolhe entre eles).
    pub fn servers(&self) -> &[ServerConfigEntry] {
        &self.servers
    }

    /// Aplica o que voltou de um job.
    pub fn apply(&mut self, result: &crate::request::JobResult) {
        use crate::request::JobResult;

        match result {
            JobResult::Servers(servers) => {
                self.servers = servers.clone();
                self.error = None;
            }
            JobResult::Saved(name) => {
                self.form = None;
                self.message = Some(format!("Server '{name}' saved."));
                self.error = None;
            }
            JobResult::Removed(name) => {
                self.message = Some(format!("Server '{name}' removed."));
                self.error = None;
                self.confirming_remove = None;
            }
            JobResult::PasswordSaved(name) => {
                self.form = None;
                self.message = Some(format!("Password saved for '{name}'."));
                self.error = None;
            }
            JobResult::Tested { server, databases } => {
                self.message = Some(if databases.is_empty() {
                    format!("'{server}' connected, no accessible databases.")
                } else {
                    format!("'{server}' connected: {}", databases.join(", "))
                });
                self.error = None;
            }
            // O esquema é da tela de consulta; a de servidores não tem o que fazer com ele.
            JobResult::Schema { .. } | JobResult::Databases { .. } => {}
            JobResult::Failed(error) => {
                self.error = Some(error.clone());
            }
            JobResult::QueryDone { .. } | JobResult::QueryWithoutDatabases => {}
        }
    }

    /// A senha que ficou esperando o servidor ser gravado, se houver.
    pub fn take_pending_password(&mut self) -> Option<String> {
        self.pending_password.take()
    }

    /// Desenha a tela e devolve o pedido, quando houver.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Request> {
        let mut request = None;

        ui.horizontal(|ui| {
            if ui.button("Reload").clicked() {
                request = Some(Request::ReloadServers);
            }
            if ui.button("Add server").clicked() {
                self.form = Some(Form::new());
                self.confirming_remove = None;
            }
            ui.label(format!("{} server(s)", self.servers.len()));
        });

        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if self.form.is_some() {
            // O formulário sai do estado enquanto é desenhado: ele precisa de `&mut self` para
            // guardar a senha pendente, e o empréstimo não pode durar o desenho inteiro.
            let mut form = self.form.take().expect("acabou de ser verificado");
            let (new_request, keep_open) = self.form_ui(ui, &mut form);
            request = new_request.or(request);
            if keep_open {
                self.form = Some(form);
            }
        }

        ui.separator();
        let servers = self.servers.clone();
        egui::ScrollArea::vertical()
            .id_salt("servers")
            .max_height(260.0)
            .show(ui, |ui| {
                for server in servers {
                    ui.horizontal(|ui| {
                        ui.monospace(format!(
                            "{:<14} {}:{} {}",
                            server.name,
                            server.host,
                            server.port,
                            if server.fetch_all_databases {
                                "(auto)".to_string()
                            } else if server.databases.is_empty() {
                                "(default)".to_string()
                            } else {
                                server.databases.join(", ")
                            }
                        ));

                        if ui.button("Edit").clicked() {
                            self.form = Some(Form::edit(&server));
                            self.confirming_remove = None;
                        }
                        if ui.button("Test").clicked() {
                            request = Some(Request::TestConnection(server.clone()));
                        }

                        // Remover pede confirmação no mesmo botão: o cadastro apaga a senha do
                        // cofre junto, então um clique só seria fácil demais.
                        let removing = self.confirming_remove.as_deref() == Some(&server.name);
                        let label = if removing { "Confirm remove" } else { "Remove" };
                        if ui.button(label).clicked() {
                            if removing {
                                self.confirming_remove = None;
                                request = Some(Request::RemoveServer(server.name.clone()));
                            } else {
                                self.confirming_remove = Some(server.name.clone());
                            }
                        }
                    });
                }
            });

        request
    }

    /// O formulário de cadastro/edição. Devolve o pedido e se o formulário deve continuar aberto.
    fn form_ui(&mut self, ui: &mut egui::Ui, form: &mut Form) -> (Option<Request>, bool) {
        let mut request = None;
        let mut keep_open = true;
        let editing = form.original.is_some();

        ui.group(|ui| {
            ui.label(if editing { "Edit server" } else { "New server" });

            // Modelo: atalho para cadastrar a partir de um servidor que já existe. Só no cadastro
            // novo (editar um servidor existente é sobre aquele servidor) e só quando há de onde
            // copiar. Clicar numa linha copia **na hora**, inclusive a mesma já escolhida — o combo
            // é um botão de copiar, não um vínculo que se mantém.
            if !editing && !self.servers.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Template");
                    let selected = form.template.clone().unwrap_or_else(|| "—".to_string());
                    egui::ComboBox::from_id_salt("server_template")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for server in &self.servers {
                                if ui
                                    .selectable_label(
                                        form.template.as_deref() == Some(server.name.as_str()),
                                        &server.name,
                                    )
                                    .clicked()
                                {
                                    form.copy_from(server);
                                }
                            }
                        });
                    ui.weak("copia os valores do servidor escolhido (sem nome e sem senha)");
                });
            }

            egui::Grid::new("server_form")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.name));
                    ui.end_row();

                    ui.label("Host");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.host));
                    ui.end_row();

                    ui.label("Port");
                    let mut port = i64::from(form.draft.port);
                    if ui
                        .add(egui::DragValue::new(&mut port).range(1..=65535))
                        .changed()
                    {
                        form.draft.port = u16::try_from(port).unwrap_or(5432);
                    }
                    ui.end_row();

                    ui.label("Username");
                    ui.add(egui::TextEdit::singleline(&mut form.draft.username));
                    ui.end_row();

                    ui.label("Databases");
                    ui.add(egui::TextEdit::singleline(&mut form.databases));
                    ui.end_row();

                    ui.label("Auto-discover");
                    ui.checkbox(&mut form.draft.fetch_all_databases, "fetch all");
                    ui.end_row();

                    ui.label("Exclude patterns");
                    ui.add(egui::TextEdit::singleline(&mut form.exclude_patterns));
                    ui.end_row();

                    ui.label("SSL mode");
                    egui::ComboBox::from_id_salt("ssl_mode")
                        .selected_text(&form.draft.ssl_mode)
                        .show_ui(ui, |ui| {
                            for mode in [
                                "Disable",
                                "Allow",
                                "Prefer",
                                "Require",
                                "VerifyCA",
                                "VerifyFull",
                            ] {
                                ui.selectable_value(
                                    &mut form.draft.ssl_mode,
                                    mode.to_string(),
                                    mode,
                                );
                            }
                        });
                    ui.end_row();

                    ui.label("Password (keyring)");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.password)
                            .password(true)
                            .hint_text(if editing { "unchanged" } else { "optional" }),
                    );
                    ui.end_row();
                });

            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    request = Some(Request::SaveServer {
                        original: form.original.clone(),
                        server: form.to_entry(),
                    });
                    if !form.password.is_empty() {
                        // A senha vai junto: guardar depois de salvar exigiria saber que o save deu
                        // certo, e o `App` já encadeia os dois pedidos nessa ordem.
                        self.pending_password = Some(form.password.clone());
                    }
                }
                if ui.button("Cancel").clicked() {
                    keep_open = false;
                }
            });
        });

        (request, keep_open)
    }
}

/// Separa a lista por vírgula, como os campos de bancos e padrões do CLI.
fn split(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Copiar de um modelo traz **todos** os valores do formulário, menos nome e senha.
    ///
    /// O nome é a identidade do servidor novo e a senha mora no cofre com a chave do nome: copiar a
    /// senha daria ao servidor novo a senha do antigo sem ninguém perceber.
    /// CONTRACT: IF-215
    #[test]
    fn copying_a_template_brings_every_value_except_name_and_password() {
        let template = ServerConfigEntry {
            name: "origem".to_string(),
            host: "db.interno".to_string(),
            port: 5544,
            username: "leitor".to_string(),
            databases: vec!["a".to_string(), "b".to_string()],
            fetch_all_databases: true,
            exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
            ssl_mode: "Require".to_string(),
            timeout: 45,
            max_parallelism: 8,
        };

        let mut form = Form::new();
        form.draft.name = "novo".to_string();
        form.copy_from(&template);

        assert_eq!(form.draft.name, "novo", "o nome é do servidor novo");
        assert!(form.password.is_empty(), "senha não é copiada");

        let entry = form.to_entry();
        assert_eq!(entry.host, "db.interno");
        assert_eq!(entry.port, 5544);
        assert_eq!(entry.username, "leitor");
        assert_eq!(entry.databases, vec!["a".to_string(), "b".to_string()]);
        assert!(entry.fetch_all_databases);
        assert_eq!(
            entry.exclude_patterns,
            vec!["template*".to_string(), "postgres".to_string()]
        );
        assert_eq!(entry.ssl_mode, "Require");
        assert_eq!(entry.timeout, 45);
        assert_eq!(entry.max_parallelism, 8);

        assert_eq!(
            form.template.as_deref(),
            Some("origem"),
            "o formulário lembra de onde copiou"
        );
    }

    /// Depois de copiar, os campos continuam editáveis e valem para o save.
    ///
    /// CONTRACT: IF-215
    #[test]
    fn copied_values_can_be_changed_before_saving() {
        let template = ServerConfigEntry {
            name: "origem".to_string(),
            host: "antigo".to_string(),
            port: 5432,
            ..ServerConfigEntry::default()
        };

        let mut form = Form::new();
        form.copy_from(&template);
        form.draft.host = "novo-host".to_string();
        form.databases = "so-esta".to_string();

        let entry = form.to_entry();
        assert_eq!(entry.host, "novo-host", "a edição vence o modelo");
        assert_eq!(entry.databases, vec!["so-esta".to_string()]);
    }

    #[test]
    fn the_form_splits_the_lists_like_the_cli() {
        let mut form = Form::new();
        form.draft.name = "dev".to_string();
        form.draft.host = "localhost".to_string();
        form.databases = "a, b ,".to_string();
        form.exclude_patterns = "template*, postgres".to_string();

        let entry = form.to_entry();

        assert_eq!(entry.databases, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(
            entry.exclude_patterns,
            vec!["template*".to_string(), "postgres".to_string()]
        );
    }

    #[test]
    fn editing_keeps_the_original_name_and_prefills_the_lists() {
        let server = ServerConfigEntry {
            name: "dev".to_string(),
            host: "localhost".to_string(),
            databases: vec!["a".to_string(), "b".to_string()],
            exclude_patterns: vec!["x*".to_string()],
            ..ServerConfigEntry::default()
        };

        let form = Form::edit(&server);

        assert_eq!(form.original.as_deref(), Some("dev"));
        assert_eq!(form.databases, "a, b");
        assert_eq!(form.exclude_patterns, "x*");
        assert_eq!(form.to_entry().name, "dev");
    }

    #[test]
    fn the_list_arrives_from_the_core() {
        let state = State::new(vec![ServerConfigEntry {
            name: "dev".to_string(),
            ..ServerConfigEntry::default()
        }]);

        assert_eq!(state.servers().len(), 1);
    }
}
