//! Aba **About**: versão, licença do produto e os textos legais que viajam com o binário.
//!
//! Por que os textos vivem **dentro do programa**: quem recebe só o executável (o `.zip`/`.tar.gz`
//! da release, ou uma cópia avulsa) precisa conseguir ler a licença sem depender de arquivos
//! vizinhos. A cópia embutida é a mesma que os scripts de empacotamento colocam no pacote.
//!
//! Os arquivos ficam **dentro da crate** porque `include_str!` apontando para fora do diretório da
//! crate não entraria no pacote do `cargo publish` (o mesmo motivo do ícone, em `lib.rs`).
//!
//! **Regeneração.** O `THIRD-PARTY-LICENSES.md` é gerado pelo `cargo-about`, não escrito à mão:
//!
//! ```text
//! cargo about -L warn generate -m crates/mustel/Cargo.toml about.hbs \
//!     -o crates/mustel-gui/assets/licenses/THIRD-PARTY-LICENSES.md
//! ```
//!
//! Sem `--target`: a lista é a **união** das plataformas, porque o mesmo texto viaja no binário do
//! Windows e no do Linux. Rode de novo quando o `Cargo.lock` mudar.
//!
//! `LICENSE` e `THIRD-PARTY-NOTICES.md` são cópias dos arquivos da raiz; a fonte da verdade
//! continua sendo a raiz, e é ela que o `publish.ps1`/`publish.sh` empacotam.

use std::sync::OnceLock;

/// Versão do produto, herdada do workspace pelo `mustel-gui`.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Repositório do projeto — o `Cargo.toml` é a fonte.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Autor(es) declarados no `Cargo.toml`.
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

/// Texto da licença do próprio Mustel.
const LICENSE: &str = include_str!("../assets/licenses/LICENSE");

/// Textos completos das licenças dos componentes compilados (união das plataformas).
const THIRD_PARTY_LICENSES: &str = include_str!("../assets/licenses/THIRD-PARTY-LICENSES.md");

/// Avisos curtos: o componente MPL-2.0 e as fontes embutidas.
const THIRD_PARTY_NOTICES: &str = include_str!("../assets/licenses/THIRD-PARTY-NOTICES.md");

/// Licença da fonte embutida no editor de SQL.
const FIRA_CODE_OFL: &str = include_str!("../assets/fonts/FiraCode-OFL.txt");

/// Um documento legal da aba.
pub(crate) struct Document {
    /// Título do bloco (o que abre e fecha).
    pub(crate) title: &'static str,
    /// Uma linha dizendo o que o documento cobre.
    pub(crate) summary: &'static str,
    /// O texto inteiro, como está no arquivo distribuído.
    pub(crate) body: &'static str,
}

/// Os documentos, na ordem em que aparecem na tela.
///
/// A licença do produto vem primeiro: é a pergunta que se faz ao abrir esta aba. O relatório de
/// terceiros vem em seguida e a fonte embutida por último, porque a OFL dela já está reproduzida
/// dentro do relatório — o bloco separado serve para achar o aviso da **Fira Code** sem rolar 400 KB.
pub(crate) const DOCUMENTS: [Document; 4] = [
    Document {
        title: "Mustel — GNU AGPL-3.0-only",
        summary: "The license of this program.",
        body: LICENSE,
    },
    Document {
        title: "Third-party licenses",
        summary: "Full license texts of every crate compiled into this binary, for every platform.",
        body: THIRD_PARTY_LICENSES,
    },
    Document {
        title: "Third-party notices",
        summary: "The short notices: the MPL-2.0 component and the embedded fonts.",
        body: THIRD_PARTY_NOTICES,
    },
    Document {
        title: "Fira Code — SIL Open Font License 1.1",
        summary: "The font embedded in the SQL editor.",
        body: FIRA_CODE_OFL,
    },
];

