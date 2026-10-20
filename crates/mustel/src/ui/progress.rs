//! Indicador de progresso (adaptador sobre `indicatif`).
//!
//! Fora de terminal o indicador é **oculto** em vez de escrever lixo no stdout redirecionado —
//! o legado misturava progresso e resultado no mesmo stream, o que atrapalha pipe/CSV.
//!
//! Aqui moram duas peças: o [`Progress`] (spinner/barra, para fases como a descoberta de bancos) e a
//! [`LiveTable`], a **tabela ao vivo** do `query run` — a mesma `AnsiConsole.Live` do legado, que
//! redesenha a tabela a cada banco que termina e a **deixa na tela** no fim (`AutoClear(false)`).

use std::cell::RefCell;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use mustel_core::events::{CoreEvent, EventSink, Status};

use super::{is_interactive, table};

/// Indicador de progresso do CLI.
#[derive(Debug)]
pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    /// Cria um spinner com mensagem (oculto fora de terminal).
    pub fn spinner(message: &str) -> Self {
        let bar = if is_interactive() {
            let bar = ProgressBar::new_spinner();
            bar.set_style(
                ProgressStyle::with_template("{spinner} {msg}").expect("static template is valid"),
            );
            bar.enable_steady_tick(Duration::from_millis(120));
            bar
        } else {
            ProgressBar::hidden()
        };

        bar.set_message(message.to_string());
        Self { bar }
    }

    /// Cria uma barra com total conhecido (oculta fora de terminal).
    pub fn bar(total: u64) -> Self {
        let bar = if is_interactive() {
            ProgressBar::new(total)
        } else {
            ProgressBar::hidden()
        };
        Self { bar }
    }

    /// Atualiza a mensagem exibida.
    pub fn set_message(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    /// Avança o progresso.
    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    /// Encerra e limpa o indicador.
    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

/// Leva os eventos do núcleo para a tabela do terminal.
///
/// É o `TerminalSink` do T40 no pedaço que já existe: o núcleo emite [`CoreEvent`] e esta camada
/// decide o desenho. A GUI (T41) vai implementar a mesma trait com a tabela dela.
pub struct TableSink<'a> {
    table: &'a LiveTable,
}

impl<'a> TableSink<'a> {
    /// Escuta os eventos para a tabela dada.
    pub fn new(table: &'a LiveTable) -> Self {
        Self { table }
    }
}

impl EventSink for TableSink<'_> {
    fn emit(&self, event: CoreEvent<'_>) {
        match event {
            CoreEvent::DatabaseFinished {
                server,
                database,
                status,
                rows,
                duration,
                error,
            } => self
                .table
                .database_finished(server, database, status, rows, duration, error),
            CoreEvent::ServerWithoutDatabases { server } => {
                self.table.server_without_databases(server);
            }
        }
    }
}

/// Colunas da tabela de resultados do legado, na ordem.
const LIVE_TABLE_HEADERS: [&str; 6] =
    ["Status", "Server", "Database", "Rows", "Duration", "Detail"];

/// O traço do legado nas células que não têm valor (`[grey]—[/]`).
const LIVE_TABLE_DASH: &str = "—";

/// Tabela de resultados que se redesenha a cada banco que termina.
///
/// É o `AnsiConsole.Live(resultsTable).AutoClear(false)` do legado: cada banco que termina acrescenta
/// uma linha, o desenho é refeito no lugar e, no fim, a tabela **fica** na tela (o resumo e os
/// caminhos dos arquivos saem logo abaixo). Fora de terminal nada é escrito.
///
/// O desenho é N4 (o legado passa pelo codepage do console e desenha com bordas do Spectre); o que
/// interessa é o **conteúdo** das linhas, que é o que os testes fixam.
#[derive(Debug)]
pub struct LiveTable {
    bar: ProgressBar,
    /// Linhas já desenhadas, na ordem em que os bancos terminaram.
    rows: RefCell<Vec<Vec<String>>>,
}

impl Default for LiveTable {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveTable {
    /// Cria a tabela vazia (oculta fora de terminal).
    pub fn new() -> Self {
        let bar = if is_interactive() {
            let bar = ProgressBar::new_spinner();
            bar.set_style(ProgressStyle::with_template("{msg}").expect("static template is valid"));
            bar
        } else {
            ProgressBar::hidden()
        };

        Self {
            bar,
            rows: RefCell::new(Vec::new()),
        }
    }

    /// Linha de um banco que terminou: `✓`/`✗`, servidor, banco, linhas e a duração (ou o erro).
    ///
    /// O legado mostra a duração com uma casa (`0.3s`) no sucesso e traço no erro — e, no erro, a
    /// mensagem vai na coluna `Detail`.
    pub fn database_finished(
        &self,
        server: &str,
        database: &str,
        status: Status,
        rows: u64,
        duration: Duration,
        error: &str,
    ) {
        let row = match status {
            Status::Success => vec![
                "✓".to_string(),
                server.to_string(),
                database.to_string(),
                rows.to_string(),
                format!("{:.1}s", duration.as_secs_f64()),
                LIVE_TABLE_DASH.to_string(),
            ],
            Status::Error => vec![
                "✗".to_string(),
                server.to_string(),
                database.to_string(),
                LIVE_TABLE_DASH.to_string(),
                LIVE_TABLE_DASH.to_string(),
                error.to_string(),
            ],
        };

        self.push(row);
    }

