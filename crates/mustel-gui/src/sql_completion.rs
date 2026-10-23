//! Ligação entre o editor de SQL e o motor de sugestão do núcleo.
//!
//! O `CompletionPopup` do egui faz três perguntas ao aplicativo, e as três ficam aqui — fora da tela
//! e testáveis sem janela:
//!
//! - **onde começa a palavra** ([`word_boundary`]): o popup pergunta se um caractere *separa* palavras,
//!   o inverso do que o motor pergunta. A regra vem do núcleo ([`completion::is_word_boundary`]), para
//!   que o trecho substituído seja exatamente o que o motor leu;
//! - **quando abrir** ([`wants_suggestions`] e [`Trigger`]): começo de um token (`FROM |`), qualquer
//!   letra digitada ou `Ctrl+Space`;
//! - **como pedir sob demanda** ([`force_requested`]): lê os modificadores do próprio evento de tecla.
//!
//! [`Trigger`] existe porque a pergunta é feita **a cada quadro**: o atalho só acontece no quadro da
//! tecla e o aceite só no quadro do `Tab`. Sem memória entre quadros, o `Ctrl+Space` abria e fechava
//! no quadro seguinte, e o texto aceito reabria a lista com ele mesmo na frente.

use egui::{CompletionQuery, Event, InputState, Key, Suggestion};
use mustel_core::completion;
use mustel_core::usecases::schema::Schema;

/// Id do editor de SQL.
///
/// O `TextEdit` e o popup compartilham este id: é nele que o egui guarda o cursor, o foco e o estado
/// do popup. Precisa ser **estável entre quadros** — [`egui::Id::unique`] é, porque usa sementes
/// fixas; um id novo a cada quadro perderia o foco e o cursor.
pub fn id() -> egui::Id {
    egui::Id::unique("sql_completion")
}

/// Fronteira de palavra do popup (a regra é a do núcleo, ver [`completion::is_word_boundary`]).
pub fn word_boundary(character: char) -> bool {
    completion::is_word_boundary(character)
}

/// `Ctrl+Space` foi pedido neste quadro?
///
/// Lê os modificadores **do próprio evento** de tecla, não de [`InputState::modifiers`]: o evento
/// carrega o estado do instante da digitação, que é o que o backend (winit) preenche — `command` no
/// Mac, `command` = `ctrl` nas outras plataformas. O estado do quadro só fica correto se um
/// `ModifiersChanged` tiver chegado antes, e é isso que fazia o atalho morrer em silêncio.
pub fn force_requested(input: &InputState) -> bool {
    input.events.iter().any(|event| {
        matches!(
            event,
            Event::Key {
                key: Key::Space,
                pressed: true,
                modifiers,
                ..
            } if modifiers.command || modifiers.ctrl
        )
    })
}

/// O popup deve consultar o motor neste quadro?
///
/// Abre no **começo de um token** (palavra vazia depois de um separador, como em `FROM |`) e a partir
/// da **primeira letra** digitada. Exigir duas letras escondia a lista exatamente onde ela é mais
/// útil: escolher a tabela do `FROM` sem digitar o nome inteiro.
///
/// A exceção é o começo do texto com o editor vazio — ali não há contexto nenhum, e o popup cobriria
/// a caixa à toa; nesse caso só `Ctrl+Space` abre.
/// CONTRACT: IF-210
pub fn wants_suggestions(query: &CompletionQuery<'_>, force: bool) -> bool {
    force || !(query.word.is_empty() && query.is_at_start())
}

/// Candidatos do motor, prontos para o popup.
///
/// O cursor que o motor recebe é o **fim da palavra** que o popup identificou: assim o trecho que o
/// popup vai substituir é o mesmo que o motor usou para filtrar.
pub fn suggestions(query: &CompletionQuery<'_>, schema: &Schema) -> Vec<Suggestion> {
    completion::suggest(query.text, query.word_range.end.0, schema)
        .into_iter()
        .map(|candidate| match candidate.detail {
            Some(detail) => Suggestion::new(candidate.insert).description(detail),
            None => Suggestion::new(candidate.insert),
        })
        .collect()
}