/// Desenha o conteúdo da aba (o título e o subtítulo da categoria já vieram de `settings`).
///
/// `selected` é o documento aberto no leitor; a escolha é estado da tela (não do arquivo), como a
/// própria categoria. Devolve o pedido de **check de atualização** quando o botão é clicado.
pub(crate) fn ui(ui: &mut egui::Ui, selected: &mut usize) -> Option<crate::request::Request> {
    let mut request = None;

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("Mustel {VERSION}"))
            .size(20.0)
            .strong(),
    );
    ui.label("Automation CLI for PostgreSQL queries, database backups and server settings.");
    ui.add_space(8.0);

    egui::Grid::new("about_meta")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("License");
            ui.label("AGPL-3.0-only");
            ui.end_row();

            ui.label("Author");
            ui.label(AUTHORS);
            ui.end_row();

            ui.label("Source");
            // Link de verdade: o clique abre no navegador do sistema (o `eframe` trata o
            // `OutputCommand::OpenUrl`). Texto = URL, para copiar ser seleção de texto, não botão.
            ui.hyperlink_to(REPOSITORY, REPOSITORY);
            ui.end_row();

            ui.label("Support");
            ui.horizontal(|ui| {
                // `ui.link` (sem URL): o e-mail **não** vai pelo `webbrowser`/`xdg-open`, que sem
                // um handler de `mailto:` abriria o navegador. O pedido vai para o `App`, que usa
                // o `xdg-email`/`FileProtocolHandler` e, sem cliente, copia o endereço.
                if ui.link(crate::SUPPORT_EMAIL).clicked() {
                    request = Some(crate::request::Request::ComposeSupportEmail);
                }
                if ui
                    .small_button("Copy")
                    .on_hover_text("Copy the address")
                    .clicked()
                {
                    ui.ctx().copy_text(crate::SUPPORT_EMAIL.to_string());
                }
            });
            ui.end_row();
        });

    ui.add_space(8.0);

    // Atualização: o check manual fica aqui (o toast automático pode ter sido fechado) e o link
    // leva às notas da release, que a janela não repete.
    ui.horizontal(|ui| {
        if ui.button("Check for updates").clicked() {
            request = Some(crate::request::Request::CheckForUpdates { manual: true });
        }
        ui.hyperlink_to("Release notes", format!("{REPOSITORY}/releases/latest"));
    });
    ui.add_space(8.0);

    ui.label(
        "This binary embeds Fira Code (SIL OFL 1.1), Heroicons (MIT) and the egui default fonts. \
         The complete texts are below.",
    );
    ui.add_space(8.0);

    // Um documento por vez: o leitor ocupa toda a altura que sobra do painel, em vez de dividir a
    // tela com quatro blocos recolhíveis (o texto ficava espremido em poucas linhas).
    let mut chosen = (*selected).min(DOCUMENTS.len() - 1);

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("about_document")
            .selected_text(DOCUMENTS[chosen].title)
            .width((ui.available_width() - 90.0).clamp(180.0, 360.0))
            .show_ui(ui, |ui| {
                for (index, option) in DOCUMENTS.iter().enumerate() {
                    ui.selectable_value(selected, index, option.title);
                }
            });
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(DOCUMENTS[chosen].body.to_string());
        }
    });

    // O seletor pode ter mudado a escolha **neste** quadro: relê antes de desenhar o texto, para o
    // painel não passar um quadro mostrando o documento anterior.
    chosen = (*selected).min(DOCUMENTS.len() - 1);
    let document = &DOCUMENTS[chosen];

    ui.label(egui::RichText::new(document.summary).weak());
    ui.add_space(4.0);

    viewer(ui, chosen);

    request
}

/// Mostra o texto inteiro num rolo **virtualizado** que ocupa o que resta do painel.
///
/// Um `Label` com 400 KB faria o egui montar a galley inteira a cada quadro; `show_rows` desenha
/// uma janela de linhas. O rolo é horizontal também porque texto de licença tem linha longa e
/// quebrar linha mudaria a altura de cada uma (e a virtualização depende de altura fixa).
fn viewer(ui: &mut egui::Ui, index: usize) {
    let lines = lines_of(index);
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let height = ui.available_height().max(row_height * 3.0);

    egui::ScrollArea::both()
        .id_salt(("about_body", index))
        .max_height(height)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, lines.len(), |ui, range| {
            for line in &lines[range] {
                ui.monospace(*line);
            }
        });
}

