//! Frontend gráfico do Mustel (egui).
//!
//! Consome os mesmos casos de uso do `mustel-core` em processo, sem IPC (D9): recebe eventos por um
//! [`ChannelSink`], roda o trabalho em um runtime tokio de background e redesenha via
//! `ctx.request_repaint()`. Ligado ao binário `mustel` pelo subcomando `gui` (T41).
//!
//! Como o encanamento se junta:
//!
//! ```text
//! janela (thread principal, egui)            runtime tokio de background
//! App::update ──lê── Receiver<GuiEvent>      job: usecases::query_run::run(&request, sink, token)
//!             ──lê── Receiver<TestResult>              │
//!                                                   sink: ChannelSink
//!                                                      │ envia + request_repaint()
//!                                                      ▼
//!                                              a janela acorda e desenha
//! ```
//!
//! A crate inteira fica atrás da feature `gui` do binário: `--no-default-features` compila um CLI sem
//! `egui` (verificado no T41).

mod about;
pub mod actions;
mod activity_log;
pub mod ai_panel;
mod app;
pub mod desktop;
pub mod events;
pub mod fonts;
pub mod icons;
pub mod picker;
pub mod query;
pub mod reference;
pub mod request;
pub mod runtime;
pub mod schema_cache;
pub mod screens;
pub mod search_combo;
pub mod selection;
pub mod smooth_scroll;
pub mod sql_completion;
pub mod sql_editor;
pub mod tabs;
mod test_button;
pub mod theme;
mod toast;
mod update;

pub use app::App;
pub use events::{ChannelSink, GuiEvent};
pub use runtime::Runtime;

/// Endereço de suporte do produto (relatório de atividade e About).
pub(crate) const SUPPORT_EMAIL: &str = "support@mustelguild.dev";

