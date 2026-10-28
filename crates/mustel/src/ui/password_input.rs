//! Leitura de senha por uma linha do stdin (`--password-stdin`).
//!
//! A senha não entra em `argv`, no histórico do shell nem na lista de processos. O valor é lido
//! como veio, menos o `\r\n`/`\n` final; uma linha vazia é erro de argumento.

use std::io::BufRead;

use mustel_core::error::CoreError;

/// Lê a senha do stdin do processo.
pub fn read_from_stdin() -> Result<String, CoreError> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    read_password(&mut input)
}

/// Lê uma linha de um `BufRead` (testável sem terminal).
pub fn read_password(input: &mut impl BufRead) -> Result<String, CoreError> {
    let mut line = String::new();
    input.read_line(&mut line).map_err(CoreError::Io)?;

    let password = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .or_else(|| line.strip_suffix('\r'))
        .unwrap_or(&line)
        .to_string();

    if password.is_empty() {
        return Err(CoreError::InvalidArgument(
            "password read from stdin is empty".to_string(),
        ));
    }

    Ok(password)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn read(input: &str) -> Result<String, CoreError> {
        read_password(&mut Cursor::new(input))
    }

    #[test]
    fn removes_crlf_lf_and_a_plain_carriage_return() {
        assert_eq!(read("segredo\r\n").expect("CRLF"), "segredo");
        assert_eq!(read("segredo\n").expect("LF"), "segredo");
        assert_eq!(read("segredo\r").expect("CR"), "segredo");
        assert_eq!(read("segredo").expect("sem newline"), "segredo");
    }

    #[test]
    fn preserves_spaces_and_colons() {
        assert_eq!(
            read("  senha:com:dois  \r\n").expect("senha"),
            "  senha:com:dois  "
        );
    }

    #[test]
    fn an_empty_line_is_an_argument_error() {
        for input in ["", "\n", "\r\n"] {
            let error = read(input).expect_err("deve recusar");
            assert!(matches!(error, CoreError::InvalidArgument(_)), "{error:?}");
            assert!(error.to_string().contains("stdin"), "{error}");
        }
    }
}