/// As linhas do documento, quebradas **uma vez** e guardadas.
///
/// Quebrar 400 KB a cada quadro desperdiçaria o orçamento de 60 fps da janela à toa.
fn lines_of(index: usize) -> &'static [&'static str] {
    static CACHE: OnceLock<Vec<Vec<&'static str>>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| {
        DOCUMENTS
            .iter()
            .map(|document| document.body.lines().collect())
            .collect()
    });
    &cache[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os quatro textos estão embutidos, inteiros, e são os das licenças certas.
    #[test]
    fn the_documents_are_the_license_texts() {
        assert_eq!(DOCUMENTS.len(), 4);

        assert!(
            LICENSE.contains("GNU AFFERO GENERAL PUBLIC LICENSE"),
            "a licença do produto é a AGPL"
        );
        assert!(
            THIRD_PARTY_LICENSES.contains("SIL Open Font License 1.1"),
            "o relatório cobre a fonte embutida do egui"
        );
        assert!(
            THIRD_PARTY_LICENSES.contains("MIT License") || THIRD_PARTY_LICENSES.contains("MIT"),
            "o relatório cobre as crates MIT"
        );
        assert!(
            FIRA_CODE_OFL.contains("SIL Open Font License"),
            "a Fira Code tem o seu próprio texto"
        );

        for document in &DOCUMENTS {
            assert!(!document.title.trim().is_empty(), "título vazio");
            assert!(!document.summary.trim().is_empty(), "resumo vazio");
            assert!(
                document.body.lines().count() > 10,
                "{} veio truncada",
                document.title
            );
        }
    }

    /// A quebra em linhas cobre **todos** os documentos e não perde a última linha.
    #[test]
    fn the_line_cache_covers_every_document() {
        for (index, document) in DOCUMENTS.iter().enumerate() {
            let lines = lines_of(index);
            assert_eq!(
                lines.len(),
                document.body.lines().count(),
                "a contagem de {} mudou",
                document.title
            );
            assert!(!lines.is_empty(), "{} ficou sem linhas", document.title);
        }
    }

    /// A aba desenha sem janela: versão, licença, repositório e o texto do documento escolhido.
    #[test]
    fn the_about_renders_without_a_window() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut selected = 0;
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| ui(panel, &mut selected));
        });
        output.textures_delta.clear();

        let mut painted = Vec::new();
        collect_text(&output.shapes, &mut painted);

        let everything = painted
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            everything.contains(&format!("Mustel {VERSION}")),
            "a versão aparece: {everything}"
        );
        assert!(
            everything.contains("AGPL-3.0-only"),
            "a licença aparece: {everything}"
        );
        assert!(
            everything.contains(REPOSITORY),
            "o repositório aparece: {everything}"
        );
        assert!(
            everything.contains(DOCUMENTS[0].title),
            "o documento aberto aparece no seletor: {everything}"
        );
        assert!(
            everything.contains("GNU AFFERO GENERAL PUBLIC LICENSE"),
            "o texto da licença está à vista, não só o título: {everything}"
        );
    }

    /// Trocar o documento no seletor troca o texto lido.
    #[test]
    fn choosing_a_document_shows_its_text() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut selected = 1;
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| ui(panel, &mut selected));
        });
        output.textures_delta.clear();

        let mut painted = Vec::new();
        collect_text(&output.shapes, &mut painted);
        let everything = painted
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            everything.contains(DOCUMENTS[1].title),
            "o seletor mostra o documento escolhido: {everything}"
        );
        assert!(
            everything.contains("Licenças de terceiros"),
            "o texto do relatório está à vista: {everything}"
        );
        assert!(
            !everything.contains("GNU AFFERO GENERAL PUBLIC LICENSE"),
            "o texto da AGPL não está mais na tela: {everything}"
        );
    }

    /// O leitor ocupa a altura que sobra do painel — não uma caixa de 240 px.
    ///
    /// A última linha pintada precisa chegar perto do rodapé da janela; com o teto antigo ela
    /// parava logo abaixo do cabeçalho, que era o que deixava o texto espremido.
    #[test]
    fn the_reader_takes_the_rest_of_the_panel() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut selected = 0;
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| ui(panel, &mut selected));
        });
        output.textures_delta.clear();

        let mut painted = Vec::new();
        collect_text(&output.shapes, &mut painted);

        let bottom = painted
            .iter()
            .map(|(_, position)| position.y)
            .fold(f32::MIN, f32::max);
        assert!(
            bottom > 450.0,
            "o texto tem de descer até o fim do painel (última linha em y={bottom})"
        );
        assert!(
            painted.len() > 25,
            "só {} linhas desenhadas — o leitor está curto",
            painted.len()
        );
    }

    /// O repositório é um **link**: clicar nele pede ao sistema para abrir a URL.
    ///
    /// O teste olha o pedido (`OutputCommand::OpenUrl`), não o navegador — em CI não há navegador,
    /// e o que interessa é que o widget se comporte como link.
    #[test]
    fn the_repository_is_a_clickable_link() {
        let ctx = egui::Context::default();
        let screen = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };

        let mut selected = 0;

        // Primeiro quadro: descobre onde o link foi pintado.
        let mut first = ctx.run_ui(screen(), |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| ui(panel, &mut selected));
        });
        first.textures_delta.clear();
        let mut painted = Vec::new();
        collect_text(&first.shapes, &mut painted);
        let position = painted
            .iter()
            .find(|(text, _)| text == REPOSITORY)
            .map(|(_, position)| *position)
            .expect("o repositório está na tela");
        let target = egui::pos2(position.x + 4.0, position.y + 6.0);

        // Segundo: clica nele.
        let mut second = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(target),
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..screen()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |panel| ui(panel, &mut selected));
            },
        );
        second.textures_delta.clear();

        assert!(
            second
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(
                    command,
                    egui::OutputCommand::OpenUrl(open) if open.url == REPOSITORY
                )),
            "o clique no repositório pede a abertura da URL: {:?}",
            second.platform_output.commands
        );
    }

    /// O endereço de suporte pede ao **app** para abrir o cliente de e-mail.
    ///
    /// Não pode ser um `mailto:` do `webbrowser`: sem um handler de `mailto:` configurado o
    /// `xdg-open` cai no navegador. O pedido é do app, que usa o caminho de e-mail do projeto e,
    /// sem cliente, copia o endereço.
    /// CONTRACT: IF-267
    #[test]
    fn the_support_address_asks_for_the_email_app() {
        let ctx = egui::Context::default();
        let screen = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let mut selected = 0;

        let mut first = ctx.run_ui(screen(), |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| {
                let _ = ui(panel, &mut selected);
            });
        });
        first.textures_delta.clear();
        let mut painted = Vec::new();
        collect_text(&first.shapes, &mut painted);
        let position = painted
            .iter()
            .find(|(text, _)| text == crate::SUPPORT_EMAIL)
            .map(|(_, position)| *position)
            .expect("o endereço de suporte está na tela");
        let target = egui::pos2(position.x + 4.0, position.y + 6.0);

        let mut clicked = None;
        let mut second = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(target),
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..screen()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |panel| {
                    clicked = ui(panel, &mut selected);
                });
            },
        );
        second.textures_delta.clear();

        assert_eq!(
            clicked,
            Some(crate::request::Request::ComposeSupportEmail),
            "o clique no suporte pede o cliente de e-mail ao app"
        );
    }

    /// O backend só abre links com a feature `links` do `eframe`.
    ///
    /// Sem ela o clique num `hyperlink_to` emite `OutputCommand::OpenUrl` e o `egui-winit` apenas
    /// registra um warning — como o app não tem logger, o clique fica em silêncio e **nenhum** link
    /// da janela abre. O teste do clique não pega isso: ele vê o comando emitido, não o SO abrindo.
    #[test]
    fn the_window_is_built_with_url_opening_enabled() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            manifest.contains("\"links\""),
            "o eframe precisa da feature `links` (egui-winit/webbrowser); sem ela os links do About \
             não abrem no sistema"
        );
    }

    /// O botão `Check for updates` devolve o pedido do check **manual** (o que ignora o estado
    /// local do updater).
    /// CONTRACT: IF-268
    #[test]
    fn the_update_button_asks_for_a_manual_check() {
        let ctx = egui::Context::default();
        let screen = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let mut selected = 0;

        let mut first = ctx.run_ui(screen(), |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| {
                let _ = ui(panel, &mut selected);
            });
        });
        first.textures_delta.clear();
        let mut painted = Vec::new();
        collect_text(&first.shapes, &mut painted);
        let position = painted
            .iter()
            .find(|(text, _)| text == "Check for updates")
            .map(|(_, position)| *position)
            .expect("o botão de update está na tela");
        let target = egui::pos2(position.x + 4.0, position.y + 6.0);

        let mut clicked = None;
        let mut second = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(target),
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..screen()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |panel| {
                    clicked = ui(panel, &mut selected);
                });
            },
        );
        second.textures_delta.clear();

        assert_eq!(
            clicked,
            Some(crate::request::Request::CheckForUpdates { manual: true }),
            "o botão pede um check manual"
        );
    }

    /// Junta o texto e a posição de todo `Shape::Text`, descendo nos grupos.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<(String, egui::Pos2)>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text) => {
                    out.push((text.galley.text().to_string(), text.pos));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }

        for clipped in shapes {
            walk(&clipped.shape, out);
        }
    }
}
