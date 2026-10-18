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
//!
//! Duas peças escrevem e releem esses arquivos:
//!
//! - [`Appender`] é a escrita do legado (`StreamWriter(path, append: true, AutoFlush = true)`: o
//!   cabeçalho sai na criação do arquivo, cada linha seguinte acrescenta e o arquivo aparece no disco
//!   durante a execução, não só no fim);
//! - [`RecordReader`] lê um CSV registro a registro e [`merge_consolidated`] junta os arquivos por
//!   servidor no consolidado. Os dois existem para a memória não crescer com o tamanho do resultado
//!   (critério do T34): o buffer é de dezenas de KB mesmo com centenas de MB de arquivo.

use std::io::Write as _;
use std::path::{Path, PathBuf};

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
pub fn render_csv(header: &[String], rows: &[Vec<String>]) -> String {
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
const WINDOWS_INVALID_FILE_NAME_CHARS: [char; 10] =
    ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];

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

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{fraction:06}0Z")
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

/// Cabeçalho de um CSV de resultado: `Server`, `Database` e as colunas da consulta.
pub fn header_for(columns: &[String]) -> Vec<String> {
    let mut header = vec!["Server".to_string(), "Database".to_string()];
    header.extend(columns.iter().cloned());
    header
}

/// Escritor de **append** de um CSV do `query run`.
///
/// É o `StreamWriter(path, append: true, AutoFlush = true)` + `CsvHelper` do legado: o arquivo nasce
/// com BOM e cabeçalho na **primeira** escrita (o `if (!fileExists)` do `CsvExporter`) e cada escrita
/// seguinte só acrescenta dados. O arquivo é aberto, escrito e fechado a cada chamada, como o legado
/// — por isso o CSV aparece no disco durante a execução, e não só no fim.
pub struct Appender {
    path: PathBuf,
    header: Vec<String>,
}

impl Appender {
    /// Prepara o arquivo `path`, que passará a ter o cabeçalho dado.
    pub fn new(path: PathBuf, header: Vec<String>) -> Self {
        Self { path, header }
    }

    /// Caminho do arquivo.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Acrescenta as linhas; na primeira vez o arquivo é criado com BOM e cabeçalho.
    ///
    /// O cabeçalho sai quando o arquivo está **vazio ou ausente**, como no .NET (o `StreamWriter`
    /// escreve o preâmbulo quando a posição é zero).
    pub fn append(&self, rows: &[Vec<String>]) -> std::io::Result<()> {
        let empty = std::fs::metadata(&self.path)
            .map(|metadata| metadata.len() == 0)
            .unwrap_or(true);

        if !empty && rows.is_empty() {
            return Ok(());
        }

        // Arquivo novo (ou vazio) sai inteiro, como o `StreamWriter` do legado; existente só ganha
        // as linhas novas.
        let content = if empty {
            render_csv(&self.header, rows)
        } else {
            let mut content = String::new();
            for row in rows {
                write_record(&mut content, row);
            }
            content
        };

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(content.as_bytes())?;
        // `AutoFlush = true` do legado: os dados saem para o disco a cada escrita.
        file.flush()
    }
}

/// Quebra um CSV em registros e campos, com as regras que o `CsvHelper` usa para reler o que ele
/// mesmo escreveu: aspas dobradas dentro de campo citado, `\r\n` (ou `\n`, ou `\r`) terminando o
/// registro, quebra dentro de campo preservada e BOM inicial descartado.
pub fn parse_records(text: &str) -> Vec<Vec<String>> {
    let mut reader = RecordReader::new(text.as_bytes());
    let mut records: Vec<Vec<String>> = Vec::new();

    while let Ok(Some(record)) = reader.next_record() {
        records.push(record);
    }

    records
}

/// Leitor **incremental** de registros de um CSV, com as mesmas regras de [`parse_records`].
///
/// Existe para o merge do consolidado não carregar os arquivos na memória: um export de 1M linhas tem
/// dezenas de MB, e o critério do T34 é manter a memória estável. O buffer só guarda um punhado de
/// KBs por vez, independente do tamanho do arquivo.
struct RecordReader<R: std::io::Read> {
    source: R,
    /// Bytes lidos e ainda não consumidos.
    buffer: Vec<u8>,
    /// Próxima posição a ler em `buffer`.
    position: usize,
    /// Fim do arquivo alcançado.
    eof: bool,
    /// O BOM ainda não foi descartado (só no comecinho do arquivo).
    at_start: bool,
    /// Buffer reutilizado nas leituras.
    chunk: Vec<u8>,
}