/// O que o popup mostra: os candidatos do motor, ou nada quando não é hora de sugerir.
///
/// Chamado uma vez por quadro. Sem esquema carregado não há o que sugerir, mas o [`Trigger`] é
/// atualizado do mesmo jeito — a memória do atalho não pode depender de o banco estar carregado.
pub fn candidates(
    trigger: &mut Trigger,
    query: &CompletionQuery<'_>,
    schema: Option<&Schema>,
    force: bool,
) -> Vec<Suggestion> {
    let wanted = trigger.wants(query, force);
    let Some(schema) = schema else {
        return Vec::new();
    };

    if wanted {
        suggestions(query, schema)
    } else {
        Vec::new()
    }
}

/// O que sobrevive entre quadros para o popup não piscar.
///
/// O egui decide abrir a cada quadro pelos candidatos que recebe, e as duas coisas que precisam
/// durar mais de um quadro são o pedido do `Ctrl+Space` e a lista recém-aceita:
///
/// - `Ctrl+Space` vale enquanto o cursor não sair de onde foi pressionado (senão o popup abria e
///   fechava em ~16 ms — foi o que o relato chamou de "não funciona");
/// - depois de aceitar, a lista não reabre com a palavra recém-inserida (ela seria o único item,
///   já selecionado, na frente de quem quer continuar digitando).
#[derive(Default)]
pub struct Trigger {
    /// Onde o popup foi pedido ou onde um texto foi aceito, e por quê.
    anchor: Option<(WordRange, Mode)>,
    /// Uma sugestão foi aceita no quadro anterior.
    just_accepted: bool,
}

/// Posição da palavra sob o cursor, como `(início, fim)` em caracteres.
type WordRange = (usize, usize);

/// Por que o popup está ancorado numa posição.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// `Ctrl+Space` pediu a lista aqui: continua pedindo.
    Forced,
    /// Uma sugestão foi aceita aqui: não reabre.
    Accepted,
}

impl Trigger {
    /// O popup deve consultar o motor neste quadro? (atualiza a memória do atalho e do aceite)
    pub fn wants(&mut self, query: &CompletionQuery<'_>, force: bool) -> bool {
        let at = (query.word_range.start.0, query.word_range.end.0);

        // O aceite vale para o quadro **seguinte**, quando a palavra já é a inserida.
        if std::mem::take(&mut self.just_accepted) {
            self.anchor = Some((at, Mode::Accepted));
            return false;
        }

        if force {
            self.anchor = Some((at, Mode::Forced));
            return true;
        }

        match self.anchor {
            Some((anchor, Mode::Forced)) if anchor == at => return true,
            Some((anchor, Mode::Accepted)) if anchor == at => return false,
            _ => self.anchor = None,
        }

        wants_suggestions(query, false)
    }

    /// Registra o que o popup devolveu: um texto aceito fecha a lista até o próximo movimento.
    pub fn record_accepted(&mut self, accepted: bool) {
        if accepted {
            self.just_accepted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::text::CharIndex;

    /// Uma `CompletionQuery` com a palavra começando em `start`.
    fn query<'a>(text: &'a str, start: usize, end: usize) -> CompletionQuery<'a> {
        CompletionQuery {
            text,
            word: &text[start..end],
            word_range: CharIndex(start)..CharIndex(end),
        }
    }

    /// O popup abre no começo do token e a partir da primeira letra.
    ///
    /// CONTRACT: IF-210
    #[test]
    fn it_opens_at_the_start_of_a_token_and_on_the_first_letter() {
        // `SELECT * FROM ` — o cursor está no começo de uma tabela: é onde a lista mais serve.
        let sql = "SELECT * FROM ";
        assert!(wants_suggestions(&query(sql, 14, 14), false), "FROM |");

        // Uma letra já basta: duas escondiam a tabela enquanto o nome é digitado.
        let sql = "SELECT * FROM t";
        assert!(wants_suggestions(&query(sql, 14, 15), false), "FROM t");

        // No meio de uma palavra também (o cursor não está no começo do texto).
        let sql = "SELECT * FROM test";
        assert!(wants_suggestions(&query(sql, 14, 18), false), "FROM test");
    }

