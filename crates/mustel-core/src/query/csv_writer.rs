//! Escrita do CSV do `query run`, byte a byte como o legado.
//!
//! O legado usa o `CsvHelper` (cultura invariante). O que foi **medido** no T32 rodando o oráculo e
//! olhando os bytes:
//!
//! - arquivo começa com **BOM UTF-8** (`EF BB BF`) — vem do `Encoding.UTF8` do `StreamWriter`, que
//!   emite o BOM;
//! - quebra de linha `\r\n`;
//! - delimitador `,`; campo só é citado quando contém `,`, `"`, `\r` ou `\n`, e `"` interno vira
//!   `""`;
//! - `\n` **dentro** de um campo é preservado como veio (não vira `\r\n`);
//! - a primeira linha é `Server,Database,` + colunas da consulta, na ordem em que apareceram.
//!
//! Nomes de arquivo (medidos): dentro de `<saída>/<timestamp>/` ficam `<servidor>_<timestamp>.csv`
//! (sanitizado), `<timestamp>_erros.csv` (só quando há erro), `<timestamp>_log.csv` e
//! `consolidated_<timestamp>.csv`.

use std::fmt::Write as _;
use std::path::Path;

/// BOM UTF-8, como o `StreamWriter(path, append, Encoding.UTF8)` do .NET escreve.
const UTF8_BOM: &str = "\u{feff}";

/// Quebra de linha do `CsvHelper` (padrão: `\r\n`, independente do SO).
const NEWLINE: &str = "\r\n";

/// Escreve um campo como o `CsvHelper` escreve: cita só quando precisa.
pub fn write_field(out: &mut String, field: &str) {
    if needs_quoting(field) {
        out.push('"');
        for character in field.chars() {
            if character == '"' {
                out.push('"');
            }
            out.push(character);
        }
        out.push('"');
    } else {
        out.push_str(field);
    }
}

/// O campo precisa de aspas? (regra do `CsvHelper`: delimitador, aspas ou quebra de linha.)
fn needs_quoting(field: &str) -> bool {
    field.contains([',', '"', '\r', '\n'])
}

/// Escreve uma linha de campos e a quebra do `CsvHelper`.
pub fn write_record(out: &mut String, fields: &[String]) {
    let mut first = true;
    for field in fields {
        if !first {
            out.push(',');
        }
        first = false;
        write_field(out, field);
    }
    out.push_str(NEWLINE);
}

/// Conteúdo completo de um CSV com o cabeçalho e as linhas dadas (BOM incluído).
pub fn render_csv(
    header: &[String],
    rows: &[Vec<String>],
) -> String {
    // O BOM vem antes de tudo — inclusive do cabeçalho.
    let mut out = String::from(UTF8_BOM);
    write_record(&mut out, header);
    for row in rows {
        write_record(&mut out, row);
    }

    out
}

/// Nome do arquivo por servidor: `<servidor sanitizado>_<timestamp>.csv`.
pub fn server_file_name(server: &str, timestamp: &str) -> String {
    format!("{}_{timestamp}.csv", sanitize_filename(server))
}

/// Nome do arquivo consolidado: `consolidated_<timestamp>.csv`.
pub fn consolidated_file_name(timestamp: &str) -> String {
    format!("consolidated_{timestamp}.csv")
}

/// Nome do arquivo de erros: `<timestamp>_erros.csv`.
pub fn errors_file_name(timestamp: &str) -> String {
    format!("{timestamp}_erros.csv")
}

/// Nome do arquivo de log: `<timestamp>_log.csv`.
pub fn log_file_name(timestamp: &str) -> String {
    format!("{timestamp}_log.csv")
}

/// Caracteres que o legado troca por `_` no nome do arquivo (`Path.GetInvalidFileNameChars` no
/// Windows, com a lista dele).
const WINDOWS_INVALID_FILE_NAME_CHARS: [char; 10] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

/// Troca os caracteres inválidos de nome de arquivo por `_`, como o `SanitizeFilename` do legado.
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|character| {
            if WINDOWS_INVALID_FILE_NAME_CHARS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect()
}

/// Timestamp do diretório de execução: `yyyy-MM-dd_HHmmss` em **UTC**, como o legado.
pub fn timestamp_for(now: std::time::SystemTime) -> String {
    let micros = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_micros() as i64)
        .unwrap_or_default();
    let (year, month, day, hour, minute, second) = civil_parts(micros);

    format!("{year:04}-{month:02}-{day:02}_{hour:02}{minute:02}{second:02}")
}

/// Timestamp do log: formato `"O"` do .NET (ISO 8601 com 7 casas de fração e `Z`).
pub fn iso8601_for(now: std::time::SystemTime) -> String {
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let micros = duration.as_micros() as i64;
    let (year, month, day, hour, minute, second) = civil_parts(micros);
    // O .NET escreve 7 casas (ticks de 100 ns); os microssegundos entram com um zero à direita.
    let fraction = duration.subsec_micros();

    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{fraction:06}0Z"
    )
}