    /// Linha do servidor que ficou sem banco nenhum (o `no databases` do legado).
    pub fn server_without_databases(&self, server: &str) {
        self.push(vec![
            LIVE_TABLE_DASH.to_string(),
            server.to_string(),
            LIVE_TABLE_DASH.to_string(),
            LIVE_TABLE_DASH.to_string(),
            LIVE_TABLE_DASH.to_string(),
            "no databases".to_string(),
        ]);
    }

    /// Deixa a tabela na tela, como o `AutoClear(false)` do legado.
    pub fn finish(&self) {
        self.bar.finish();
    }

    /// Acrescenta a linha e redesenha.
    fn push(&self, row: Vec<String>) {
        let mut rows = self.rows.borrow_mut();
        rows.push(row);

        if !self.bar.is_hidden() {
            let (width, height) = terminal_size();
            self.bar.set_message(draw(&rows, width, height));
        }
    }
}

/// Tamanho do terminal onde o desenho sai (`stderr`, o stream do `indicatif`).
///
/// Devolve `(largura, altura)`. O `console` devolve na ordem inversa — **(linhas, colunas)** —, o
/// que já custou uma depuração aqui: com a ordem trocada, uma janela de 8 linhas por 83 colunas vira
/// uma tabela de 8 colunas de largura.
fn terminal_size() -> (usize, usize) {
    let default = (24u16, 80u16);
    let (rows, columns) = console::Term::stderr().size_checked().unwrap_or(default);

    (
        if columns == 0 {
            80
        } else {
            usize::from(columns)
        },
        if rows == 0 { 24 } else { usize::from(rows) },
    )
}

/// A tabela pronta para desenhar: ajustada à largura e, se não couber na altura, cortada com `…`
/// (o `Overflow(VerticalOverflow.Ellipsis)` do legado).
///
/// A altura tem de ser respeitada para o redesenho no lugar se manter correto — o `indicatif` conta
/// as linhas da mensagem para apagar o desenho anterior.
pub(crate) fn draw(rows: &[Vec<String>], width: usize, height: usize) -> String {
    let table = table::render_fitted(&LIVE_TABLE_HEADERS, rows, width);
    let mut lines: Vec<&str> = table.lines().collect();

    if height > 1 && lines.len() > height {
        lines.truncate(height - 1);
        lines.push("…");
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_silent_outside_a_terminal() {
        let progress = Progress::spinner("trabalhando");
        progress.set_message("ainda");
        progress.inc(1);
        progress.finish();
        assert!(
            progress.bar.is_hidden(),
            "fora de terminal deve ficar oculto"
        );
    }

    #[test]
    fn the_live_table_is_silent_outside_a_terminal_and_keeps_the_rows() {
        let table = LiveTable::new();
        table.database_finished(
            "dev",
            "db",
            Status::Success,
            3,
            Duration::from_millis(320),
            "",
        );
        table.database_finished(
            "dev",
            "outro",
            Status::Error,
            0,
            Duration::ZERO,
            "Failed to connect to 127.0.0.1:1",
        );
        table.server_without_databases("vazio");
        table.finish();

        assert!(table.bar.is_hidden(), "fora de terminal não desenha");
        assert_eq!(
            *table.rows.borrow(),
            vec![
                vec!["✓", "dev", "db", "3", "0.3s", "—"],
                vec![
                    "✗",
                    "dev",
                    "outro",
                    "—",
                    "—",
                    "Failed to connect to 127.0.0.1:1"
                ],
                vec!["—", "vazio", "—", "—", "—", "no databases"],
            ],
            "as linhas da tabela do legado, na ordem em que os bancos terminam"
        );
    }

    #[test]
    fn the_drawn_table_fits_the_terminal_and_is_cropped_with_an_ellipsis() {
        let rows: Vec<Vec<String>> = (0..8)
            .map(|index| {
                vec![
                    "✓".to_string(),
                    "dev".to_string(),
                    format!("banco_{index}"),
                    "3".to_string(),
                    "0.3s".to_string(),
                    "—".to_string(),
                ]
            })
            .collect();

        let drawn = draw(&rows, 80, 24);
        assert_eq!(
            drawn.lines().count(),
            table::render_fitted(&LIVE_TABLE_HEADERS, &rows, 80)
                .lines()
                .count(),
            "com espaço de sobra, a tabela sai inteira"
        );

        let cropped = draw(&rows, 80, 6);
        assert_eq!(cropped.lines().count(), 6, "cabe na altura pedida");
        assert_eq!(cropped.lines().last(), Some("…"), "e o corte é marcado");
        assert!(cropped.contains("banco_0"), "o começo continua visível");
    }
}
