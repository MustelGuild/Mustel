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

/// Padrões de travessia de caminho que o legado procura (`SecurityUtils.PathTraversalPatterns`).
const PATH_TRAVERSAL_PATTERNS: [&str; 11] = [
    "..",
    "../",
    "..\\",
    "%2e%2e",
    "%2e%2e%2f",
    "%2e%2e%5c",
    "..%2f",
    "..%5c",
    "%252e%252e",
    "....//",
    "..\\..\\",
];

/// Pastas que o legado recusa como destino, no Unix (`/etc`, `/bin`, ...).
#[cfg(not(windows))]
const SENSITIVE_UNIX_PATHS: [&str; 5] = ["/etc", "/usr/bin", "/usr/sbin", "/bin", "/sbin"];

/// Caminho seguro para operar: sem travessia (`..`), sem byte nulo, sem caractere inválido e fora de
/// pastas de sistema.
///
/// Porte de `SecurityUtils.IsValidPath` **sem** o parâmetro `basePath` (o legado chama sem ele nas
/// duas pontas). Como no original, a checagem de pasta sensível é feita sobre o caminho **absoluto**
/// e por prefixo simples — `C:\WindowsUpdate` também é recusado, porque o legado não exige a
/// fronteira de diretório. As pastas de sistema do Windows vêm do ambiente (`SystemRoot`,
/// `ProgramFiles`), que é como o `SpecialFolder` do .NET as resolve.
pub fn is_valid_path(path: &str) -> bool {
    if path.trim().is_empty() {
        return false;
    }

    let lowercase = path.to_lowercase();
    if PATH_TRAVERSAL_PATTERNS
        .iter()
        .any(|pattern| lowercase.contains(pattern))
    {
        return false;
    }

    if path.contains('\0') {
        return false;
    }

    if path.chars().any(is_invalid_path_char) {
        return false;
    }

    let absolute = std::path::absolute(path)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());

    !sensitive_paths().iter().any(|sensitive| {
        absolute
            .to_lowercase()
            .starts_with(&sensitive.to_lowercase())
    })
}

/// Caracteres que o `Path.GetInvalidPathChars` do .NET recusa.
fn is_invalid_path_char(character: char) -> bool {
    // No Unix o .NET só recusa o byte nulo.
    #[cfg(not(windows))]
    {
        character == '\0'
    }

    // No Windows: nulo, controles e `|` (mais `"`, `<` e `>` no nome de arquivo, que o caminho
    // completo do legado não chega a conter porque o `Path.GetFullPath` já teria recusado).
    #[cfg(windows)]
    {
        character == '\0' || character == '|' || ('\u{1}'..='\u{1f}').contains(&character)
    }
}

/// Pastas de sistema que o legado recusa (vazias quando o SO não expõe a pasta).
fn sensitive_paths() -> Vec<String> {
    #[cfg(windows)]
    {
        let mut paths: Vec<String> = ["SystemRoot", "ProgramFiles"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .filter(|value| !value.is_empty())
            .collect();

        if let Ok(system_root) = std::env::var("SystemRoot") {
            paths.push(format!("{system_root}\\System32"));
        }

        paths
    }

    #[cfg(not(windows))]
    {
        SENSITIVE_UNIX_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect()
    }
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

        // ` * ` e `* ` são da spec `pgpass-wildcard-validation`: o curinga é a string inteira.
        for invalid in [
            "",
            "   ",
            "host_com_underscore",
            "-leading.example",
            " * ",
            "* ",
            " *",
            "**",
        ] {
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

        // O curinga com espaço em volta é recusado (spec `pgpass-wildcard-validation`).
        for invalid in ["", "with space", "quotes'", " * ", "* ", &"a".repeat(64)] {
            assert!(!is_valid_username(invalid), "deveria recusar {invalid}");
        }
    }

    #[test]
    fn port_accepts_range_and_wildcard() {
        assert!(is_valid_port("1"));
        assert!(is_valid_port("5432"));
        assert!(is_valid_port("65535"));
        assert!(is_valid_port("*"));

        for invalid in ["0", "65536", "abc", "", " * ", "* ", "-"] {
            assert!(!is_valid_port(invalid), "deveria recusar {invalid}");
        }
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

    #[test]
    fn valid_dump_paths_are_accepted() {
        for valid in ["mydb.dump", "backups/mydb.dump", "C:\\dados\\mydb.dump"] {
            assert!(is_valid_path(valid), "deveria aceitar {valid}");
        }
    }

    #[test]
    fn path_traversal_patterns_are_refused() {
        for invalid in [
            "",
            "   ",
            "..",
            "../mydb.dump",
            "..\\mydb.dump",
            "backups/../../etc/mydb.dump",
            "%2e%2e%2fmydb.dump",
            "....//mydb.dump",
        ] {
            assert!(!is_valid_path(invalid), "deveria recusar {invalid:?}");
        }
    }

    #[test]
    fn a_null_byte_is_refused() {
        assert!(!is_valid_path("mydb\0.dump"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_folders_are_refused() {
        let windows = std::env::var("SystemRoot").expect("SystemRoot no ambiente");

        assert!(!is_valid_path(&format!("{windows}\\Temp\\mydb.dump")));
        // Prefixo simples, como no legado: sem fronteira de diretório.
        assert!(!is_valid_path(&format!("{windows}Update\\mydb.dump")));
        assert!(!is_valid_path("C:\\Windows\\System32\\mydb.dump"));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_system_folders_are_refused() {
        for invalid in ["/etc/mydb.dump", "/bin/mydb.dump", "/usr/bin/mydb.dump"] {
            assert!(!is_valid_path(invalid), "deveria recusar {invalid}");
        }
    }
}
