//! O painel de conversa com a IA (aba **Query**): estado e desenho.
//!
//! O painel é desenhado pelo [`App`](crate::app::App) — é irmão do `Panel::left`, e um painel do
//! egui precisa ser criado **antes** do `CentralPanel`. Por isso a conversa mora aqui, e não na tela
//! de query: a tela é dona do **editor**, o painel é dono da conversa (D11).
//!
//! O que este módulo garante, e os testes provam sem janela:
//! - o contexto da sessão é **pinado** ao começar; trocar os combos só **alerta** (D20);
//! - o system prompt é remontado a cada envio a partir do DDL da sessão (nunca guardado);
//! - uma falha mantém o transcript, para tentar de novo sem perder a conversa.

use mustel_core::config::model::AiProfile;
use mustel_core::usecases::ai::{self, ChatMessage};
use mustel_core::usecases::ai_session::AiSession;
use mustel_core::usecases::ddl::Ddl;

use crate::request::Request;

/// Em que pé está o turno.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Turn {
    /// Nada em andamento: esperando o dono escrever.
    #[default]
    Idle,
    /// Pedido enviado; o painel mostra que está pensando.
    Sending,
}

/// O estado do painel.
#[derive(Debug, Clone, Default)]
pub struct AiChat {
    /// Painel visível?
    pub open: bool,
    /// A conversa (mensagens + contexto pinado).
    pub session: AiSession,
    /// O que está **selecionado** nos combos (pode divergir do contexto da sessão).
    pub selected_server: String,
    /// Banco selecionado nos combos.
    pub selected_database: String,
    /// Perfil selecionado nos combos.
    pub selected_profile: String,
    /// DDL da sessão (vem do cache em disco ou do catálogo).
    pub ddl: Option<Ddl>,
    /// O DDL carregado passou do limite de aviso (`ai::LARGE_DDL_CHARS`)?
    ///
    /// Calculado **uma vez** em [`AiChat::apply_ddl`]: contar caracteres a cada quadro num schema
    /// grande seria desperdício, e o painel se desenha continuamente.
    pub ddl_is_large: bool,
    /// Turno em andamento.
    pub turn: Turn,
    /// Último erro, mostrado acima do campo de envio.
    pub error: Option<String>,
    /// Texto do campo de envio / de "peça um ajuste".
    pub input: String,
    /// O aviso de contexto divergente está dispensado (o dono escolheu `Keep talking`)?
    pub alert_dismissed: bool,
}

impl AiChat {
    /// Carrega o estado a partir da sessão gravada.
    pub fn from_session(session: AiSession, profiles: &[AiProfile]) -> Self {
        let selected_profile = session.profile_id.clone();
        let selected_profile = if profiles
            .iter()
            .any(|profile| profile.id == selected_profile)
        {
            selected_profile
        } else {
            profiles
                .first()
                .map(|profile| profile.id.clone())
                .unwrap_or_default()
        };
        Self {
            open: session.panel_open,
            selected_server: session.server.clone(),
            selected_database: session.database.clone(),
            selected_profile,
            session,
            ..Self::default()
        }
    }

    /// A sessão tem conversa ou contexto?
    pub fn has_session(&self) -> bool {
        !self.session.is_empty()
    }

    /// A seleção dos combos diverge do contexto pinado da sessão?
    pub fn context_diverged(&self) -> bool {
        self.has_session()
            && (self.selected_server != self.session.server
                || self.selected_database != self.session.database)
    }

    /// O aviso de contexto divergente deve aparecer?
    pub fn shows_context_alert(&self) -> bool {
        self.context_diverged() && !self.alert_dismissed
    }

    /// Dispensa o aviso **sem** trocar o contexto (`Keep talking`).
    pub fn keep_talking(&mut self) {
        self.alert_dismissed = true;
    }