/// Data/hora civil (UTC) a partir de microssegundos desde 1970-01-01.
fn civil_parts(micros_since_unix: i64) -> (i64, u32, u32, i64, i64, i64) {
    let days = micros_since_unix.div_euclid(86_400_000_000);
    let micros_of_day = micros_since_unix.rem_euclid(86_400_000_000);
    let (year, month, day) = crate::postgres::value::civil_from_days(days);

    let seconds_of_day = micros_of_day / 1_000_000;

    (
        year,
        month,
        day,
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60,
    )
}

/// Escreve o arquivo (BOM + conteúdo) e devolve o caminho.
pub fn write_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, content)
}

/// Monta a linha de dados de um resultado (usada no per-server e no consolado).
pub fn data_row(
    server: &str,
    database: &str,
    columns: &[String],
    values: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    let mut fields = Vec::with_capacity(columns.len() + 2);
    fields.push(server.to_string());
    fields.push(database.to_string());
    for column in columns {
        let mut field = String::new();
        let _ = write!(field, "{}", values.get(column).cloned().unwrap_or_default());
        fields.push(field);
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn header_of(columns: &[&str]) -> Vec<String> {
        let mut header = vec!["Server".to_string(), "Database".to_string()];
        header.extend(columns.iter().map(|column| (*column).to_string()));
        header
    }

    #[test]
    fn a_csv_starts_with_the_bom_and_uses_crlf() {
        let csv = render_csv(&header_of(&["id"]), &[vec!["s".into(), "d".into(), "1".into()]]);

        assert!(csv.starts_with('\u{feff}'), "o BOM vem primeiro");
        assert_eq!(csv.as_bytes()[..3], [0xEF, 0xBB, 0xBF]);
        assert!(csv.ends_with("\r\n"), "CRLF no fim da última linha");
        assert!(!csv.replace("\r\n", "").contains('\n'), "nenhum LF solto");
    }

    #[test]
    fn fields_are_quoted_only_when_they_need_to_be() {
        let mut out = String::new();
        write_record(
            &mut out,
            &[
                "simples".to_string(),
                "com, vírgula".to_string(),
                "com \"aspas\"".to_string(),
                "com\nquebra".to_string(),
                String::new(),
            ],
        );

        // É exatamente o que o oráculo escreveu para esses valores.
        assert_eq!(
            out,
            "simples,\"com, vírgula\",\"com \"\"aspas\"\"\",\"com\nquebra\",\r\n"
        );
    }

    #[test]
    fn a_newline_inside_a_field_is_preserved() {
        let mut out = String::new();
        write_record(&mut out, &["linha1\nlinha2".to_string()]);

        // O campo vai citado e a quebra interna continua sendo `\n` (não vira `\r\n`).
        assert_eq!(out, "\"linha1\nlinha2\"\r\n");
    }

    #[test]
    fn file_names_follow_the_legacy() {
        assert_eq!(server_file_name("127.0.0.1", "2026-10-18_143228"), "127.0.0.1_2026-10-18_143228.csv");
        assert_eq!(consolidated_file_name("ts"), "consolidated_ts.csv");
        assert_eq!(errors_file_name("ts"), "ts_erros.csv");
        assert_eq!(log_file_name("ts"), "ts_log.csv");
    }

    #[test]
    fn server_names_are_sanitized_like_the_legacy() {
        assert_eq!(sanitize_filename("meu/servidor:5432"), "meu_servidor_5432");
        assert_eq!(sanitize_filename("dev.local"), "dev.local");
        assert_eq!(sanitize_filename("a<b>c|d*e?f\"g\\h"), "a_b_c_d_e_f_g_h");
    }

    #[test]
    fn the_execution_timestamp_matches_the_legacy_pattern() {
        // 2026-10-18T14:32:28Z em microssegundos desde a época Unix.
        let unix_micros: i64 = 1_792_326_748 * 1_000_000;
        let stamp = timestamp_for(std::time::UNIX_EPOCH + std::time::Duration::from_micros(unix_micros as u64));

        assert_eq!(stamp, "2026-10-18_143228");
    }

    #[test]
    fn the_log_timestamp_uses_the_dotnet_round_trip_format() {
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_micros(1_792_326_748_228_411);
        // `ExecutedAt` do oráculo: `2026-10-18T14:32:28.2284119Z` (7 casas).
        assert_eq!(iso8601_for(now), "2026-10-18T14:32:28.2284110Z");
    }

    #[test]
    fn a_data_row_fills_missing_columns_with_empty_text() {
        let columns = vec!["a".to_string(), "b".to_string()];
        let mut values = HashMap::new();
        values.insert("a".to_string(), "1".to_string());

        assert_eq!(
            data_row("s", "d", &columns, &values),
            vec!["s".to_string(), "d".to_string(), "1".to_string(), String::new()]
        );
    }
}
