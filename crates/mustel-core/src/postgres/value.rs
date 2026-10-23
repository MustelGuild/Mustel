//! Conversão de valores do servidor para o texto que vai ao CSV.
//!
//! **Esta é a parte sensível a paridade do `query run`.** O legado escreve CSV a partir de
//! `reader.GetValue(i)?.ToString() ?? string.Empty` — ou seja, o texto é o **`ToString()` do objeto
//! .NET que o Npgsql devolve**, e não a representação textual do PostgreSQL. Isso foi medido
//! (T32) rodando o oráculo contra um Postgres real, e o resultado é contraintuitivo:
//!
//! | Tipo do servidor | Texto no CSV |
//! |------------------|--------------|
//! | `bool` | `True` / `False` (maiúsculas) |
//! | `int2/4/8` | dígitos |
//! | `float4/8` | `1.5`, `2` (sem `2.0`), `NaN`, `Infinity` |
//! | `numeric` | texto com a **escala** do valor (`0.00`) |
//! | `text/varchar/enum` | como veio |
//! | `json/jsonb` | texto canônico do PostgreSQL (`{"k": 1}`) |
//! | `bytea` | `System.Byte[]` (o .NET imprime o **tipo**, não o conteúdo) |
//! | array | `System.String[]`, `System.Int32[]`, ... (idem) |
//! | `uuid` | `3f2504e0-4f89-11d3-9a0c-0305e82c3301` (minúsculas, com hífens) |
//! | `timestamp(tz)` | `10/18/2026 10:52:15 AM` (cultura **en-US**, em UTC, sem fração) |
//! | `date` | `10/18/2026` |
//! | `time` | `10:52 AM` (padrão curto: **sem** segundos) |
//!
//! **Divergência deliberada (registrada)**: no .NET esses textos de data/hora e de número vêm da
//! **cultura da máquina**; aqui ficam fixos na convenção en-US (que é a da máquina onde o oráculo foi
//! medido e a convenção natural de CSV). Numa máquina pt-BR o legado escreveria `18/10/2026` e
//! `1500,75`.
//!
//! O desenho separa duas coisas: [`TextValue`] descreve o que veio do servidor (construível em teste
//! sem conexão) e [`format_cell`] decide o texto. A fronteira com o driver (que produz `TextValue` a
//! partir de uma linha) fica em [`crate::postgres::client`].

/// Valor de uma célula, já reduzido ao que importa para exportar.
#[derive(Debug, Clone, PartialEq)]
pub enum TextValue<'a> {
    /// `NULL` do SQL.
    Null,
    /// Booleano.
    Bool(bool),
    /// Inteiro (int2/int4/int8).
    Int(i64),
    /// Ponto flutuante de 8 bytes (float8).
    Float(f64),
    /// Ponto flutuante de 4 bytes (float4) — formatado **como `f32`**, senão `0.1f` viraria
    /// `0.10000000149011612` na conversão para `f64`.
    Float32(f32),
    /// `numeric` — texto já com a escala do valor (`0.00` não vira `0`).
    Decimal(String),
    /// Texto, `varchar`, `char`, enum, `json`/`jsonb`.
    Text(&'a str),
    /// `bytea`: o .NET imprime o **tipo** do objeto, não o conteúdo.
    Bytes,
    /// Array: idem — o nome do tipo do array no .NET (`System.String[]`, `System.Int32[]`, ...).
    Array(&'static str),
    /// `timestamp`/`timestamptz`: microssegundos desde 2000-01-01T00:00:00Z.
    Timestamp(i64),
    /// `date`: dias desde 2000-01-01.
    Date(i32),
    /// `time`: microssegundos desde a meia-noite.
    Time(i64),
    /// `uuid`.
    Uuid([u8; 16]),
}

/// Texto de uma célula para o CSV, com a semântica do `ToString()` do .NET.
///
/// `NULL` vira campo vazio — o mesmo que o export do legado faz para valor nulo (e por isso nulo e
/// string vazia são indistinguíveis no CSV, como no oráculo).
pub fn format_cell(value: &TextValue<'_>) -> String {
    match value {
        TextValue::Null => String::new(),
        TextValue::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        TextValue::Int(value) => value.to_string(),
        TextValue::Float(value) => format_float(*value),
        TextValue::Float32(value) => format_float32(*value),
        TextValue::Decimal(text) => text.clone(),
        TextValue::Text(value) => (*value).to_string(),
        TextValue::Bytes => "System.Byte[]".to_string(),
        TextValue::Array(name) => (*name).to_string(),
        TextValue::Timestamp(micros) => format_timestamp(*micros),
        TextValue::Date(days) => format_date(*days),
        TextValue::Time(micros) => format_time(*micros),
        TextValue::Uuid(bytes) => format_uuid(bytes),
    }
}

/// `float` como o `double.ToString()` do .NET.
///
/// O Rust já formata com o menor número de dígitos que ida-e-volta representa o valor (igual ao
/// `"R"` do .NET), então `2.0` sai `2` — **sem** o `.0` que a primeira versão deste módulo
/// acrescentava. Os nomes de `NaN`/infinito seguem o .NET.
///
/// **Ressalva conhecida**: para valores muito grandes ou muito pequenos o .NET troca para notação
/// científica (`1E+20`) e o Rust não (`100000000000000000000`) — divergência não verificada no
/// oráculo por não haver esse caso nos fixtures.
fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }

    value.to_string()
}

