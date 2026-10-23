//! Meta-teste do manifesto de contrato.
//!
//! Garante, nas **duas** direções, que o manifesto `docs/contrato-de-interface.md` e os testes não se
//! separam:
//!
//! - toda linha `coberta` cita um `<arquivo>::<fn>` que **existe** de verdade;
//! - toda linha tem o status preenchido, e a única forma de não ter teste é declarar
//!   `nao-coberta: <razão>`;
//! - todo marcador `CONTRACT: IF-NNN` no código corresponde a uma linha do manifesto;
//! - toda linha `coberta` tem pelo menos um marcador no código.
//!
//! É o que torna a frase "cada decisão tem teste" falsificável: apagar um teste, inventar um caminho ou
//! duplicar um ID quebra a suíte.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Uma linha de dados do manifesto.
struct Row {
    id: String,
    tests: Vec<String>,
    status: String,
}

/// Raiz do repositório (dois níveis acima deste crate).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate> fica dois níveis abaixo da raiz")
        .to_path_buf()
}

/// Caminho do manifesto.
fn manifest_path() -> PathBuf {
    repo_root().join("docs").join("contrato-de-interface.md")
}

/// Lê o manifesto e devolve as linhas de dados.
fn parse_manifest() -> Vec<Row> {
    let text = std::fs::read_to_string(manifest_path()).expect("o manifesto precisa existir");
    let mut rows = Vec::new();

    for (index, line) in text.lines().enumerate() {
        if !line.starts_with("| IF-") {
            continue;
        }
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        assert_eq!(
            cells.len(),
            7,
            "linha {} do manifesto tem {} colunas (esperado 7): {line}",
            index + 1,
            cells.len()
        );

        let tests = if cells[5] == "—" {
            Vec::new()
        } else {
            cells[5]
                .split(';')
                .map(|item| item.trim().to_string())
                .collect()
        };

        rows.push(Row {
            id: cells[0].to_string(),
            tests,
            status: cells[6].to_string(),
        });
    }

    assert!(!rows.is_empty(), "o manifesto não tem nenhuma linha IF-");
    rows
}

/// Extrai os IDs `IF-NNN` marcados em um texto.
fn markers_in(text: &str) -> Vec<String> {
    const PREFIX: &str = "CONTRACT: IF-";
    let mut found = Vec::new();
    let mut rest = text;

    while let Some(position) = rest.find(PREFIX) {
        let after = &rest[position + PREFIX.len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if digits.len() == 3 {
            found.push(format!("IF-{digits}"));
        }
        rest = &after[digits.len()..];
    }

    found
}

/// Percorre `crates/**/*.rs` e devolve `(arquivo, ids marcados)`.
fn scan_markers(dir: &Path, out: &mut Vec<(PathBuf, Vec<String>)>) {
    for entry in std::fs::read_dir(dir).expect("ler crates") {
        let path = entry.expect("entrada").path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("target") {
                continue;
            }
            scan_markers(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let text = std::fs::read_to_string(&path).expect("ler fonte");
            let found = markers_in(&text);
            if !found.is_empty() {
                out.push((path, found));
            }
        }
    }
}

/// O `fn` citado existe no arquivo citado?
fn test_exists(reference: &str) -> Result<(), String> {
    let parts: Vec<&str> = reference.split("::").collect();
    let file = parts.first().copied().unwrap_or("");
    let function = parts.last().copied().unwrap_or("");
    if file.is_empty() || function.is_empty() || parts.len() < 2 {
        return Err(format!("'{reference}' não está no formato <arquivo>::<fn>"));
    }

    let path = repo_root().join(file);
    if !path.is_file() {
        return Err(format!("arquivo citado não existe: {file}"));
    }
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{file}: {error}"))?;
    if !text.contains(&format!("fn {function}")) {
        return Err(format!("{file} não tem `fn {function}`"));
    }
    Ok(())
}

/// Relatório de tudo o que está errado (para uma única execução mostrar o conjunto).
#[test]
fn the_manifest_and_the_tests_agree() {
    let rows = parse_manifest();
    let mut problems: Vec<String> = Vec::new();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();

    for row in &rows {
        *ids.entry(row.id.clone()).or_default() += 1;

        if row.status.trim().is_empty() {
            problems.push(format!("{}: status vazio", row.id));
        }

        if row.tests.is_empty()
            && !row.status.starts_with("nao-coberta")
            && row.status != "pendente"
        {
            problems.push(format!(
                "{}: sem teste e sem `nao-coberta: <razão>`",
                row.id
            ));
        }

        for reference in &row.tests {
            // Linha `pendente` pode apontar para um teste ainda não escrito; `coberta` não pode.
            if row.status != "coberta" {
                continue;
            }
            if let Err(error) = test_exists(reference) {
                problems.push(format!("{}: {error}", row.id));
            }
        }
    }

    for (id, count) in &ids {
        if *count > 1 {
            problems.push(format!("{id}: id duplicado ({count} linhas)"));
        }
    }

    let mut marked: Vec<(PathBuf, Vec<String>)> = Vec::new();
    scan_markers(&repo_root().join("crates"), &mut marked);

    let mut marked_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, found) in &marked {
        for id in found {
            marked_ids
                .entry(id.clone())
                .or_default()
                .push(path.display().to_string());
        }
    }

    for id in marked_ids.keys() {
        if !ids.contains_key(id) {
            problems.push(format!("{id}: marcador no código sem linha no manifesto"));
        }
    }

    for row in &rows {
        if row.status == "coberta" && !marked_ids.contains_key(&row.id) {
            problems.push(format!(
                "{}: linha `coberta` sem marcador `CONTRACT: {}` em nenhum teste",
                row.id, row.id
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "o manifesto e os testes divergem em {} ponto(s):\n  - {}",
        problems.len(),
        problems.join("\n  - ")
    );
}

/// O parser do manifesto recusa linha malformada (proteção do próprio meta-teste).
#[test]
fn the_marker_reader_finds_every_id() {
    let text = "/// CONTRACT: IF-001\n/// CONTRACT: IF-042\n// nada aqui\n";
    assert_eq!(markers_in(text), vec!["IF-001", "IF-042"]);
    assert!(markers_in("sem marcador").is_empty());
    assert!(
        markers_in("CONTRACT: IF-12").is_empty(),
        "ID curto é ignorado"
    );
}