/// Tamanho do pedaço lido do arquivo por vez.
const READ_CHUNK_BYTES: usize = 64 * 1024;

impl<R: std::io::Read> RecordReader<R> {
    fn new(source: R) -> Self {
        Self {
            source,
            buffer: Vec::new(),
            position: 0,
            eof: false,
            at_start: true,
            chunk: vec![0; READ_CHUNK_BYTES],
        }
    }

    /// Próximo registro, ou `None` no fim do arquivo.
    fn next_record(&mut self) -> std::io::Result<Option<Vec<String>>> {
        let mut record: Vec<String> = Vec::new();
        let mut field: Vec<u8> = Vec::new();
        let mut quoted = false;
        let mut started = false;

        while let Some(byte) = self.next_byte()? {
            started = true;

            if quoted {
                if byte != b'"' {
                    field.push(byte);
                } else if self.peek_byte()? == Some(b'"') {
                    self.next_byte()?;
                    field.push(b'"');
                } else {
                    quoted = false;
                }
                continue;
            }

            match byte {
                b'"' => quoted = true,
                b',' => record.push(text_of(std::mem::take(&mut field))),
                b'\r' | b'\n' => {
                    if byte == b'\r' && self.peek_byte()? == Some(b'\n') {
                        self.next_byte()?;
                    }
                    record.push(text_of(field));

                    return Ok(Some(record));
                }
                _ => field.push(byte),
            }
        }

        // O legado sempre fecha o registro com `\r\n`; um arquivo sem a quebra final não perde a linha.
        if !started {
            return Ok(None);
        }

        record.push(text_of(field));

        Ok(Some(record))
    }

    /// Próximo byte, descartando o BOM na primeira leitura.
    fn next_byte(&mut self) -> std::io::Result<Option<u8>> {
        if self.at_start {
            self.at_start = false;
            self.fill()?;
            if self.buffer[self.position..].starts_with(UTF8_BOM.as_bytes()) {
                self.position += UTF8_BOM.len();
            }
        }

        if self.position >= self.buffer.len() {
            if self.eof {
                return Ok(None);
            }
            self.fill()?;
            if self.position >= self.buffer.len() {
                return Ok(None);
            }
        }

        let byte = self.buffer[self.position];
        self.position += 1;

        Ok(Some(byte))
    }

    /// Byte seguinte **sem** consumir.
    fn peek_byte(&mut self) -> std::io::Result<Option<u8>> {
        if self.position >= self.buffer.len() {
            if self.eof {
                return Ok(None);
            }
            self.fill()?;
            if self.position >= self.buffer.len() {
                return Ok(None);
            }
        }

        Ok(Some(self.buffer[self.position]))
    }

    /// Lê mais um pedaço, descartando o que já foi consumido (o buffer não cresce com o arquivo).
    fn fill(&mut self) -> std::io::Result<()> {
        if self.position > 0 {
            self.buffer.drain(..self.position);
            self.position = 0;
        }
        if self.eof {
            return Ok(());
        }

        let mut chunk = std::mem::take(&mut self.chunk);
        let read = self.source.read(&mut chunk)?;
        chunk.truncate(read);
        self.buffer.extend_from_slice(&chunk);
        chunk.resize(READ_CHUNK_BYTES, 0);
        self.chunk = chunk;
        if read == 0 {
            self.eof = true;
        }

        Ok(())
    }
}

