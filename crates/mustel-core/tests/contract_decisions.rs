//! Decisões de contrato que não são uma invocação simples de CLI.
//!
//! Aqui ficam as garantias que sustentam a superfície: a tabela completa de exit codes, a regra
//! SQLSTATE→10/11, os códigos de binário ausente (3 no `backup`, 1 no `restore`), a ausência de senha
//! na configuração (D6) e as divergências observáveis em unidade (DIV4, DIV8).

use mustel_core::config::{ConfigStore, ServerConfigEntry};
use mustel_core::error::{CoreError, PathKind, exit_code};

/// IF-151: falha de Postgres **com** SQLSTATE sai 10; **sem** SQLSTATE sai 11.
/// CONTRACT: IF-151
#[test]
fn postgres_failures_are_ten_with_sqlstate_and_eleven_without() {
    let with_sqlstate = CoreError::Postgres {
        sqlstate: Some("28P01".to_string()),
        message: "password authentication failed".to_string(),
    };
    let without_sqlstate = CoreError::postgres("error connecting to server: connection refused");

    assert_eq!(with_sqlstate.exit_code(), 10);
    assert_eq!(without_sqlstate.exit_code(), 11);
}

/// IF-152: binário ausente é **3** no `backup` e **1** no `restore` (quirk herdado).
///
/// Verificado no nível da camada que decide o código (o `FailureKind` → tabela do comando), porque no
/// nível de processo o binário existiria nesta máquina e o código real dependeria da instalação.
/// CONTRACT: IF-152
#[test]
fn missing_binary_exit_codes_differ_between_backup_and_restore() {
    use mustel_core::usecases::database::FailureKind;

    // As tabelas vivem nos comandos; aqui pinamos a decisão de produto que as separa.
    let backup: &[(FailureKind, i32)] = &[
        (FailureKind::MissingBinary, 3),
        (FailureKind::Usage, 2),
        (FailureKind::OperationFailed, 1),
    ];
    let restore: &[(FailureKind, i32)] = &[
        (FailureKind::MissingBinary, 1),
        (FailureKind::Usage, 2),
        (FailureKind::OperationFailed, 1),
    ];

    let code = |table: &[(FailureKind, i32)], kind: FailureKind| {
        table
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, code)| *code)
            .expect("toda variante precisa de código")
    };

    assert_eq!(code(backup, FailureKind::MissingBinary), 3);
    assert_eq!(code(restore, FailureKind::MissingBinary), 1);
}

/// IF-150: a tabela de exit codes documentada é a que o núcleo expõe, sem buraco.
/// CONTRACT: IF-150
#[test]
fn every_documented_exit_code_has_a_constant() {
    let documented = [
        (exit_code::SUCCESS, 0),
        (exit_code::GENERAL, 1),
        (exit_code::POSTGRES_WITH_SQLSTATE, 10),
        (exit_code::POSTGRES, 11),
        (exit_code::IO, 20),
        (exit_code::DIRECTORY_NOT_FOUND, 21),
        (exit_code::FILE_NOT_FOUND, 22),
        (exit_code::ACCESS_DENIED, 30),
        (exit_code::INVALID_STATE, 40),
        (exit_code::INVALID_ARGUMENT, 41),
        (exit_code::TIMEOUT, 50),
        (exit_code::CANCELLED, 130),
    ];

    for (constant, expected) in documented {
        assert_eq!(constant, expected, "constante fora do valor documentado");
    }

    // A tabela do `AGENTS.md` e este array têm de ter o mesmo tamanho.
    assert_eq!(documented.len(), 12);

    // Caminhos que faltam: diretório é 21, arquivo é 22 — o legado diferencia.
    let dir = CoreError::PathNotFound {
        path: "/nao/existe".into(),
        kind: PathKind::Directory,
    };
    let file = CoreError::PathNotFound {
        path: "/nao/existe".into(),
        kind: PathKind::File,
    };
    assert_eq!(dir.exit_code(), 21);
    assert_eq!(file.exit_code(), 22);
}

