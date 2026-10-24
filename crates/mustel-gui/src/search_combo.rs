//! Combo com **campo de busca** — o mesmo widget para fontes, bases e queries salvas.
//!
//! Um `ComboBox` do `egui` não tem busca: com dezenas de fontes instaladas ou de bases num servidor,
//! achar o nome rolando a lista é o que dói. Aqui o botão mostra o valor atual (vazio quando nada
//! está escolhido) e abre um popup com o campo de busca e a lista filtrada.
//!
//! O estado do popup (aberto/fechado e texto da busca) vive na memória do `egui`, por `id`: assim
//! quem chama não precisa carregar mais campos no próprio estado, e dois combos na mesma tela não
//! compartilham busca.

/// Uma linha do combo.
pub struct Item<'a> {
    /// Texto mostrado (e usado como chave e como alvo da busca).
    pub text: &'a str,
    /// Detalhe só de exibição, à direita do texto (ex.: a contagem de bases do servidor).
    ///
    /// Fica **fora** da busca: procurar "104" não pode achar um servidor por causa do número.
    pub note: Option<&'a str>,
    /// É o valor atual?
    pub selected: bool,
}

impl<'a> Item<'a> {
    /// Um item sem detalhe.
    pub fn new(text: &'a str, selected: bool) -> Self {
        Self {
            text,
            note: None,
            selected,
        }
    }

    /// Um item com detalhe à direita (contagem, por exemplo).
    pub fn with_note(text: &'a str, note: &'a str, selected: bool) -> Self {
        Self {
            text,
            note: Some(note),
            selected,
        }
    }
}

#[derive(Default, Clone)]
struct State {
    open: bool,
    filter: String,
}

