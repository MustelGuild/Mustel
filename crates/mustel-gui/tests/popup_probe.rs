//! Teste headless do popup com a ligação **de produção** (`mustel_gui::sql_completion`).
//!
//! Digita-se como o winit entrega (um `Event::Text` por caractere) e observa-se o que o editor faz:
//! qual é a palavra, quando a lista abre e o que o `Tab` insere. Foco e cursor são pedidos à memória
//! do egui — por isso isto não substitui um teste com teclado de verdade, mas é o que prova as
//! decisões sem abrir janela.

use egui::{Context, Event, Key, Modifiers, RawInput, TextEdit};
use mustel_core::usecases::schema::{Column, Function, RelationKind, Schema, Table};
use mustel_gui::sql_completion;

fn schema() -> Schema {
    Schema {
        tables: vec![
            Table {
                schema: "public".to_string(),
                name: "bench_bkp".to_string(),
                kind: RelationKind::Table,
                columns: vec![Column {
                    name: "nome".to_string(),
                    type_name: "text".to_string(),
                }],
            },
            Table {
                schema: "public".to_string(),
                name: "clientes".to_string(),
                kind: RelationKind::Table,
                columns: vec![Column {
                    name: "id".to_string(),
                    type_name: "integer".to_string(),
                }],
            },
        ],
        functions: vec![Function {
            schema: "public".to_string(),
            name: "substr".to_string(),
            arguments: Some("text, integer".to_string()),
        }],
    }
}

/// O que o popup viu e mostrou num quadro.
#[derive(Debug, Default, Clone)]
struct Seen {
    word: String,
    range: (usize, usize),
    suggestions: usize,
    open: bool,
    accepted: bool,
}

/// O editor da tela de consulta, sem janela.
struct Editor {
    ctx: Context,
    text: String,
    schema: Schema,
    trigger: sql_completion::Trigger,
    seen: Seen,
}

impl Editor {
    fn new() -> Self {
        let ctx = Context::default();
        // Um quadro antes: o id do editor precisa existir para receber o foco.
        let mut output = ctx.run_ui(RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx.memory_mut(|memory| memory.request_focus(sql_completion::id()));

        Self {
            ctx,
            text: String::new(),
            schema: schema(),
            trigger: sql_completion::Trigger::default(),
            seen: Seen::default(),
        }
    }

    /// Um quadro com os eventos dados — o mesmo widget, o mesmo gatilho e o mesmo motor do app.
    fn frame(&mut self, events: Vec<Event>) {
        let mut seen = Seen::default();
        let schema = &self.schema;
        let text = &mut self.text;
        let trigger = &mut self.trigger;

        let mut input = RawInput {
            events,
            ..Default::default()
        };
        let mut output = self.ctx.run_ui(input.take(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let force = ui.input(sql_completion::force_requested);
                let edit = TextEdit::multiline(text).desired_width(f32::INFINITY);
                let out = egui::CompletionPopup::new(sql_completion::id())
                    .word_boundary(sql_completion::word_boundary)
                    .accept_keys([Key::Tab, Key::Enter])
                    .show(ui, edit, |query| {
                        seen.word = query.word.to_string();
                        seen.range = (query.word_range.start.0, query.word_range.end.0);
                        let suggestions =
                            sql_completion::candidates(trigger, query, Some(schema), force);
                        seen.suggestions = suggestions.len();
                        suggestions
                    });
                trigger.record_accepted(out.accepted.is_some());
                seen.accepted = out.accepted.is_some();
                seen.open = out.is_open;
            });
        });
        output.textures_delta.clear();
        self.seen = seen;
    }

    /// Digita como o winit: um evento de texto por caractere, um quadro por evento.
    fn type_text(&mut self, typed: &str) {
        for character in typed.chars() {
            self.frame(vec![Event::Text(character.to_string())]);
        }
    }

    /// Pressiona uma tecla sem modificadores.
    fn press(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
    }
}

/// A palavra é o identificador sob o cursor — o inverso (a palavra ser o **espaço**) deixava o motor
/// e o popup em desacordo: nada abria ao digitar e o texto aceito grudava no anterior.
#[test]
fn the_word_is_the_identifier_under_the_cursor() {
    let mut editor = Editor::new();
    editor.type_text("select * from bench_bkp");

    assert_eq!(
        editor.seen.word, "bench_bkp",
        "a palavra é a tabela digitada, não o espaço que veio antes: {:?}",
        editor.seen
    );
    assert_eq!(editor.seen.range, (14, 23), "o trecho é a tabela inteira");
}

#[test]
fn the_table_list_opens_right_after_from() {
    let mut editor = Editor::new();
    editor.type_text("select name from ");

    assert_eq!(
        editor.seen.word, "",
        "o cursor está no começo de um token novo"
    );
    assert!(
        editor.seen.suggestions > 0,
        "o motor tem tabelas para oferecer: {:?}",
        editor.seen
    );
    assert!(
        editor.seen.open,
        "a lista aparece sem digitar nada: {:?}",
        editor.seen
    );

    // E aceitar o primeiro candidato não come o espaço do `from` (era `selectnome`).
    editor.press(Key::Tab);
    assert!(editor.seen.accepted, "o `Tab` aceitou uma sugestão");
    assert_eq!(
        editor.text, "select name from bench_bkp",
        "o texto aceito entra depois do espaço"
    );

    // E a lista não fica reaberta com o nome recém-inserido na frente.
    editor.frame(vec![]);
    assert!(
        !editor.seen.open,
        "depois do aceite a lista fica fechada: {:?}",
        editor.seen
    );
}

/// `Ctrl+Space` abre mesmo no começo do texto, onde o gatilho automático não abre.
///
/// CONTRACT: IF-210
#[test]
fn ctrl_space_opens_the_list_without_any_typing() {
    let mut editor = Editor::new();
    editor.frame(vec![]);
    assert!(
        !editor.seen.open,
        "editor vazio e sem contexto não abre sozinho"
    );

    // Como o winit manda: o Ctrl chega como `ModifiersChanged` e o evento de tecla carrega os
    // modificadores do instante da digitação (no Windows, `ctrl` — `command` só no Mac).
    editor.frame(vec![Event::ModifiersChanged(Modifiers::CTRL)]);
    editor.frame(vec![Event::Key {
        key: Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    }]);

    assert!(
        editor.seen.suggestions > 0,
        "Ctrl+Space pede a lista ao motor: {:?}",
        editor.seen
    );
    assert!(editor.seen.open, "e ela aparece: {:?}", editor.seen);

    // O ponto que só o uso real revelou: o atalho vale no quadro da tecla, e nos seguintes a lista
    // precisa **continuar** aberta (senão abre e fecha em ~16 ms, que é o mesmo que não abrir).
    for frame in 0..3 {
        editor.frame(vec![]);
        assert!(
            editor.seen.open,
            "quadro {frame} depois do atalho: a lista continua aberta ({:?})",
            editor.seen
        );
    }
}

/// Dentro de string ou comentário não se sugere nada (regra do núcleo, conferida na ligação).
#[test]
fn nothing_opens_inside_a_literal_or_a_comment() {
    let mut editor = Editor::new();
    editor.type_text("select 'bench");
    assert!(
        editor.seen.suggestions == 0 && !editor.seen.open,
        "string aberta não sugere: {:?}",
        editor.seen
    );

    let mut editor = Editor::new();
    editor.type_text("select 1 -- bench");
    assert!(
        editor.seen.suggestions == 0 && !editor.seen.open,
        "comentário não sugere: {:?}",
        editor.seen
    );
}