    /// Começa uma sessão nova: adota a seleção atual e apaga a conversa.
    pub fn start_new_session(&mut self) {
        self.session = AiSession {
            server: self.selected_server.clone(),
            database: self.selected_database.clone(),
            profile_id: self.selected_profile.clone(),
            panel_open: self.open,
            messages: Vec::new(),
        };
        self.ddl = None;
        self.ddl_is_large = false;
        self.error = None;
        self.alert_dismissed = false;
        self.input.clear();
        self.turn = Turn::Idle;
    }

    /// Guarda o DDL carregado (e o contexto a que ele pertence).
    ///
    /// Se ainda não há sessão, o DDL que chega **define** o contexto: é o caso de abrir o painel e
    /// escolher banco antes de conversar.
    pub fn apply_ddl(&mut self, ddl: Ddl) {
        if self.session.server.is_empty() {
            self.session.server = self.selected_server.clone();
            self.session.database = self.selected_database.clone();
            self.session.profile_id = self.selected_profile.clone();
        }
        self.ddl_is_large = ai::is_large_schema(&ddl.text);
        self.ddl = Some(ddl);
        self.error = None;
    }

    /// Registra um erro de turno, **mantendo** a conversa (dá para tentar de novo).
    pub fn apply_error(&mut self, error: String) {
        self.turn = Turn::Idle;
        self.error = Some(error);
    }

    /// Registra a resposta da IA como mais uma mensagem.
    pub fn apply_reply(&mut self, sql: String) {
        self.session.messages.push(ChatMessage::assistant(sql));
        self.turn = Turn::Idle;
        self.error = None;
        self.input.clear();
    }

    /// Prepara o envio: vê o que pedir e passa para `Sending`.
    ///
    /// Devolve o `Request` quando há o que enviar; caso contrário devolve o motivo em `error`.
    pub fn ask(&mut self) -> Option<Request> {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            self.error = Some("Type what you need first.".to_string());
            return None;
        }
        if self.session.server.is_empty() || self.session.database.is_empty() {
            self.error = Some("Pick a database first.".to_string());
            return None;
        }
        if self.session.profile_id.is_empty() {
            self.error = Some("Pick an AI profile first (Settings › AI).".to_string());
            return None;
        }
        let ddl = match &self.ddl {
            Some(ddl) => ddl,
            None => {
                self.error = Some("Load the schema first.".to_string());
                return None;
            }
        };

        self.session.messages.push(ChatMessage::user(text));
        self.error = None;
        self.turn = Turn::Sending;

        Some(Request::GenerateSql {
            profile: self.session.profile_id.clone(),
            ddl: ddl.text.clone(),
            messages: self.session.messages.clone(),
        })
    }
}