/// Desenha o combo e devolve o **índice** do item escolhido neste quadro, quando houver.
///
/// `button_text` é o que aparece no botão: o valor atual, ou vazio quando nada foi escolhido.
pub fn search_combo(
    ui: &mut egui::Ui,
    id: &str,
    button_text: &str,
    items: &[Item<'_>],
    enabled: bool,
) -> Option<usize> {
    let state_id = egui::Id::unique(("search_combo", id));
    let mut state = ui
        .data(|data| data.get_temp::<State>(state_id))
        .unwrap_or_default();
    let mut chosen = None;

    let button = ui
        .add_enabled(
            enabled,
            // Mesmo desenho dos combos de fonte: valor à esquerda, triângulo à direita.
            egui::Button::new(format!("{button_text}   ▼")).min_size(egui::vec2(200.0, 0.0)),
        )
        .on_hover_text(if enabled {
            "Choose…"
        } else {
            "Nothing to choose yet"
        });

    if button.clicked() {
        state.open = !state.open;
        // Abrir limpa a busca anterior: o dono quer ver a lista, não a última procura.
        state.filter.clear();
    }

    if state.open {
        // `IgnoreClicks` + fechamento decidido aqui: com `CloseOnClickOutside` o popup fecharia no
        // mesmo quadro em que abre (o clique do botão fica fora do popup, que ainda está em
        // *sizing pass*).
        let inner = egui::Popup::from_response(&button)
            .open(true)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .width(260.0)
            .show(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.filter)
                        .id_salt(id)
                        .hint_text("Search…")
                        .desired_width(f32::INFINITY),
                );

                let needle = state.filter.trim().to_lowercase();
                let mut any = false;

                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (index, item) in items.iter().enumerate() {
                            // Linha sem texto não é desenhada: o egui pintaria só o fundo azul da
                            // seleção, virando um quadrado sem significado. "Nada escolhido" mora
                            // no **botão** vazio, não numa linha em branco.
                            if item.text.trim().is_empty() {
                                continue;
                            }
                            if !needle.is_empty() && !item.text.to_lowercase().contains(&needle) {
                                continue;
                            }
                            any = true;
                            // O detalhe vai à direita, fraco: não é parte do nome nem da busca.
                            let response = ui.horizontal(|ui| {
                                let clicked =
                                    ui.selectable_label(item.selected, item.text).clicked();
                                if let Some(note) = item.note {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| ui.weak(note),
                                    );
                                }
                                clicked
                            });
                            if response.inner {
                                chosen = Some(index);
                                state.open = false;
                            }
                        }

                        if !any {
                            ui.weak("Nothing matches the search.");
                        }
                    });
            });

        // Fecha com `Esc` ou com um clique fora do popup — o clique que **abriu** não conta.
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let clicked_outside = !button.clicked()
            && ui.input(|input| input.pointer.any_pressed())
            && ui
                .input(|input| input.pointer.interact_pos())
                .is_some_and(|position| {
                    inner
                        .as_ref()
                        .is_some_and(|inner| !inner.response.rect.contains(position))
                        && !button.rect.contains(position)
                });

        if escape || clicked_outside {
            state.open = false;
        }
    }

    ui.data_mut(|data| data.insert_temp(state_id, state));
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Textos pintados no quadro, para conferir o que a tela desenhou sem abrir janela.
    fn collect_text(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => {
                    let content = text.galley.text().trim().to_string();
                    if !content.is_empty() {
                        out.push(content);
                    }
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

    fn items<'a>(texts: &'a [&'a str], selected: Option<&str>) -> Vec<Item<'a>> {
        texts
            .iter()
            .map(|text| Item::new(text, Some(*text) == selected))
            .collect()
    }

    /// Sem escolha o botão fica **vazio** (não um `-`), e o nome aparece depois de escolher.
    /// CONTRACT: IF-232
    #[test]
    fn the_button_is_blank_until_something_is_picked() {
        let ctx = egui::Context::default();
        let options = ["configurador", "dev"];

        let painted = |button_text: &str| {
            let items = items(&options, None);
            let mut texts: Vec<String> = Vec::new();
            // Dois quadros: no primeiro o `Area` do popup roda em *sizing pass*.
            for _ in 0..2 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let _ = search_combo(ui, "test_combo", button_text, &items, true);
                    });
                });
                texts.clear();
                collect_text(&output.shapes, &mut texts);
                output.textures_delta.clear();
            }
            texts
        };

        let texts = painted("");
        assert!(
            !texts.iter().any(|text| text.trim() == "-"),
            "sem escolha não aparece `-`: {texts:?}"
        );

        let texts = painted("dev");
        assert!(
            texts.iter().any(|text| text.contains("dev")),
            "com escolha aparece o nome: {texts:?}"
        );
    }

    /// O popup tem busca, filtra sem diferenciar maiúsculas e **não** oferece o texto digitado.
    /// CONTRACT: IF-232
    #[test]
    fn the_popup_searches_and_no_longer_offers_the_typed_text() {
        let ctx = egui::Context::default();
        let options = ["configurador", "dev", "test2"];
        let state_id = egui::Id::unique(("search_combo", "test_combo"));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                // Abre e digita `CONF` direto na memória do widget, como a janela faria.
                ctx.data_mut(|data| {
                    data.insert_temp(
                        state_id,
                        State {
                            open: true,
                            filter: "CONF".to_string(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items = items(&options, None);
                    let _ = search_combo(ui, "test_combo", "", &items, true);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(
            texts.iter().any(|text| text == "CONF"),
            "o campo de busca mostra o texto digitado: {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "configurador"),
            "a que casa aparece: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text == "dev"),
            "a que não casa fica fora: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.contains("Use '")),
            "o texto digitado não vira opção: {texts:?}"
        );
    }

    /// Sem filtro o popup mostra a dica de busca (o campo existe).
    /// CONTRACT: IF-232
    #[test]
    fn an_empty_filter_shows_the_search_hint() {
        let ctx = egui::Context::default();
        let options = ["configurador", "dev"];
        let state_id = egui::Id::unique(("search_combo", "test_combo"));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                ctx.data_mut(|data| {
                    data.insert_temp(
                        state_id,
                        State {
                            open: true,
                            filter: String::new(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items = items(&options, None);
                    let _ = search_combo(ui, "test_combo", "", &items, true);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(
            texts.iter().any(|text| text == "Search…"),
            "mostra a dica do campo de busca: {texts:?}"
        );
        for option in options {
            assert!(
                texts.iter().any(|text| text == option),
                "lista {option}: {texts:?}"
            );
        }
    }

    /// O detalhe (`note`) aparece na linha mas **não** entra na busca nem vira valor.
    ///
    /// É o que permite mostrar `Local (104)` sem que procurar "104" ache o servidor — e sem
    /// contaminar o nome usado como filtro.
    /// CONTRACT: IF-236
    #[test]
    fn the_note_is_shown_but_not_searchable() {
        let ctx = egui::Context::default();
        let options = ["Local", "Local 2"];
        let notes = ["104", "0"];
        let state_id = egui::Id::unique(("search_combo", "test_combo"));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                ctx.data_mut(|data| {
                    data.insert_temp(
                        state_id,
                        State {
                            open: true,
                            filter: "104".to_string(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items: Vec<Item<'_>> = options
                        .iter()
                        .zip(notes)
                        .map(|(text, note)| Item::with_note(text, note, false))
                        .collect();
                    let _ = search_combo(ui, "test_combo", "", &items, true);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(
            !texts.iter().any(|text| text == "Local"),
            "o numero nao pode ser alvo de busca: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text == "Nothing matches the search."),
            "busca por numero nao acha nada: {texts:?}"
        );
    }

    /// Sem filtro, o detalhe é desenhado ao lado do nome.
    /// CONTRACT: IF-236
    #[test]
    fn the_note_is_drawn_next_to_the_name() {
        let ctx = egui::Context::default();
        let state_id = egui::Id::unique(("search_combo", "test_combo"));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                ctx.data_mut(|data| {
                    data.insert_temp(
                        state_id,
                        State {
                            open: true,
                            filter: String::new(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items = [
                        Item::with_note("Local", "104", false),
                        Item::with_note("Local 2", "0", false),
                    ];
                    let _ = search_combo(ui, "test_combo", "", &items, true);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        for expected in ["Local", "104", "Local 2", "0"] {
            assert!(
                texts.iter().any(|text| text == expected),
                "{expected} aparece: {texts:?}"
            );
        }
    }

    /// Linha sem texto não é desenhada (era o quadrado azul da seleção sem rótulo).
    /// CONTRACT: IF-232
    #[test]
    fn an_item_without_text_is_not_drawn() {
        let ctx = egui::Context::default();
        let options = ["", "dev"];
        let state_id = egui::Id::unique(("search_combo", "test_combo"));

        let mut texts: Vec<String> = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                ctx.data_mut(|data| {
                    data.insert_temp(
                        state_id,
                        State {
                            open: true,
                            filter: String::new(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items = items(&options, Some(""));
                    let _ = search_combo(ui, "test_combo", "", &items, true);
                });
            });
            texts.clear();
            collect_text(&output.shapes, &mut texts);
            output.textures_delta.clear();
        }

        assert!(
            texts.iter().any(|text| text == "dev"),
            "as demais linhas continuam: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.trim().is_empty()),
            "nenhuma linha vazia é pintada: {texts:?}"
        );
    }

    /// A busca é **por combo**: dois na mesma tela não compartilham o filtro.
    #[test]
    fn two_combos_do_not_share_the_search() {
        let ctx = egui::Context::default();
        let options = ["dev"];

        let mut seen = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                ctx.data_mut(|data| {
                    data.insert_temp(
                        egui::Id::unique(("search_combo", "combo_a")),
                        State {
                            open: true,
                            filter: "zzz".to_string(),
                        },
                    )
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let items = items(&options, None);
                    let _ = search_combo(ui, "combo_a", "", &items, true);
                    let _ = search_combo(ui, "combo_b", "", &items, true);
                    seen.push(());
                });
            });
            output.textures_delta.clear();
        }

        // `combo_a` filtra tudo para fora; `combo_b` (fechado) não desenha busca nenhuma.
        assert_eq!(seen.len(), 2);
    }
}