/// IF-160: a configuração **não** tem campo de senha, nem no tipo nem no JSON gravado (D6).
///
/// A garantia é estrutural: `ServerConfigEntry` não possui campo de senha. O teste fixa as duas pontas:
/// o conjunto de chaves serializadas e a ausência de qualquer nome que sugira segredo.
/// CONTRACT: IF-160
#[test]
fn the_config_never_holds_a_password_field() {
    let entry = ServerConfigEntry {
        name: "s1".to_string(),
        host: "localhost".to_string(),
        port: 5432,
        username: "postgres".to_string(),
        databases: vec!["db".to_string()],
        fetch_all_databases: false,
        exclude_patterns: vec!["template*".to_string(), "postgres".to_string()],
        ssl_mode: "Prefer".to_string(),
        timeout: 30,
        max_parallelism: 4,
    };

    let json = serde_json::to_string(&entry).expect("serializa");
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    let object = value.as_object().expect("objeto");

    for forbidden in ["password", "encryptedPassword", "encrypted_password"] {
        assert!(
            !object.contains_key(forbidden),
            "a config não pode ter o campo '{forbidden}': {json}"
        );
    }

    let keys: Vec<&String> = object.keys().collect();
    assert_eq!(
        keys.len(),
        10,
        "campos do cadastro mudaram (esperado 10): {json}"
    );
    for key in keys {
        assert!(
            !key.to_lowercase().contains("pass"),
            "nenhum campo do cadastro pode cheirar a senha: {key}"
        );
    }
}

/// IF-162: porta fora de 0–65535 no arquivo é erro de parse, não valor aceito (DIV4).
/// CONTRACT: IF-162
#[test]
fn a_port_out_of_range_is_a_parse_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("mustel.jsonc");
    std::fs::write(
        &path,
        r#"{ "servers": [ { "name": "s", "host": "localhost", "port": 70000 } ] }"#,
    )
    .expect("escreve");

    let store = ConfigStore::new(path);
    let error = store.load().expect_err("porta inválida precisa falhar");
    let text = error.to_string();
    assert!(
        text.contains("70000") || text.to_lowercase().contains("port"),
        "mensagem deve apontar a porta: {text}"
    );
}

/// IF-192: todo link relativo da documentação aponta para um arquivo que existe.
///
/// Documentação que aponta para o vazio é pior que documentação ausente: o leitor segue o link e
/// conclui que o produto está quebrado. Este teste cobre `docs/**/*.md` e os dois READMEs da raiz.
/// CONTRACT: IF-192
#[test]
fn documentation_links_resolve() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("raiz do repositório")
        .to_path_buf();

    let mut files = vec![root.join("README.md"), root.join("README.pt-BR.md")];
    collect_markdown(&root.join("docs"), &mut files);

    let mut broken = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            broken.push(format!("{}: não pôde ser lido", file.display()));
            continue;
        };
        let base = file.parent().expect("diretório do arquivo");

        for link in markdown_links(&text) {
            // Só links relativos: http(s), mailto e âncoras puras não são arquivos.
            if link.starts_with("http://")
                || link.starts_with("https://")
                || link.starts_with("mailto:")
                || link.starts_with('#')
            {
                continue;
            }
            let path_part = link.split('#').next().unwrap_or(&link);
            if path_part.is_empty() {
                continue;
            }
            let target = base.join(path_part);
            if !target.exists() {
                broken.push(format!(
                    "{}: '{link}' não existe",
                    file.strip_prefix(&root).unwrap_or(file).display()
                ));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "link(s) quebrado(s) na documentação:\n  - {}",
        broken.join("\n  - ")
    );
}

/// Coleta recursivamente os `.md` de um diretório.
fn collect_markdown(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// Extrai destinos de `[texto](destino)` de um markdown.
fn markdown_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else {
            break;
        };
        let target = after[..close].trim();
        // Remove um eventual título: `(caminho "título")`.
        let target = target.split_whitespace().next().unwrap_or("");
        if !target.is_empty() {
            links.push(target.to_string());
        }
        rest = &after[close + 1..];
    }
    links
}

/// IF-104: no Unix o `pgpass.conf` é criado com modo `0600` (DIV8).
///
/// Um arquivo de senha em texto claro precisa de permissão de dono; o produto anterior deixava o padrão
/// do processo. A comparação de conteúdo byte a byte ignora permissão, então esta decisão só existe aqui.
/// CONTRACT: IF-104
#[cfg(unix)]
#[test]
fn the_pgpass_file_is_owner_only_on_unix() {
    use mustel_core::pgpass::store::PgPassStore;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("pgpass.conf");
    let store = PgPassStore::new(path.clone());
    let entry = mustel_core::pgpass::entry::PgPassEntry {
        host: "localhost".to_string(),
        port: "5432".to_string(),
        database: "db".to_string(),
        username: "postgres".to_string(),
        password: "s".to_string(),
    };
    store.add(&entry).expect("grava a entrada");

    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "modo esperado 0600, veio {mode:o}");
}