/// Campo do CSV a partir dos bytes crus (o arquivo é UTF-8).
fn text_of(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Colunas do cabeçalho de um arquivo por servidor — e só isso —, ou `None` quando o arquivo não
/// existe ou não tem **nenhuma** linha de dados.
///
/// O legado sai cedo (`if (records.Count == 0) return [];`) antes de olhar o cabeçalho — então um
/// arquivo só com cabeçalho não contribui nem com as colunas dele para o consolidado.
fn server_file_header(path: &Path) -> std::io::Result<Option<Vec<String>>> {
    let mut reader = RecordReader::new(std::fs::File::open(path)?);

    let Some(header) = reader.next_record()? else {
        return Ok(None);
    };
    if reader.next_record()?.is_none() {
        return Ok(None);
    }

    Ok(Some(header))
}

/// Onde cada coluna do cabeçalho do arquivo está no resultado que chegou.
///
/// É a tradução do `dataRow.ContainsKey(columnName) ? dataRow[columnName] : string.Empty` do
/// `CsvExporter`: quando dois bancos do mesmo servidor devolvem colunas diferentes, quem manda é o
/// cabeçalho do **primeiro** resultado, e o valor que não tem coluna correspondente fica vazio.
pub fn projection(columns: &[String], header: &[String]) -> Vec<Option<usize>> {
    header
        .iter()
        .map(|column| columns.iter().position(|candidate| candidate == column))
        .collect()
}

/// Linha do arquivo: `Server`, `Database` e os valores na ordem do cabeçalho do arquivo.
pub fn project_row(
    server: &str,
    database: &str,
    values: &[String],
    projection: &[Option<usize>],
) -> Vec<String> {
    let mut fields = Vec::with_capacity(projection.len() + 2);
    fields.push(server.to_string());
    fields.push(database.to_string());
    for index in projection {
        fields.push(
            index
                .and_then(|index| values.get(index).cloned())
                .unwrap_or_default(),
        );
    }

    fields
}

/// Quantas linhas o merge acumula antes de cada escrita no consolidado.
const MERGE_BATCH_ROWS: usize = 512;

/// Faz o merge dos CSVs por servidor no consolidado, **relendo os arquivos**, como o
/// `MergeServerCsvsToConsolidated` do legado.
///
/// O que o byte a byte do oráculo mostra, e que está reproduzido aqui: as colunas do consolidado são
/// a união das colunas dos arquivos **na ordem dos servidores** (e, dentro de cada arquivo, na ordem
/// do cabeçalho dele) e cada linha traz os valores do arquivo dela, lida **campo a campo** com o
/// cabeçalho — inclusive quando a linha tem campos a mais (o caso torto de um segundo banco com
/// outras colunas, que o oráculo escreve e relê assim mesmo). Sem nenhuma linha de dados o
/// consolidado **não** é criado (o `allResults.Count == 0` do legado), nem sobra dele um arquivo só
/// com cabeçalho.
///
/// A leitura é em dois passos — cabeçalhos primeiro, para a união e para saber quem entra, e depois
/// as linhas —, e vai em pedacos: a memória não cresce com o tamanho dos arquivos.
pub fn merge_consolidated(
    directory: &Path,
    timestamp: &str,
    server_names: &[String],
) -> std::io::Result<Option<PathBuf>> {
    let mut headers: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for server in server_names {
        let path = directory.join(server_file_name(server, timestamp));
        if !path.is_file() {
            continue;
        }
        if let Some(header) = server_file_header(&path)? {
            headers.push((path, header));
        }
    }

    if headers.is_empty() {
        return Ok(None);
    }

    let mut columns: Vec<String> = Vec::new();
    for (_, header) in &headers {
        for column in header.get(2..).unwrap_or_default() {
            if !columns.contains(column) {
                columns.push(column.clone());
            }
        }
    }

    let appender = Appender::new(
        directory.join(consolidated_file_name(timestamp)),
        header_for(&columns),
    );
    let mut batch: Vec<Vec<String>> = Vec::new();

    for (path, header) in &headers {
        let projection = projection(header.get(2..).unwrap_or_default(), &columns);
        let mut reader = RecordReader::new(std::fs::File::open(path)?);
        // O cabeçalho do arquivo já foi lido no primeiro passo.
        reader.next_record()?;

        while let Some(record) = reader.next_record()? {
            let server = record.first().cloned().unwrap_or_default();
            let database = record.get(1).cloned().unwrap_or_default();
            batch.push(project_row(
                &server,
                &database,
                record.get(2..).unwrap_or_default(),
                &projection,
            ));

            if batch.len() >= MERGE_BATCH_ROWS {
                appender.append(&batch)?;
                batch.clear();
            }
        }
    }

    if !batch.is_empty() {
        appender.append(&batch)?;
    }

    Ok(Some(appender.path().to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_of(columns: &[&str]) -> Vec<String> {
        let mut header = vec!["Server".to_string(), "Database".to_string()];
        header.extend(columns.iter().map(|column| (*column).to_string()));
        header
    }

    fn columns_of(columns: &[&str]) -> Vec<String> {
        columns.iter().map(|column| (*column).to_string()).collect()
    }

    #[test]
    fn a_csv_starts_with_the_bom_and_uses_crlf() {
        let csv = render_csv(
            &header_of(&["id"]),
            &[vec!["s".into(), "d".into(), "1".into()]],
        );

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
        assert_eq!(
            server_file_name("127.0.0.1", "2026-10-18_143228"),
            "127.0.0.1_2026-10-18_143228.csv"
        );
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
        // 2026-10-18T14:32:28Z — época conferida no próprio Postgres:
        // `SELECT extract(epoch from '2026-10-18T14:32:28Z'::timestamptz)` = 1792333948.
        let unix_micros: i64 = 1_792_333_948 * 1_000_000;
        let stamp = timestamp_for(
            std::time::UNIX_EPOCH + std::time::Duration::from_micros(unix_micros as u64),
        );

        assert_eq!(stamp, "2026-10-18_143228");
    }

    #[test]
    fn the_log_timestamp_uses_the_dotnet_round_trip_format() {
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_micros(1_792_333_948_228_411);
        // `ExecutedAt` do oráculo: `2026-10-18T14:32:28.2284119Z` (7 casas de fração).
        assert_eq!(iso8601_for(now), "2026-10-18T14:32:28.2284110Z");
    }

    #[test]
    fn an_appender_writes_the_header_once_and_appends_the_rest() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("dev_ts.csv");
        let appender = Appender::new(path.clone(), header_of(&["id", "nome"]));

        appender
            .append(&[vec!["dev".into(), "a".into(), "1".into(), "x".into()]])
            .expect("primeira escrita");
        appender
            .append(&[vec!["dev".into(), "b".into(), "2".into(), "y".into()]])
            .expect("append");

        assert_eq!(
            std::fs::read_to_string(&path).expect("lê"),
            "\u{feff}Server,Database,id,nome\r\ndev,a,1,x\r\ndev,b,2,y\r\n",
            "BOM e cabeçalho só na criação"
        );
    }

    #[test]
    fn an_empty_result_still_creates_the_file_with_just_the_header() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("dev_ts.csv");

        Appender::new(path.clone(), header_of(&["id"]))
            .append(&[])
            .expect("cria");

        // O legado cria o arquivo na primeira linha escrita, mesmo sem dado nenhum.
        assert_eq!(
            std::fs::read_to_string(&path).expect("lê"),
            "\u{feff}Server,Database,id\r\n"
        );
    }

    #[test]
    fn the_parser_round_trips_what_the_writer_escapes() {
        let rows = vec![vec![
            "dev".to_string(),
            "a".to_string(),
            "com, vírgula".to_string(),
            "com \"aspas\"".to_string(),
            "linha1\nlinha2".to_string(),
            String::new(),
        ]];
        let text = render_csv(&header_of(&["a", "b", "c", "d"]), &rows);

        assert_eq!(parse_records(&text), {
            let mut expected = vec![header_of(&["a", "b", "c", "d"])];
            expected.extend(rows);
            expected
        });
    }

    #[test]
    fn the_parser_accepts_bare_lf_and_a_missing_final_newline() {
        assert_eq!(
            parse_records("a,b\nc,d"),
            vec![columns_of(&["a", "b"]), columns_of(&["c", "d"])]
        );
    }

    #[test]
    fn a_server_file_without_data_rows_contributes_nothing() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("dev_ts.csv");
        std::fs::write(&path, "\u{feff}Server,Database,id\r\n").expect("semeadura");

        assert!(
            server_file_header(&path).expect("lê").is_none(),
            "o legado sai antes de ler o cabeçalho"
        );
    }

    #[test]
    fn the_merge_unions_the_columns_and_keeps_the_first_appearance_order() {
        let directory = tempfile::tempdir().expect("tempdir");
        let timestamp = "2026-10-18_143228";
        // O servidor `um` tem as colunas `id,nome`; o `dois`, `nome,id` — a ordem do header de cada
        // arquivo é o do primeiro resultado que completou nele.
        std::fs::write(
            directory.path().join(server_file_name("um", timestamp)),
            render_csv(
                &header_of(&["id", "nome"]),
                &[vec!["um".into(), "a".into(), "1".into(), "x".into()]],
            ),
        )
        .expect("semeadura");
        std::fs::write(
            directory.path().join(server_file_name("dois", timestamp)),
            render_csv(
                &header_of(&["nome", "id"]),
                &[vec!["dois".into(), "b".into(), "y".into(), "2".into()]],
            ),
        )
        .expect("semeadura");

        let path = merge_consolidated(
            directory.path(),
            timestamp,
            &["um".to_string(), "dois".to_string()],
        )
        .expect("merge")
        .expect("tem consolidado");

        assert_eq!(
            std::fs::read_to_string(&path).expect("lê"),
            "\u{feff}Server,Database,id,nome\r\num,a,1,x\r\ndois,b,2,y\r\n",
            "a união é id,nome (ordem do primeiro arquivo) e os valores são remapeados"
        );
    }

    #[test]
    fn the_merge_prefers_the_server_order_and_fills_what_the_file_does_not_have() {
        let directory = tempfile::tempdir().expect("tempdir");
        let timestamp = "ts";
        std::fs::write(
            directory.path().join(server_file_name("um", timestamp)),
            render_csv(
                &header_of(&["id"]),
                &[vec!["um".into(), "a".into(), "1".into()]],
            ),
        )
        .expect("semeadura");
        std::fs::write(
            directory.path().join(server_file_name("dois", timestamp)),
            render_csv(
                &header_of(&["outro"]),
                &[vec!["dois".into(), "b".into(), "z".into()]],
            ),
        )
        .expect("semeadura");

        let path = merge_consolidated(
            directory.path(),
            timestamp,
            &["um".to_string(), "dois".to_string()],
        )
        .expect("merge")
        .expect("tem consolidado");

        assert_eq!(
            std::fs::read_to_string(&path).expect("lê"),
            "\u{feff}Server,Database,id,outro\r\num,a,1,\r\ndois,b,,z\r\n",
            "coluna que o arquivo não tem vira vazio"
        );
    }

    #[test]
    fn the_merge_reads_a_ragged_line_positionally() {
        // O que o oráculo escreveu no cenário C do T34: o segundo banco trouxe colunas diferentes, a
        // linha dele ficou com campos a mais do que o cabeçalho, e a releitura casa **campo a campo**
        // com o cabeçalho — então o valor excedente (`20000000`) sai debaixo do nome da coluna
        // daquela posição (`nome`). É torto e é o que o legado faz.
        let directory = tempfile::tempdir().expect("tempdir");
        let timestamp = "ts";
        std::fs::write(
            directory.path().join(server_file_name("dev", timestamp)),
            "\u{feff}Server,Database,id,nome\r\ndev,c,1,20000000\r\n",
        )
        .expect("semeadura");

        let path = merge_consolidated(directory.path(), timestamp, &["dev".to_string()])
            .expect("merge")
            .expect("tem consolidado");

        assert_eq!(
            std::fs::read_to_string(&path).expect("lê"),
            "\u{feff}Server,Database,id,nome\r\ndev,c,1,20000000\r\n"
        );
    }

    #[test]
    fn the_merge_writes_nothing_when_no_server_file_has_rows() {
        let directory = tempfile::tempdir().expect("tempdir");
        let timestamp = "ts";
        std::fs::write(
            directory.path().join(server_file_name("um", timestamp)),
            render_csv(&header_of(&["id"]), &[]),
        )
        .expect("semeadura");

        assert!(
            merge_consolidated(directory.path(), timestamp, &["um".to_string()])
                .expect("merge")
                .is_none(),
            "só cabeçalho não gera consolidado"
        );
    }
}