/// `float` de 4 bytes, formatado com os dígitos do próprio `f32`.
///
/// Formatar como `f64` mostraria o ruído da conversão (`0.1` viraria `0.10000000149011612`), e o
/// `ToString()` do .NET imprime os dígitos do `float`.
fn format_float32(value: f32) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }

    value.to_string()
}

/// Dias entre 1970-01-01 e 2000-01-01 (época do `timestamp` do PostgreSQL).
const EPOCH_2000_IN_DAYS: i64 = 10_957;

/// `timestamp`/`timestamptz` no formato `M/d/yyyy h:mm:ss tt` (en-US), em UTC, sem fração.
///
/// O Npgsql devolve `DateTime` com `Kind = Utc` para `timestamptz` e o `ToString()` imprime a hora
/// **UTC** como veio, sem offset — é o que se vê no CSV do oráculo.
fn format_timestamp(micros_since_2000: i64) -> String {
    let total_micros = micros_since_2000 + EPOCH_2000_IN_DAYS * 86_400_000_000;
    format_datetime_en_us(total_micros)
}

/// `date` no formato `M/d/yyyy` (o `DateOnly.ToString()` do .NET usa o padrão curto da cultura).
fn format_date(days_since_2000: i32) -> String {
    let days = i64::from(days_since_2000) + EPOCH_2000_IN_DAYS;
    let (year, month, day) = civil_from_days(days);

    format!("{month}/{day}/{year}")
}

/// `time` no formato curto `h:mm tt` — o `TimeOnly.ToString()` do .NET **não** mostra segundos.
fn format_time(micros_since_midnight: i64) -> String {
    let seconds_in_day = micros_since_midnight.div_euclid(1_000_000);
    let hour = seconds_in_day / 3_600;
    let minute = (seconds_in_day % 3_600) / 60;
    let (hour12, meridiem) = twelve_hour(hour);

    format!("{hour12}:{minute:02} {meridiem}")
}

/// `uuid` como o `Guid.ToString()`: minúsculas, com hífens nas posições 8-4-4-4-12.
fn format_uuid(bytes: &[u8; 16]) -> String {
    let hex = |range: std::ops::Range<usize>| -> String {
        bytes[range]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    };

    format!(
        "{}-{}-{}-{}-{}",
        hex(0..4),
        hex(4..6),
        hex(6..8),
        hex(8..10),
        hex(10..16)
    )
}

/// Data e hora civis a partir de microssegundos desde 1970-01-01T00:00:00Z, no formato en-US.
fn format_datetime_en_us(total_micros: i64) -> String {
    let days = total_micros.div_euclid(86_400_000_000);
    let micros_of_day = total_micros.rem_euclid(86_400_000_000);
    let (year, month, day) = civil_from_days(days);

    let seconds_of_day = micros_of_day / 1_000_000;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (hour12, meridiem) = twelve_hour(hour);

    format!("{month}/{day}/{year} {hour12}:{minute:02}:{second:02} {meridiem}")
}

/// Hora no relógio de 12 horas e o marcador (`AM`/`PM`) — a meia-noite é `12 AM`.
fn twelve_hour(hour: i64) -> (i64, &'static str) {
    let meridiem = if hour < 12 { "AM" } else { "PM" };
    let hour12 = match hour % 12 {
        0 => 12,
        other => other,
    };

    (hour12, meridiem)
}

