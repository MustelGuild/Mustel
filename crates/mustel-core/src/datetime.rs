//! Datas e horas em **UTC**, sem dependência de calendário.
//!
//! O projeto evita `chrono`; a conversão civil (algoritmo `civil_from_days` de Howard Hinnant) já
//! existia no leitor de `timestamp` do Postgres e agora é compartilhada com o log de atividade. Tudo
//! aqui é **UTC**: é a convenção do CSV e dos nomes de arquivo da execução.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milissegundos entre 1970-01-01T00:00:00Z e agora.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Dias inteiros desde 1970-01-01 (truncando a fração do dia).
pub fn days_since_epoch(at_ms: u64) -> i64 {
    (at_ms / 86_400_000) as i64
}

/// Data civil (ano, mês, dia) a partir de dias desde 1970-01-01.
///
/// Algoritmo `civil_from_days` de Howard Hinnant (`chrono` faz o mesmo, mas o projeto evita uma
/// dependência só para formatar uma data). Público porque o `query`, o log de atividade e o
/// `postgres` montam nomes de arquivo e textos a partir dele.
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

/// `Ano-Mês-Dia Hora:Minuto:Segundo` em UTC (a hora do log e do relatório).
pub fn format_utc(at_ms: u64) -> String {
    let (year, month, day, hour, minute, second) = parts(at_ms);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// `Ano-Mês-Dia` em UTC (o nome do arquivo diário do log).
pub fn day_key(at_ms: u64) -> String {
    let (year, month, day, _, _, _) = parts(at_ms);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `AnoMêsDia-HoraMinutoSegundo` em UTC (o nome do `.log` do relatório).
pub fn format_utc_compact(at_ms: u64) -> String {
    let (year, month, day, hour, minute, second) = parts(at_ms);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

/// As seis partes civis de um instante em UTC.
fn parts(at_ms: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = (at_ms / 86_400_000) as i64;
    let seconds = (at_ms % 86_400_000) / 1_000;
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-06T23:20:00Z, o instante usado nos testes de log.
    const NOVEMBER_6_2026: u64 = 20_763 * 86_400_000 + (23 * 3_600 + 20 * 60) * 1_000;

    #[test]
    fn the_epoch_is_the_first_of_january_1970() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(format_utc(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn a_leap_day_survives() {
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(
            format_utc(19_782 * 86_400_000 + 3_600_000),
            "2024-02-29 01:00:00"
        );
    }

    #[test]
    fn a_day_before_the_epoch_is_handled() {
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn utc_formatting_is_zero_padded() {
        assert_eq!(
            format_utc(NOVEMBER_6_2026),
            "2026-11-06 23:20:00",
            "mês, dia, hora e minuto com dois dígitos"
        );
        assert_eq!(day_key(NOVEMBER_6_2026), "2026-11-06");
        assert_eq!(
            format_utc_compact(NOVEMBER_6_2026),
            "20261106-232000",
            "o nome do relatório não usa separadores de caminho"
        );
    }

    #[test]
    fn the_day_counter_matches_the_civil_date() {
        assert_eq!(days_since_epoch(NOVEMBER_6_2026), 20_763);
        assert_eq!(
            civil_from_days(days_since_epoch(NOVEMBER_6_2026)),
            (2026, 11, 6)
        );
    }

    #[test]
    fn now_is_after_the_epoch() {
        assert!(now_ms() > NOVEMBER_6_2026, "o relógio do sistema avançou");
    }
}