/// Erro ao abrir a janela.
#[derive(Debug, thiserror::Error)]
pub enum GuiError {
    /// O runtime tokio de background não pôde ser criado.
    #[error("failed to start the background runtime: {0}")]
    Runtime(#[from] std::io::Error),
    /// O `eframe` recusou abrir a janela (o evento mais comum é não haver servidor gráfico).
    #[error("failed to open the window: {0}")]
    Window(String),
}

/// Ícone da janela, embutido no binário.
///
/// É uma versão **derivada** de `assets/Mustel_icon.png` (raiz do repositório): quadrada em 256 px,
/// com a arte encaixada e centralizada, que é o que a barra de título e a barra de tarefas usam. Fica
/// dentro da crate de propósito — `include_bytes!` apontando para fora do diretório da crate não
/// entraria no pacote do `cargo publish`, e o build a partir do crate publicado quebraria.
///
/// Se o ícone master mudar, regenere este: encaixe a arte num quadrado de 256×256, fundo
/// transparente, e salve sobre `crates/mustel-gui/assets/icon.png`.
const WINDOW_ICON: &[u8] = include_bytes!("../assets/icon.png");

/// Identificador da aplicação no Wayland, e nome do `.desktop` que a acompanha.
///
/// No Wayland não existe ícone por janela (o `xdg-shell` não tem esse pedido), então o
/// `with_icon` abaixo é ignorado e o compositor procura `mustel.desktop` pelo `app_id` para
/// descobrir a arte — sem esse casamento ele cai no ícone genérico do Wayland. O arquivo vive em
/// `assets/mustel.desktop`, dentro da crate, para entrar no `cargo publish` junto com o ícone.
const APP_ID: &str = "mustel";

/// Como a janela nasce: título, tamanho mínimo, ícone e **maximizada**.
///
/// Maximizada (e não `fullscreen`): a janela ocupa a tela mas continua com barra de título, dá para
/// desmaximizar e fechar pelo sistema — o que importa numa ferramenta de trabalho. O `fullscreen`
/// de verdade exigiria um atalho nosso para sair (o F11 do egui não existe).
fn viewport() -> egui::ViewportBuilder {
    let builder = egui::ViewportBuilder::default()
        .with_inner_size([900.0, 620.0])
        .with_min_inner_size({
            let min = crate::fonts::min_window_size();
            [min.x, min.y]
        })
        .with_title("Mustel")
        .with_app_id(APP_ID)
        .with_maximized(true);

    match eframe::icon_data::from_png_bytes(WINDOW_ICON) {
        Ok(icon) => builder.with_icon(icon),
        // Ícone é enfeite: PNG quebrado não pode impedir a janela de abrir.
        Err(_) => builder,
    }
}

/// Abre a janela e roda até ela ser fechada.
///
/// O log de atividade começa antes da janela e termina depois dela: um `app.start` sem `app.stop`
/// é a assinatura de um fechamento abrupto. O hook de panic grava no mesmo arquivo.
pub fn run() -> Result<(), GuiError> {
    let runtime = Runtime::new()?;
    let log = activity_store();

    if let Some(store) = &log {
        mustel_core::activity::crash::install(store.clone());
        let now = mustel_core::datetime::now_ms();
        let _ = store.prune(activity_log::DAYS, now);
        let _ = store.append(
            &mustel_core::activity::Entry::new(
                mustel_core::activity::Area::App,
                "app.start",
                mustel_core::activity::Status::Info,
            )
            .with_detail(format!("Mustel {}", env!("CARGO_PKG_VERSION"))),
        );
    }

    let options = eframe::NativeOptions {
        viewport: viewport(),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Mustel",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, runtime)?))),
    )
    .map_err(|error| GuiError::Window(error.to_string()));

    if let Some(store) = &log {
        let _ = store.append(&mustel_core::activity::Entry::new(
            mustel_core::activity::Area::App,
            "app.stop",
            mustel_core::activity::Status::Info,
        ));
    }

    result
}

/// O store do log padrão, quando o SO expõe uma pasta de estado.
fn activity_store() -> Option<mustel_core::activity::Store> {
    mustel_core::activity::log_dir().map(mustel_core::activity::Store::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A janela nasce maximizada, com título e ícone.
    ///
    /// O ícone é conferido **decodificando** o PNG embutido: um asset renomeado ou corrompido
    /// passaria despercebido (a janela abriria sem ícone), e é justamente o que este teste pega.
    /// CONTRACT: IF-213
    #[test]
    fn the_window_starts_maximized_with_the_mustel_icon() {
        let viewport = viewport();

        assert_eq!(
            viewport.maximized,
            Some(true),
            "a janela abre ocupando a tela"
        );
        assert_eq!(viewport.fullscreen, None, "maximizada, não fullscreen");
        assert_eq!(viewport.title.as_deref(), Some("Mustel"));

        let icon = viewport.icon.expect("o ícone do Mustel entra na janela");
        assert!(
            icon.width >= 64 && icon.height >= 64,
            "ícone grande o bastante para a barra de tarefas: {}x{}",
            icon.width,
            icon.height
        );
        assert_eq!(
            icon.rgba.len(),
            (icon.width as usize) * (icon.height as usize) * 4,
            "RGBA consistente com as dimensões"
        );
    }

    /// O `app_id` casa com o `.desktop`, e é isso que dá ícone no Wayland.
    ///
    /// No Wayland o `with_icon` é ignorado: o compositor resolve a arte pelo `app_id`, procurando um
    /// `.desktop` com o mesmo nome. Sem esse casamento a janela fica com o ícone genérico do Wayland,
    /// e é um problema que só aparece na janela — nenhum teste de unidade do resto do app veria.
    /// CONTRACT: IF-257
    #[test]
    fn the_window_app_id_matches_the_desktop_entry() {
        let viewport = viewport();

        assert_eq!(
            viewport.app_id.as_deref(),
            Some(APP_ID),
            "o compositor procura o .desktop pelo app_id"
        );

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(format!("{APP_ID}.desktop"));
        assert!(
            path.exists(),
            "o compositor procura `{APP_ID}.desktop`; o arquivo precisa ter esse nome"
        );

        let desktop = include_str!("../assets/mustel.desktop");
        assert!(
            desktop.contains("Icon=mustel"),
            "o .desktop precisa apontar para o ícone instalado como `mustel`"
        );
        assert!(
            desktop.contains("Exec=mustel gui"),
            "o .desktop precisa abrir o subcomando `gui`"
        );
    }
}