/// Desenha o painel e devolve o pedido que ele quer fazer.
///
/// `profiles` é a lista do `gui.ai`; `servers` é o cadastro; `editor_has_text` decide o rótulo do
/// botão que joga o SQL no editor (com texto lá dentro, ele avisa que vai **substituir**).
pub fn ui(
    ui: &mut egui::Ui,
    chat: &mut AiChat,
    servers: &[mustel_core::config::ServerConfigEntry],
    profiles: &[AiProfile],
    databases: &[String],
    editor_has_text: bool,
    other_job: Option<&str>,
) -> Option<Request> {
    let mut request = None;
    // O painel tem largura fixa: o conteúdo **não** pode crescer além dela (senão transborda sobre o
    // editor). Tudo daqui para baixo vive dentro desta largura.
    let width = ui.available_width();
    ui.set_max_width(width);
    // E nem da altura: recolhidos os campos do topo, o transcript empurrava o resto para fora do
    // painel. Recortar garante que o painel inteiro fica do lado dele da divisória.
    ui.set_clip_rect(ui.max_rect());
    // Barra sólida: a padrão do egui é `floating`, que só aparece enquanto se rola — numa conversa
    // longa, quem abre o painel não vê que há mais abaixo. Sólida, e só quando o transcript não
    // cabe (é o que `VisibleWhenNeeded` faz por padrão).
    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();

    ui.horizontal(|ui| {
        // O título é o nome do botão que abria o painel: o gesto saiu da linha do `Run query` e
        // virou o próprio painel (o ícone à direita esconde, e o handle da barra traz de volta).
        ui.heading("AI Generate SQL");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::icons::button(ui, crate::icons::Icon::Sparkles)
                .on_hover_text("Hide the AI panel")
                .clicked()
            {
                request = Some(Request::ToggleAiPanel);
            }
        });
    });
    ui.separator();

    // Sem perfil configurado, o painel se explica em vez de ficar morto (D17).
    if profiles.is_empty() {
        ui.weak("No AI profile configured yet.");
        if ui.button("Open Settings").clicked() {
            request = Some(Request::OpenSettings);
        }
        return request;
    }

    // Cabeçalho: contexto da sessão.
    // Duas colunas (rótulo | combo), como os formulários do resto da janela: cabe na largura do
    // painel e os combos não empurram o conteúdo para fora dele.
    let combo = crate::search_combo::Size {
        button: (width - 80.0).max(140.0),
        popup: 320.0,
    };

    egui::Grid::new("ai_context")
        .num_columns(2)
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            ui.label("Server");
            let server_items: Vec<crate::search_combo::Item<'_>> = servers
                .iter()
                .map(|server| {
                    crate::search_combo::Item::new(
                        server.name.as_str(),
                        server.name == chat.selected_server,
                    )
                })
                .collect();
            if let Some(index) = crate::search_combo::search_combo_sized(
                ui,
                "ai_server",
                &chat.selected_server,
                &server_items,
                true,
                combo,
            ) {
                chat.selected_server = servers[index].name.clone();
                chat.selected_database.clear();
                chat.alert_dismissed = false;
            }
            ui.end_row();

            ui.label("Database");
            let database_items: Vec<crate::search_combo::Item<'_>> = databases
                .iter()
                .map(|database| {
                    crate::search_combo::Item::new(
                        database.as_str(),
                        database == &chat.selected_database,
                    )
                })
                .collect();
            if let Some(index) = crate::search_combo::search_combo_sized(
                ui,
                "ai_database",
                &chat.selected_database,
                &database_items,
                true,
                combo,
            ) {
                chat.selected_database = databases[index].clone();
                chat.alert_dismissed = false;
            }
            ui.end_row();

            ui.label("Profile");
            let profile_items: Vec<crate::search_combo::Item<'_>> = profiles
                .iter()
                .map(|profile| {
                    crate::search_combo::Item::new(
                        profile.name.as_str(),
                        profile.id == chat.selected_profile,
                    )
                })
                .collect();
            let current = profiles
                .iter()
                .find(|profile| profile.id == chat.selected_profile)
                .map(|profile| profile.name.clone())
                .unwrap_or_default();
            if let Some(index) = crate::search_combo::search_combo_sized(
                ui,
                "ai_profile",
                &current,
                &profile_items,
                true,
                combo,
            ) {
                chat.selected_profile = profiles[index].id.clone();
            }
            ui.end_row();
        });

    ui.horizontal(|ui| {
        match &chat.ddl {
            Some(ddl) => {
                let size = ddl.text.len() / 1024;
                let date = ddl.dumped_at.get(..10).unwrap_or(&ddl.dumped_at);
                ui.weak(format!("DDL {size} KB · {date}"));
            }
            None => {
                ui.weak("No schema loaded");
            }
        }
        if crate::icons::button(ui, crate::icons::Icon::Refresh)
            .on_hover_text("Download the schema again")
            .clicked()
            && !chat.selected_database.is_empty()
        {
            request = Some(Request::LoadDdl {
                server: chat.selected_server.clone(),
                database: chat.selected_database.clone(),
                refresh: true,
            });
        }
    });

    if let Some(ddl) = &chat.ddl
        && chat.ddl_is_large
    {
        let size = ddl.text.len() / 1024;
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "Large schema ({size} KB): the whole schema is sent with every message, so the \
                     provider may refuse it for context size."
                ))
                .color(crate::theme::warning(ui)),
            )
            .wrap(),
        );
    }

    if chat.ddl.is_none() && !chat.selected_database.is_empty() {
        ui.weak("Load the schema to start the conversation.");
        if ui.button("Load schema").clicked() {
            request = Some(Request::LoadDdl {
                server: chat.selected_server.clone(),
                database: chat.selected_database.clone(),
                refresh: false,
            });
        }
    }

    // Aviso de contexto divergente (D20): o dono escolhe reiniciar ou seguir.
    if chat.shows_context_alert() {
        ui.separator();
        ui.colored_label(
            crate::theme::warning(ui),
            format!(
                "This conversation uses {} / {}. You selected {} / {}.",
                chat.session.server,
                chat.session.database,
                chat.selected_server,
                chat.selected_database
            ),
        );
        ui.horizontal(|ui| {
            if ui.button("Start new session").clicked() {
                chat.start_new_session();
                request = Some(Request::LoadDdl {
                    server: chat.selected_server.clone(),
                    database: chat.selected_database.clone(),
                    refresh: false,
                });
            }
            if ui.button("Keep talking").clicked() {
                chat.keep_talking();
            }
        });
    }

    ui.separator();

    // Envio: logo abaixo do cabeçalho, sempre à vista (o painel ocupa a altura toda).
    let sending = chat.turn == Turn::Sending;
    ui.horizontal(|ui| {
        let hint = if chat.session.messages.is_empty() {
            "What do you need?"
        } else {
            "Ask for a change…"
        };
        // Largura **finita**: `f32::INFINITY` num painel estreito faz o `TextEdit` pedir a largura
        // toda e o layout cresce além do painel (transbordava sobre o editor). Multilinha com duas
        // linhas de partida: a caixa acompanha o texto (é uma conversa, não um campo de busca).
        //
        // Alinhamento: o `hint` e o texto digitado compartilham o alinhamento do `TextEdit` (é
        // assim que o egui os pinta), então centrar na vertical põe o placeholder no meio da caixa
        // de duas linhas e o primeiro caractere nasce no lugar dele — sem pulo. Centralizar na
        // horizontal não: o texto é frase, e centrado ele leria como campo de busca.
        ui.add_enabled(
            !sending,
            egui::TextEdit::multiline(&mut chat.input)
                .hint_text(hint)
                .desired_rows(2)
                .align(egui::Align2::LEFT_CENTER)
                .desired_width((width - 80.0).max(120.0)),
        );
        if ui
            .add_enabled(!sending && other_job.is_none(), egui::Button::new("Send"))
            .clicked()
            && let Some(built) = chat.ask()
        {
            request = Some(built);
        }
        if sending {
            ui.spinner();
        }
    });

    // Erro e aviso ficam **acima** do transcript: depois dele, com o painel ocupando a altura
    // toda, ficariam fora da tela (foi por isso que "Load schema" pareceu não fazer nada).
    if let Some(error) = &chat.error {
        ui.add(egui::Label::new(egui::RichText::new(error).color(crate::theme::danger(ui))).wrap());
    }
    if let Some(label) = other_job {
        ui.weak(format!("Another job is running: {label}"));
    }

    // Conversa: o campo de envio já foi desenhado acima, então o transcript fica com o resto —
    // e não pode empurrar nada para fora do painel.
    egui::ScrollArea::vertical()
        .id_salt("ai_transcript")
        .max_height(ui.available_height().max(60.0))
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            let mut insert: Option<String> = None;
            for message in &chat.session.messages {
                match message.role {
                    mustel_core::usecases::ai::Role::User => {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&message.content).strong()).wrap(),
                        );
                    }
                    mustel_core::usecases::ai::Role::Assistant => {
                        let mut job = crate::sql_editor::layout_job(
                            ui,
                            &message.content,
                            ui.available_width(),
                        );
                        job.wrap.max_width = ui.available_width();
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        ui.label(galley);
                        ui.horizontal(|ui| {
                            let label = if editor_has_text {
                                "Replace editor text"
                            } else {
                                "Insert in editor"
                            };
                            if crate::icons::button_text(ui, crate::icons::Icon::Check, label)
                                .clicked()
                            {
                                insert = Some(message.content.clone());
                            }
                            if crate::icons::button(ui, crate::icons::Icon::Copy)
                                .on_hover_text("Copy this SQL")
                                .clicked()
                            {
                                ui.ctx().copy_text(message.content.clone());
                            }
                        });
                    }
                    mustel_core::usecases::ai::Role::System => {}
                }
                ui.add_space(6.0);
            }
            if let Some(sql) = insert {
                request = Some(Request::InsertEditorText(sql));
            }
        });

    request
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ddl() -> Ddl {
        Ddl {
            text: "CREATE TABLE t (id integer);".to_string(),
            dumped_at: "2026-10-24T21:10:00.0000000Z".to_string(),
        }
    }

    fn novo_chat() -> AiChat {
        let mut chat = AiChat {
            open: true,
            selected_server: "Local".to_string(),
            selected_database: "dev".to_string(),
            selected_profile: "rapido".to_string(),
            ..AiChat::default()
        };
        chat.apply_ddl(ddl());
        chat
    }

    fn profile() -> AiProfile {
        AiProfile {
            id: "rapido".to_string(),
            name: "Rápido".to_string(),
            model: "gpt-4o-mini".to_string(),
            ..AiProfile::default()
        }
    }

    /// A sessão nasce do DDL carregado e o pedido leva o perfil e o histórico.
    #[test]
    fn asking_builds_the_request_with_the_pinned_context() {
        let mut chat = novo_chat();
        chat.input = "vendas por mês".to_string();

        let request = chat.ask().expect("pedido");

        match request {
            Request::GenerateSql {
                profile,
                ddl,
                messages,
            } => {
                assert_eq!(profile, "rapido");
                assert!(ddl.contains("CREATE TABLE t"));
                assert_eq!(messages.len(), 1, "só a pergunta entrou");
                assert_eq!(messages[0].content, "vendas por mês");
            }
            other => panic!("pedido errado: {other:?}"),
        }
        assert_eq!(chat.turn, Turn::Sending);
    }

    /// A resposta vira mensagem e volta para `Idle`; o erro mantém a conversa.
    #[test]
    fn a_reply_lands_in_the_transcript_and_an_error_keeps_it() {
        let mut chat = novo_chat();
        chat.input = "x".to_string();
        let _ = chat.ask();
        chat.apply_reply("SELECT 1;".to_string());

        assert_eq!(chat.turn, Turn::Idle);
        assert_eq!(chat.session.messages.len(), 2);
        assert_eq!(chat.input, "", "o campo esvazia depois de responder");

        chat.input = "ajusta".to_string();
        let _ = chat.ask();
        chat.apply_error("429".to_string());

        assert_eq!(chat.turn, Turn::Idle);
        assert_eq!(chat.error.as_deref(), Some("429"));
        assert_eq!(chat.session.messages.len(), 3, "a conversa não se perde");
    }

    /// Trocar o combo **não** muda o contexto pinado; avisa, e `Keep talking` só dispensa.
    #[test]
    fn changing_the_selection_alerts_without_switching_the_context() {
        let mut chat = novo_chat();
        chat.input = "x".to_string();
        let _ = chat.ask();
        chat.apply_reply("SELECT 1;".to_string());

        chat.selected_database = "outro".to_string();

        assert!(chat.shows_context_alert(), "o aviso aparece");
        assert_eq!(chat.session.database, "dev", "o contexto pinado não muda");

        chat.keep_talking();
        assert!(!chat.shows_context_alert(), "dispensado");
        assert_eq!(chat.session.database, "dev", "e continua pinado");

        // Seguir conversando usa o schema da sessão.
        chat.input = "mais".to_string();
        let request = chat.ask().expect("pedido");
        match request {
            Request::GenerateSql { ddl, .. } => assert!(ddl.contains("CREATE TABLE t")),
            other => panic!("{other:?}"),
        }
    }

    /// `Start new session` adota a seleção e apaga a conversa.
    #[test]
    fn starting_a_new_session_clears_and_repins() {
        let mut chat = novo_chat();
        chat.input = "x".to_string();
        let _ = chat.ask();
        chat.apply_reply("SELECT 1;".to_string());
        chat.selected_database = "outro".to_string();

        chat.start_new_session();

        assert!(chat.session.messages.is_empty());
        assert_eq!(chat.session.database, "outro");
        assert!(chat.ddl.is_none(), "o DDL do banco anterior não vale");
        assert!(!chat.shows_context_alert(), "agora não diverge");
    }

    /// Sem DDL, sem banco ou sem perfil, o envio explica o motivo em vez de mandar lixo.
    #[test]
    fn asking_without_context_explains_itself() {
        let mut empty = AiChat {
            input: "x".to_string(),
            ..AiChat::default()
        };
        assert!(empty.ask().is_none());
        assert_eq!(empty.error.as_deref(), Some("Pick a database first."));

        let mut chat = novo_chat();
        chat.ddl = None;
        chat.input = "x".to_string();
        assert!(chat.ask().is_none());
        assert_eq!(chat.error.as_deref(), Some("Load the schema first."));

        let mut chat = novo_chat();
        chat.session.profile_id.clear();
        chat.input = "x".to_string();
        assert!(chat.ask().is_none());
        assert_eq!(
            chat.error.as_deref(),
            Some("Pick an AI profile first (Settings › AI).")
        );

        let mut chat = novo_chat();
        chat.input = "   ".to_string();
        assert!(chat.ask().is_none());
        assert_eq!(chat.error.as_deref(), Some("Type what you need first."));
    }

    /// O aviso de schema grande aparece no cabeçalho — e só quando o DDL passa do limite.
    #[test]
    fn a_large_schema_shows_a_warning_in_the_header() {
        let ctx = egui::Context::default();

        fn texts_of(ctx: &egui::Context, chat: &mut AiChat) -> Vec<String> {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_max_width(360.0);
                    let _ = super::ui(ui, chat, &[], &[profile()], &[], false, None);
                });
            });
            let mut texts: Vec<String> = Vec::new();
            fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
                match shape {
                    egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            walk(shape, out);
                        }
                    }
                    _ => {}
                }
            }
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut texts);
            }
            output.textures_delta.clear();
            texts
        }

        let mut chat = novo_chat();
        let small = texts_of(&ctx, &mut chat);
        assert!(
            !small.iter().any(|text| text.contains("Large schema")),
            "o DDL de teste é pequeno e não pode avisar: {small:?}"
        );

        chat.apply_ddl(Ddl {
            text: "x".repeat(ai::LARGE_DDL_CHARS + 1),
            dumped_at: ddl().dumped_at,
        });
        assert!(chat.ddl_is_large, "o limite foi passado");

        let large = texts_of(&ctx, &mut chat);
        assert!(
            large.iter().any(|text| text.contains("Large schema")),
            "o aviso precisa aparecer no cabeçalho: {large:?}"
        );
    }

    /// O cabeçalho se chama **`AI Generate SQL`** e o botão do canto esconde o painel.
    ///
    /// O gesto saiu da linha do `Run query` (pedido do dono): o título carrega o nome, e o ícone
    /// `sparkles` à direita alterna — quem reabre é o botão do alto da aba.
    /// CONTRACT: IF-247
    #[test]
    fn the_header_is_the_generate_button_and_its_icon_hides_the_panel() {
        let ctx = egui::Context::default();
        let mut chat = novo_chat();
        chat.open = true;

        let mut texts: Vec<String> = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_max_width(360.0);
                let _ = super::ui(ui, &mut chat, &[], &[profile()], &[], false, None);
            });
        });
        texts.clear();
        {
            fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
                match shape {
                    egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            walk(shape, out);
                        }
                    }
                    _ => {}
                }
            }
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut texts);
            }
        }
        output.textures_delta.clear();

        assert!(
            texts.iter().any(|text| text == "AI Generate SQL"),
            "o título é o nome do antigo botão: {texts:?}"
        );
        // O fechar (`x-mark`) saiu do cabeçalho; o canto agora é o sparkles.
        assert!(
            !texts
                .iter()
                .any(|text| text == &crate::icons::Icon::Close.glyph().to_string()),
            "o botão de fechar (`x-mark`) não mora mais no cabeçalho: {texts:?}"
        );
        // O título é **só texto**: o ícone antes dele saiu (o botão de colapso já carrega a
        // identidade do painel).
        assert_eq!(
            texts
                .iter()
                .filter(|text| *text == &crate::icons::Icon::Sparkles.glyph().to_string())
                .count(),
            1,
            "um único sparkles, o do botão de colapso: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "AI Generate SQL"),
            "e o título continua sendo o nome: {texts:?}"
        );
    }

    /// A sessão gravada volta com o perfil ativo, e um perfil removido cai no primeiro.
    #[test]
    fn a_loaded_session_keeps_the_open_state_and_falls_back_on_the_profile() {
        let session = AiSession {
            server: "Local".to_string(),
            database: "dev".to_string(),
            profile_id: "sumiu".to_string(),
            panel_open: true,
            messages: vec![ChatMessage::user("x")],
        };

        let chat = AiChat::from_session(session, &[profile()]);

        assert!(chat.open);
        assert_eq!(chat.selected_profile, "rapido");
        assert_eq!(chat.session.messages.len(), 1);
    }

    /// O placeholder fica no **meio** da caixa de envio, não colado no topo.
    ///
    /// A caixa tem duas linhas de partida: um `hint` de uma linha no alto deixava a metade de
    /// baixo vazia, com cara de caixa mal usada. Como o `hint` e o texto digitado compartilham o
    /// alinhamento do `TextEdit`, centrar na vertical resolve a caixa vazia sem mover o primeiro
    /// caractere para outro lugar.
    ///
    /// Medida: o centro do texto do placeholder contra o centro do `Send`, que divide a mesma linha
    /// e é centrado nela. Colado no topo, o `hint` ficaria meia caixa acima (≈12 px).
    #[test]
    fn the_placeholder_sits_in_the_middle_of_the_prompt_box() {
        let ctx = egui::Context::default();
        let mut chat = novo_chat();
        chat.open = true;

        let mut texts: Vec<(String, egui::Pos2, f32)> = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_max_width(360.0);
                let _ = super::ui(ui, &mut chat, &[], &[profile()], &[], false, None);
            });
        });
        texts.clear();
        {
            fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2, f32)>) {
                match shape {
                    egui::Shape::Text(text) => out.push((
                        text.galley.text().to_string(),
                        text.pos,
                        text.galley.size().y,
                    )),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            walk(shape, out);
                        }
                    }
                    _ => {}
                }
            }
            for clipped in &output.shapes {
                walk(&clipped.shape, &mut texts);
            }
        }
        output.textures_delta.clear();

        let center_y = |needle: &str| -> f32 {
            let found: Vec<f32> = texts
                .iter()
                .filter(|(text, _, _)| text == needle)
                .map(|(_, pos, height)| pos.y + height / 2.0)
                .collect();
            assert_eq!(found.len(), 1, "um só {needle:?} no painel: {texts:?}");
            found[0]
        };

        let hint = center_y("What do you need?");
        let send = center_y("Send");
        assert!(
            (hint - send).abs() <= 4.0,
            "o placeholder tem de estar centrado na caixa, na linha do `Send`: {hint} vs {send}"
        );
        assert!(
            !texts
                .iter()
                .any(|(text, _, _)| text.contains("sales per month")),
            "a instrução acima da caixa saiu; fica só o placeholder: {texts:?}"
        );
    }
}
