//! Validação de entradas (porte de `legacy/FurLab.CLI/SecurityUtils.cs`).
//!
//! Portado **fielmente**, inclusive onde o legado é permissivo: a checagem de IPv4 aceita
//! `999.999.999.999` (o padrão é só `digitos.digitos.digitos.digitos`) e a de hostname aceita
//! `*`, que é curinga do `pgpass.conf`. Divergir aqui mudaria o que cada comando aceita — e o
//! contrato de paridade é o comportamento observado.

use std::sync::LazyLock;

use regex::Regex;

/// Identificador PostgreSQL: letra/underscore seguido de letras, dígitos ou underscore, até 63.
pub fn is_valid_postgres_identifier(identifier: &str) -> bool {
    static PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").expect("valid regex"));

    if identifier.trim().is_empty() || identifier.len() > 63 {
        return false;
    }

    PATTERN.is_match(identifier)
}

/// Host: nome de domínio, `localhost`, IPv4, IPv6 (com ou sem colchetes) e o curinga `*`.
pub fn is_valid_host(host: &str) -> bool {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])(\.([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9]))*$|^localhost$|^(\d{1,3}\.){3}\d{1,3}$|^(\[?[0-9a-fA-F:]+\]?)$",
        )
        .expect("valid regex")
    });

    if host.trim().is_empty() || host.len() > 253 || host.contains('\0') {
        return false;
    }

    if host == "*" {
        return true;
    }

    PATTERN.is_match(host)
}

/// Usuário: letras, dígitos, `_`, `.` e `-`, até 63, ou o curinga `*`.
pub fn is_valid_username(username: &str) -> bool {
    static PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_.\-]+$").expect("valid regex"));

    if username.trim().is_empty() || username.len() > 63 {
        return false;
    }

    if username == "*" {
        return true;
    }

    PATTERN.is_match(username)
}

/// Porta: 1–65535, ou o curinga `*` do `pgpass.conf`.
pub fn is_valid_port(port: &str) -> bool {
    if port == "*" {
        return true;
    }

    match port.parse::<u32>() {
        Ok(value) => (1..=65535).contains(&value),
        Err(_) => false,
    }
}

/// Porta numérica dentro da faixa exigida pelo legado (1–65535).
pub fn is_port_in_range(port: i64) -> bool {
    (1..=65535).contains(&port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_identifier_follows_the_legacy_rules() {
        assert!(is_valid_postgres_identifier("mydb"));
        assert!(is_valid_postgres_identifier("_tmp"));
        assert!(
            !is_valid_postgres_identifier("1db"),
            "não começa com dígito"
        );
        assert!(
            !is_valid_postgres_identifier("my-db"),
            "hífen não é permitido"
        );
        assert!(!is_valid_postgres_identifier(""));
        assert!(!is_valid_postgres_identifier(&"a".repeat(64)));
    }

    #[test]
    fn host_accepts_names_localhost_ips_and_wildcard() {
        for valid in [
            "localhost",
            "db.local",
            "prod-db.company.com",
            "10.0.0.1",
            "::1",
            "[::1]",
            "*",
        ] {
            assert!(is_valid_host(valid), "deveria aceitar {valid}");
        }

        for invalid in ["", "   ", "host_com_underscore", "-leading.example"] {
            assert!(!is_valid_host(invalid), "deveria recusar {invalid}");
        }
    }

    #[test]
    fn host_keeps_the_legacy_looseness_for_ipv4() {
        // O padrão do legado é `(\d{1,3}\.){3}\d{1,3}`: não valida faixa de octeto.
        assert!(is_valid_host("999.999.999.999"));
    }

    #[test]
    fn username_accepts_safe_characters_and_wildcard() {
        for valid in ["postgres", "app.user", "app-user", "app_user", "*"] {
            assert!(is_valid_username(valid), "deveria aceitar {valid}");
        }

        for invalid in ["", "with space", "quotes'", &"a".repeat(64)] {
            assert!(!is_valid_username(invalid), "deveria recusar {invalid}");
        }
    }

    #[test]
    fn port_accepts_range_and_wildcard() {
        assert!(is_valid_port("1"));
        assert!(is_valid_port("5432"));
        assert!(is_valid_port("65535"));
        assert!(is_valid_port("*"));

        assert!(!is_valid_port("0"));
        assert!(!is_valid_port("65536"));
        assert!(!is_valid_port("abc"));
        assert!(!is_valid_port(""));
    }

    #[test]
    fn numeric_port_range_matches_the_legacy_check() {
        assert!(is_port_in_range(1));
        assert!(is_port_in_range(5432));
        assert!(is_port_in_range(65535));
        assert!(!is_port_in_range(0));
        assert!(!is_port_in_range(65536));
        assert!(!is_port_in_range(-1));
    }
}