    /// No começo do texto vazio, só `Ctrl+Space` abre.
    ///
    /// CONTRACT: IF-210
    #[test]
    fn it_only_opens_at_the_start_of_the_text_on_demand() {
        assert!(!wants_suggestions(&query("", 0, 0), false), "editor vazio");
        assert!(wants_suggestions(&query("", 0, 0), true), "Ctrl+Space");
    }

    /// `Ctrl+Space` mantém a lista aberta nos quadros seguintes.
    ///
    /// É o caso que o teste de um quadro só deixou passar: o atalho vale no quadro da tecla, e no
    /// seguinte o popup se fechava sozinho (abria e sumia em ~16 ms — o "não funciona" do relato).
    ///
    /// CONTRACT: IF-210
    #[test]
    fn ctrl_space_keeps_the_list_open_while_the_cursor_does_not_move() {
        let mut trigger = Trigger::default();
        let sql = "";

        assert!(
            trigger.wants(&query(sql, 0, 0), true),
            "o quadro da tecla abre"
        );
        for _ in 0..3 {
            assert!(
                trigger.wants(&query(sql, 0, 0), false),
                "e os seguintes continuam abertos"
            );
        }

        // Digitar muda a palavra e o pedido expira — mas aí a regra normal já abre.
        assert!(trigger.wants(&query("s", 0, 1), false), "uma letra abre");
    }

    /// O `Ctrl+Space` vale só onde foi pressionado: mover o cursor expira o pedido.
    ///
    /// CONTRACT: IF-210
    #[test]
    fn the_forced_request_expires_when_the_cursor_moves() {
        let mut trigger = Trigger::default();
        assert!(trigger.wants(&query("SELECT 1", 8, 8), true));
        assert!(trigger.wants(&query("SELECT 1", 8, 8), false));
        assert!(
            !trigger.wants(&query("SELECT 1", 0, 0), false),
            "no começo do texto o pedido antigo não vale mais"
        );
    }

    /// Depois de aceitar, a lista não reabre com a palavra recém-inserida.
    #[test]
    fn an_accepted_suggestion_does_not_reopen_the_list() {
        let mut trigger = Trigger::default();

        // Quadro do aceite: a lista estava aberta com a palavra vazia depois do `FROM `.
        assert!(trigger.wants(&query("SELECT * FROM ", 14, 14), false));
        trigger.record_accepted(true);

        // Quadro seguinte: a palavra já é a sugerida, e a lista fica fechada.
        let sql = "SELECT * FROM bench_bkp";
        assert!(!trigger.wants(&query(sql, 14, 23), false), "não reabre");
        assert!(!trigger.wants(&query(sql, 14, 23), false), "nem depois");

        // Digitar de novo abre normalmente.
        let sql = "SELECT * FROM bench_bkp ";
        assert!(
            trigger.wants(&query(sql, 24, 24), false),
            "espaço depois da tabela"
        );
    }

    /// `Ctrl+Space` depois de um aceite reabre na mesma posição.
    #[test]
    fn a_new_ctrl_space_reopens_after_an_accept() {
        let mut trigger = Trigger::default();
        let sql = "SELECT * FROM bench_bkp";
        trigger.wants(&query(sql, 14, 14), false);
        trigger.record_accepted(true);
        assert!(!trigger.wants(&query(sql, 14, 23), false));
        assert!(
            trigger.wants(&query(sql, 14, 23), true),
            "o atalho vence a pausa do aceite"
        );
    }

    /// Sem esquema carregado não há sugestão — e o gatilho não fica com estado pendurado.
    #[test]
    fn without_a_schema_the_trigger_is_still_updated() {
        let mut trigger = Trigger::default();
        let sql = "SELECT * FROM ";

        assert!(
            candidates(&mut trigger, &query(sql, 14, 14), None, false).is_empty(),
            "sem esquema, nada a sugerir"
        );

        // O `Ctrl+Space` pressionado nesse instante continua valendo quando o esquema chega.
        assert!(candidates(&mut trigger, &query(sql, 14, 14), None, true).is_empty());
        let schema = mustel_core::usecases::schema::Schema::default();
        let _ = candidates(&mut trigger, &query(sql, 14, 14), Some(&schema), false);
        assert!(
            trigger.wants(&query(sql, 14, 14), false),
            "o pedido do atalho sobrevive à falta do esquema"
        );
    }
}