/// Data civil (ano, mês, dia) a partir de dias desde 1970-01-01.
///
/// Algoritmo `civil_from_days` de Howard Hinnant (`chrono` faz o mesmo, mas o projeto evita uma
/// dependência só para formatar uma data). Público porque o `query` também precisa dele para montar
/// o timestamp dos nomes de arquivo e do log.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;

    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CONTRACT: IF-032
    fn null_becomes_an_empty_field() {
        assert_eq!(format_cell(&TextValue::Null), "");
    }

    /// Os textos abaixo são os que o **oráculo escreveu** no CSV do T32, com o mesmo dado.
    #[test]
    fn scalars_match_the_oracle_csv() {
        assert_eq!(format_cell(&TextValue::Bool(true)), "True");
        assert_eq!(format_cell(&TextValue::Bool(false)), "False");
        assert_eq!(
            format_cell(&TextValue::Int(9223372036854775807)),
            "9223372036854775807"
        );
        assert_eq!(format_cell(&TextValue::Int(-1)), "-1");
        assert_eq!(
            format_cell(&TextValue::Decimal("1500.75".to_string())),
            "1500.75"
        );
        assert_eq!(format_cell(&TextValue::Decimal("0.00".to_string())), "0.00");
        assert_eq!(format_cell(&TextValue::Text("mydb")), "mydb");
        assert_eq!(format_cell(&TextValue::Text("{\"k\": 1}")), "{\"k\": 1}");
    }

    #[test]
    /// CONTRACT: IF-030
    fn floats_follow_the_dotnet_format() {
        assert_eq!(format_cell(&TextValue::Float(1.5)), "1.5");
        // O oráculo escreveu `2` (não `2.0`) para `float8` 2.
        assert_eq!(format_cell(&TextValue::Float(2.0)), "2");
        assert_eq!(format_cell(&TextValue::Float(f64::NAN)), "NaN");
        assert_eq!(format_cell(&TextValue::Float(f64::INFINITY)), "Infinity");
        assert_eq!(
            format_cell(&TextValue::Float(f64::NEG_INFINITY)),
            "-Infinity"
        );
    }

    #[test]
    /// CONTRACT: IF-030
    fn float4_is_formatted_as_f32() {
        // O `ToString()` do `float` (4 bytes) do .NET imprime os dígitos do `f32`; converter para
        // `f64` antes mostraria o ruído da representação.
        assert_eq!(format_cell(&TextValue::Float32(0.1)), "0.1");
        assert_eq!(format_cell(&TextValue::Float32(1.5)), "1.5");
    }

    #[test]
    /// CONTRACT: IF-031
    fn bytea_and_arrays_show_the_dotnet_type_name() {
        assert_eq!(format_cell(&TextValue::Bytes), "System.Byte[]");
        assert_eq!(
            format_cell(&TextValue::Array("System.String[]")),
            "System.String[]"
        );
        assert_eq!(
            format_cell(&TextValue::Array("System.Int32[]")),
            "System.Int32[]"
        );
    }

    #[test]
    /// CONTRACT: IF-029
    fn timestamps_use_the_en_us_general_format() {
        // Números conferidos no próprio Postgres: `'2026-10-18'::date - '2000-01-01'::date` = 9787.
        // `2026-10-18T10:52:15.123456+00:00` → `10/18/2026 10:52:15 AM` no oráculo.
        assert_eq!(
            format_cell(&TextValue::Timestamp(845_635_935_123_456)),
            "10/18/2026 10:52:15 AM"
        );

        // `2026-01-02T03:04:05+00:00` (9498 dias) → `1/2/2026 3:04:05 AM` (sem zero à esquerda).
        assert_eq!(
            format_cell(&TextValue::Timestamp(820_638_245_000_000)),
            "1/2/2026 3:04:05 AM"
        );
    }

    #[test]
    /// CONTRACT: IF-029
    fn dates_and_times_use_the_short_patterns() {
        // `date` 2026-10-18 → `10/18/2026`; `date` 2026-01-02 → `1/2/2026`.
        assert_eq!(format_cell(&TextValue::Date(9_787)), "10/18/2026");
        assert_eq!(format_cell(&TextValue::Date(9_498)), "1/2/2026");

        // `time` 10:52:15.123456 → `10:52 AM` (o padrão curto esconde os segundos).
        let time = (10 * 3_600 + 52 * 60 + 15) * 1_000_000 + 123_456;
        assert_eq!(format_cell(&TextValue::Time(time)), "10:52 AM");
        assert_eq!(
            format_cell(&TextValue::Time((3 * 3_600 + 4 * 60 + 5) * 1_000_000)),
            "3:04 AM"
        );

        // Meio-dia e meia-noite no relógio de 12 horas.
        assert_eq!(
            format_cell(&TextValue::Time(12 * 3_600_000_000)),
            "12:00 PM"
        );
        assert_eq!(format_cell(&TextValue::Time(0)), "12:00 AM");
    }

    #[test]
    /// CONTRACT: IF-032
    fn uuid_is_lowercase_with_hyphens() {
        let bytes = [
            0x3f, 0x25, 0x04, 0xe0, 0x4f, 0x89, 0x11, 0xd3, 0x9a, 0x0c, 0x03, 0x05, 0xe8, 0x2c,
            0x33, 0x01,
        ];
        assert_eq!(
            format_cell(&TextValue::Uuid(bytes)),
            "3f2504e0-4f89-11d3-9a0c-0305e82c3301"
        );
        assert_eq!(
            format_cell(&TextValue::Uuid([0; 16])),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn civil_dates_round_trip_over_a_leap_year() {
        // 2024-02-29 (ano bissexto) e 1970-01-01 (origem).
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(EPOCH_2000_IN_DAYS), (2000, 1, 1));
        // Antes da origem (datas negativas são válidas em `date`).
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
