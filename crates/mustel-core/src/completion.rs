//! Motor de sugestão do editor de SQL.
//!
//! Função **pura**: recebe o texto, a posição do cursor e o [`Schema`] já em memória, e devolve os
//! candidatos. Não lê banco, não desenha, não conhece egui — é o que torna o miolo testável com um
//! esquema fictício.
//!
//! As regras vêm do plano (`MustelPlans/autocomplete-sql.md`, §4.4):
//!
//! 1. a palavra sob o cursor define o prefixo;
//! 2. a **última palavra-chave** define o contexto (tabela × coluna × palavra-chave);
//! 3. `alias.` resolve para as colunas da tabela daquele alias;
//! 4. nome de tabela sai simples quando é inequívoco e **com o schema** quando há homônimos;
//! 5. CTEs da própria consulta são sugeridas em `FROM`/`JOIN` (as colunas delas não);
//! 6. dentro de string ou comentário não se sugere nada.

use crate::usecases::schema::{Schema, Table};

/// Um item sugerido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Texto que substitui a palavra sob o cursor.
    pub insert: String,
    /// Detalhe mostrado à direita no popup (tabela de origem, tipo da coluna, assinatura).
    pub detail: Option<String>,
}

impl Candidate {
    /// Candidato sem detalhe.
    pub fn plain(insert: impl Into<String>) -> Self {
        Self {
            insert: insert.into(),
            detail: None,
        }
    }

    /// Candidato com detalhe.
    pub fn with_detail(insert: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            insert: insert.into(),
            detail: Some(detail.into()),
        }
    }
}

/// Quantos candidatos o popup recebe no máximo.
pub const MAX_CANDIDATES: usize = 50;

/// Palavras-chave oferecidas quando não há contexto melhor.
///
/// Lista curta, a mesma do realce de sintaxe: sugerir a gramática inteira do Postgres encheria o popup
/// sem ajudar.
const KEYWORDS: &[&str] = &[
    "select", "from", "where", "join", "left", "right", "inner", "outer", "group", "order", "by",
    "having", "limit", "offset", "insert", "into", "values", "update", "set", "delete", "create",
    "table", "view", "with", "as", "on", "and", "or", "not", "null", "distinct", "count", "sum",
    "avg", "min", "max", "case", "when", "then", "else", "end",
];

/// Em que contexto o cursor está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    /// Esperando uma tabela (`FROM`, `JOIN`, `INTO`, `UPDATE`, `TABLE`).
    Relation,
    /// Esperando coluna ou função (`SELECT`, `WHERE`, `ON`, `GROUP`, `HAVING`, `ORDER`, `SET`).
    Column,
    /// Início da consulta ou contexto desconhecido.
    Any,
}

/// Sugere candidatos para o texto e a posição do cursor (em **caracteres**, não bytes).
///
/// `None` quando o cursor está dentro de string ou comentário — ali sugestão é ruído.
pub fn suggest(text: &str, cursor: usize, schema: &Schema) -> Vec<Candidate> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());

    if scan_literal_or_comment(&chars, cursor) {
        return Vec::new();
    }

    // Palavra sob o cursor: para trás até um separador, e para frente idem (o popup substitui a palavra
    // inteira, então o sufixo também conta).
    let start = (0..cursor)
        .rev()
        .take_while(|index| is_word_char(chars[*index]))
        .last()
        .unwrap_or(cursor);
    let end = (cursor..chars.len())
        .take_while(|index| is_word_char(chars[*index]))
        .last()
        .map(|index| index + 1)
        .unwrap_or(cursor);

    let word: String = chars[start..end].iter().collect();

    // `alias.` ou `schema.`: o prefixo antes do ponto manda no que sugerir.
    if let Some((qualifier, rest)) = word.split_once('.') {
        // O texto inteiro: as tabelas da consulta podem estar **depois** do cursor
        // (`SELECT c. FROM clientes c`).
        return qualified(qualifier, rest, &chars, schema);
    }

    // Contexto é o que **precede** o cursor; referências são a consulta inteira.
    let context = context_before(&chars[..start]);
    let mut candidates = collect(context, &chars, schema);

    rank(&mut candidates, &word);
    candidates.truncate(MAX_CANDIDATES);
    candidates
}

/// Candidatos quando a palavra tem um qualificador antes do ponto.
fn qualified(qualifier: &str, prefix: &str, before: &[char], schema: &Schema) -> Vec<Candidate> {
    // 1. Alias da própria consulta → colunas daquela tabela.
    if let Some(table_name) = alias_target(before, qualifier)
        && let Some(table) = find_table(schema, &table_name)
    {
        let mut candidates: Vec<Candidate> = table
            .columns
            .iter()
            .filter(|column| starts_with_ignore_case(&column.name, prefix))
            .map(|column| Candidate::with_detail(column.name.clone(), column.type_name.clone()))
            .collect();
        rank(&mut candidates, prefix);
        candidates.truncate(MAX_CANDIDATES);
        return candidates;
    }

    // 2. Schema digitado → tabelas daquele schema, já qualificadas.
    let mut candidates: Vec<Candidate> = schema
        .tables
        .iter()
        .filter(|table| table.schema.eq_ignore_ascii_case(qualifier))
        .filter(|table| starts_with_ignore_case(&table.name, prefix))
        .map(|table| {
            Candidate::with_detail(
                format!("{}.{}", table.schema, table.name),
                columns_count(table),
            )
        })
        .collect();

    rank(&mut candidates, prefix);
    candidates.truncate(MAX_CANDIDATES);
    candidates
}

/// Coleta os candidatos do contexto.
fn collect(context: Context, before: &[char], schema: &Schema) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    if context != Context::Column {
        candidates.extend(table_candidates(schema, before));
        candidates.extend(cte_candidates(before));
    }

    if context != Context::Relation {
        candidates.extend(column_candidates(schema, before));
        candidates.extend(function_candidates(schema));
    }

    if context == Context::Any {
        candidates.extend(KEYWORDS.iter().map(|keyword| Candidate::plain(*keyword)));
    }

    candidates
}

/// Tabelas e views do esquema, com o nome simples ou qualificado (regra do homônimo).
fn table_candidates(schema: &Schema, _before: &[char]) -> Vec<Candidate> {
    schema
        .tables
        .iter()
        .map(|table| {
            let ambiguous = schema
                .tables
                .iter()
                .filter(|other| other.name.eq_ignore_ascii_case(&table.name))
                .count()
                > 1;

            if ambiguous {
                Candidate::with_detail(
                    format!("{}.{}", table.schema, table.name),
                    columns_count(table),
                )
            } else {
                Candidate::with_detail(table.name.clone(), columns_count(table))
            }
        })
        .collect()
}

/// Colunas das tabelas citadas na consulta (pelo nome ou pelo alias).
fn column_candidates(schema: &Schema, before: &[char]) -> Vec<Candidate> {
    let referenced = referenced_tables(before);
    let mut candidates = Vec::new();

    let tables: Vec<&Table> = if referenced.is_empty() {
        // Sem `FROM` ainda: todas as colunas seriam demais. Nada de coluna neste caso.
        Vec::new()
    } else {
        referenced
            .iter()
            .filter_map(|name| find_table(schema, name))
            .collect()
    };

    for table in tables {
        for column in &table.columns {
            candidates.push(Candidate::with_detail(
                column.name.clone(),
                format!("{}.{} ({})", table.name, column.name, column.type_name),
            ));
        }
    }

    // Duas tabelas com a mesma coluna: fica uma entrada só (o detalhe já diz a origem).
    dedupe(&mut candidates);
    candidates
}

/// Funções do esquema, com a assinatura no detalhe.
fn function_candidates(schema: &Schema) -> Vec<Candidate> {
    schema
        .functions
        .iter()
        .map(|function| {
            let insert = format!("{}(", function.name);
            match &function.arguments {
                Some(arguments) => Candidate::with_detail(insert, arguments.clone()),
                None => Candidate::plain(insert),
            }
        })
        .collect()
}

/// Nomes das CTEs declaradas na consulta (`WITH nome AS`, inclusive várias separadas por vírgula).
fn cte_candidates(before: &[char]) -> Vec<Candidate> {
    let tokens = words(before);
    let mut names = Vec::new();
    let mut inside_with = false;
    let mut depth = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        if token.eq_ignore_ascii_case("with") {
            inside_with = true;
            depth = 0;
            continue;
        }
        if !inside_with {
            continue;
        }

        match token.as_str() {
            "(" => depth += 1,
            ")" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    inside_with = false;
                }
            }
            "," if depth == 0 => {}
            _ => {
                // `<nome> AS` no nível zero é declaração de CTE.
                if depth == 0
                    && tokens
                        .get(index + 1)
                        .is_some_and(|next| next.eq_ignore_ascii_case("as"))
                    && is_identifier(token)
                {
                    names.push(Candidate::with_detail(token.clone(), "CTE"));
                }
            }
        }
    }

    names
}

/// Tabelas citadas na consulta: nomes depois de `FROM`/`JOIN` e os aliases declarados.
fn referenced_tables(before: &[char]) -> Vec<String> {
    let tokens = words(before);
    let mut names: Vec<String> = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if !(token.eq_ignore_ascii_case("from") || token.eq_ignore_ascii_case("join")) {
            continue;
        }

        // Pode haver lista: `FROM a, b`.
        let mut cursor = index + 1;
        while let Some(name) = tokens.get(cursor) {
            if !is_identifier(name) {
                break;
            }
            names.push(unqualified(name).to_string());

            // `AS alias` ou alias direto.
            let mut next = cursor + 1;
            if tokens
                .get(next)
                .is_some_and(|t| t.eq_ignore_ascii_case("as"))
            {
                next += 1;
            }
            if let Some(alias) = tokens.get(next)
                && is_identifier(alias)
                && !is_clause_keyword(alias)
            {
                names.push(unqualified(alias).to_string());
                cursor = next;
            }

            // Continua só se vier vírgula.
            match tokens.get(cursor + 1) {
                Some(next) if next == "," => cursor += 2,
                _ => break,
            }
        }
    }

    names
}

/// Tabela de um alias declarado na consulta.
fn alias_target(before: &[char], alias: &str) -> Option<String> {
    let tokens = words(before);

    for (index, token) in tokens.iter().enumerate() {
        if !(token.eq_ignore_ascii_case("from") || token.eq_ignore_ascii_case("join")) {
            continue;
        }
        let Some(table) = tokens.get(index + 1) else {
            continue;
        };
        let mut next = index + 2;
        if tokens
            .get(next)
            .is_some_and(|t| t.eq_ignore_ascii_case("as"))
        {
            next += 1;
        }
        if let Some(candidate) = tokens.get(next)
            && candidate.eq_ignore_ascii_case(alias)
        {
            return Some(unqualified(table).to_string());
        }
    }

    None
}

/// Contexto a partir da última palavra-chave antes do cursor.
fn context_before(before: &[char]) -> Context {
    let tokens = words(before);

    for token in tokens.iter().rev() {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "from" | "join" | "into" | "update" | "table" => return Context::Relation,
            "select" | "where" | "on" | "group" | "having" | "order" | "set" | "by" => {
                return Context::Column;
            }
            // Fim de uma expressão: continua procurando.
            _ => {}
        }
    }

    Context::Any
}

/// O cursor cai dentro de string ou comentário?
///
/// Varre caractere a caractere — é o que trata corretamente string/comentário **não terminados**
/// (uma aspa aberta consome o resto) e a aspa escapada (`''`). É a **única** implementação dessa
/// regra: o editor da GUI delega para cá em vez de reimplementar.
pub fn cursor_in_literal_or_comment(text: &str, cursor: usize) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    scan_literal_or_comment(&chars, cursor)
}

/// Implementação da varredura, sobre `char` (o motor já tem os caracteres em mãos).
fn scan_literal_or_comment(chars: &[char], cursor: usize) -> bool {
    let mut index = 0;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while index < cursor {
        let current = chars[index];
        let next = chars.get(index + 1).copied();

        if in_line_comment {
            if current == '\n' {
                in_line_comment = false;
            }
        } else if in_block_comment {
            if current == '*' && next == Some('/') {
                in_block_comment = false;
                index += 1;
            }
        } else if in_string {
            if current == '\'' {
                if next == Some('\'') {
                    index += 1;
                } else {
                    in_string = false;
                }
            }
        } else if current == '-' && next == Some('-') {
            in_line_comment = true;
            index += 1;
        } else if current == '/' && next == Some('*') {
            in_block_comment = true;
            index += 1;
        } else if current == '\'' {
            in_string = true;
        }

        index += 1;
    }

    in_string || in_line_comment || in_block_comment
}

/// Palavras e pontuação, na ordem em que aparecem.
fn words(chars: &[char]) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for &character in chars {
        if is_word_char(character) {
            current.push(character);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if !character.is_whitespace() {
                tokens.push(character.to_string());
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '.' || character == '"'
}

/// Caracteres que **separam** palavras no editor.
///
/// É o predicado que o `CompletionPopup` do egui espera em `word_boundary`: ele pergunta "este
/// caractere encerra a palavra?". O motor pergunta o contrário ([`is_word_char`]), então a regra
/// mora aqui, uma vez só — se as duas divergirem, o popup substitui um trecho diferente do que o
/// motor leu (foi assim que o texto aceito grudou no anterior: `select` virou `selectnome`).
///
/// `.` e `"` **não** separam: `c.` e `"Nome Estranho"` precisam chegar inteiros ao motor.
pub fn is_word_boundary(character: char) -> bool {
    !is_word_char(character)
}

fn is_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '"')
}

fn is_clause_keyword(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "where"
            | "group"
            | "order"
            | "having"
            | "limit"
            | "offset"
            | "join"
            | "inner"
            | "left"
            | "right"
            | "full"
            | "cross"
            | "on"
            | "union"
            | "except"
            | "intersect"
            | "set"
    )
}

/// Nome sem o schema (`vendas.clientes` → `clientes`).
fn unqualified(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn find_table<'a>(schema: &'a Schema, name: &str) -> Option<&'a Table> {
    let name = unqualified(name);
    schema
        .tables
        .iter()
        .find(|table| table.name.eq_ignore_ascii_case(name))
}

fn starts_with_ignore_case(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn columns_count(table: &Table) -> String {
    match table.columns.len() {
        1 => "1 coluna".to_string(),
        count => format!("{count} colunas"),
    }
}

fn dedupe(candidates: &mut Vec<Candidate>) {
    let mut seen: Vec<String> = Vec::new();
    candidates.retain(|candidate| {
        if seen.contains(&candidate.insert) {
            false
        } else {
            seen.push(candidate.insert.clone());
            true
        }
    });
}

/// Ordena por qualidade do casamento: igual, prefixo, contém.
fn rank(candidates: &mut Vec<Candidate>, word: &str) {
    let word = word.to_ascii_lowercase();

    candidates.sort_by_key(|candidate| {
        let insert = candidate.insert.to_ascii_lowercase();
        if insert == word {
            0
        } else if insert.starts_with(&word) {
            1
        } else if insert.contains(&word) {
            2
        } else {
            3
        }
    });

    candidates.retain(|candidate| {
        word.is_empty() || candidate.insert.to_ascii_lowercase().contains(&word)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usecases::schema::{Column, Function, RelationKind, Schema, Table};

    /// Esquema de teste: duas tabelas simples e uma homônima em dois schemas.
    fn schema() -> Schema {
        Schema {
            tables: vec![
                Table {
                    schema: "public".to_string(),
                    name: "clientes".to_string(),
                    kind: RelationKind::Table,
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            type_name: "integer".to_string(),
                        },
                        Column {
                            name: "nome".to_string(),
                            type_name: "text".to_string(),
                        },
                        Column {
                            name: "total_pago".to_string(),
                            type_name: "numeric".to_string(),
                        },
                    ],
                },
                Table {
                    schema: "public".to_string(),
                    name: "pedidos".to_string(),
                    kind: RelationKind::Table,
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            type_name: "integer".to_string(),
                        },
                        Column {
                            name: "cliente_id".to_string(),
                            type_name: "integer".to_string(),
                        },
                    ],
                },
                Table {
                    schema: "vendas".to_string(),
                    name: "clientes".to_string(),
                    kind: RelationKind::Table,
                    columns: vec![Column {
                        name: "id".to_string(),
                        type_name: "bigint".to_string(),
                    }],
                },
                Table {
                    schema: "public".to_string(),
                    name: "resumo".to_string(),
                    kind: RelationKind::View,
                    columns: vec![Column {
                        name: "total".to_string(),
                        type_name: "numeric".to_string(),
                    }],
                },
            ],
            functions: vec![Function {
                schema: "public".to_string(),
                name: "substr".to_string(),
                arguments: Some("text, integer".to_string()),
            }],
        }
    }

    fn inserts(candidates: &[Candidate]) -> Vec<String> {
        candidates
            .iter()
            .map(|candidate| candidate.insert.clone())
            .collect()
    }

    /// `FROM ` sugere tabelas e views.
    ///
    /// `clientes` existe em dois schemas neste esquema fictício, então sai **qualificado** (D16);
    /// `pedidos` e `resumo` são únicos e saem simples.
    /// CONTRACT: IF-205
    #[test]
    fn from_suggests_relations() {
        let candidates = suggest("SELECT * FROM ", 14, &schema());
        let names = inserts(&candidates);
        assert!(names.contains(&"public.clientes".to_string()), "{names:?}");
        assert!(names.contains(&"pedidos".to_string()), "{names:?}");
        assert!(
            names.contains(&"resumo".to_string()),
            "view também: {names:?}"
        );
        assert!(
            !names.contains(&"substr(".to_string()),
            "função não é relação: {names:?}"
        );
        assert!(
            !names.contains(&"select".to_string()),
            "em FROM não se sugere palavra-chave: {names:?}"
        );
    }

    /// Nome homônimo em dois schemas sai qualificado; nome único sai simples.
    /// CONTRACT: IF-205
    #[test]
    fn duplicate_table_names_are_qualified() {
        let names = inserts(&suggest("SELECT * FROM ", 14, &schema()));
        assert!(
            names.contains(&"public.clientes".to_string()),
            "homônimo precisa do schema: {names:?}"
        );
        assert!(
            names.contains(&"vendas.clientes".to_string()),
            "os dois lados aparecem: {names:?}"
        );
        assert!(
            names.contains(&"pedidos".to_string()),
            "nome único fica simples: {names:?}"
        );
    }

    /// Digitando `schema.` só aparecem as tabelas daquele schema.
    /// CONTRACT: IF-205
    #[test]
    fn typing_a_schema_filters_its_tables() {
        let names = inserts(&suggest("SELECT * FROM vendas.", 21, &schema()));
        assert_eq!(names, vec!["vendas.clientes".to_string()]);
    }

    /// `SELECT ` sugere colunas das tabelas citadas.
    #[test]
    fn select_suggests_columns_of_the_referenced_tables() {
        let sql = "SELECT  FROM clientes";
        let names = inserts(&suggest(sql, 7, &schema()));
        assert!(names.contains(&"nome".to_string()), "{names:?}");
        assert!(names.contains(&"total_pago".to_string()), "{names:?}");
        assert!(
            !names.contains(&"cliente_id".to_string()),
            "coluna de outra tabela não entra: {names:?}"
        );
    }

    /// `c.` resolve pelo alias declarado no `FROM`.
    /// CONTRACT: IF-206
    #[test]
    fn an_alias_resolves_to_the_table_columns() {
        let sql = "SELECT c. FROM clientes c";
        let candidates = suggest(sql, 9, &schema());
        let names = inserts(&candidates);
        assert!(names.contains(&"nome".to_string()), "{names:?}");
        assert!(names.contains(&"total_pago".to_string()), "{names:?}");
        assert!(
            !names.contains(&"cliente_id".to_string()),
            "não vaza coluna de pedidos: {names:?}"
        );
        let detail = candidates
            .iter()
            .find(|candidate| candidate.insert == "nome")
            .and_then(|candidate| candidate.detail.clone())
            .unwrap_or_default();
        assert!(detail.contains("text"), "o detalhe traz o tipo: {detail}");
    }

    /// Funções aparecem com a assinatura no detalhe.
    /// CONTRACT: IF-207
    #[test]
    fn functions_come_with_their_signature() {
        let sql = "SELECT sub FROM t";
        let candidates = suggest(sql, 10, &schema());
        let function = candidates
            .iter()
            .find(|candidate| candidate.insert == "substr(")
            .expect("função sugerida");
        assert_eq!(function.detail.as_deref(), Some("text, integer"));
    }

    /// CTE declarada na consulta é sugerida em `FROM`.
    #[test]
    fn ctes_are_suggested_in_from() {
        let sql = "WITH totais AS (SELECT 1) SELECT * FROM tot";
        let names = inserts(&suggest(sql, sql.chars().count(), &schema()));
        assert!(names.contains(&"totais".to_string()), "{names:?}");
    }

    /// Dentro de string ou comentário não se sugere nada.
    #[test]
    fn nothing_is_suggested_inside_literals_or_comments() {
        let sql = "SELECT 'cli";
        assert!(suggest(sql, sql.chars().count(), &schema()).is_empty());

        let sql = "SELECT 1 -- cli";
        assert!(suggest(sql, sql.chars().count(), &schema()).is_empty());

        let sql = "SELECT /* cli";
        assert!(suggest(sql, sql.chars().count(), &schema()).is_empty());
    }

    /// Prefixo ganha de "contém" no ranking.
    /// CONTRACT: IF-208
    #[test]
    fn ranking_puts_prefix_before_substring() {
        let names = inserts(&suggest("SELECT * FROM cli", 16, &schema()));
        let first = names.first().expect("tem candidato");
        assert!(
            first.to_ascii_lowercase().contains("clientes"),
            "o prefixo vem primeiro: {names:?}"
        );
    }

    /// Respeita o teto de candidatos.
    /// CONTRACT: IF-208
    #[test]
    fn the_candidate_list_is_capped() {
        let mut big = Schema::default();
        for index in 0..200 {
            big.tables.push(Table {
                schema: "public".to_string(),
                name: format!("tabela_{index}"),
                kind: RelationKind::Table,
                columns: Vec::new(),
            });
        }

        let candidates = suggest("SELECT * FROM ", 14, &big);
        assert_eq!(candidates.len(), MAX_CANDIDATES);
    }

    /// Em comentário de bloco fechado, a sugestão volta a funcionar depois dele.
    #[test]
    fn a_closed_block_comment_does_not_disable_suggestions() {
        let sql = "SELECT /* nada */ * FROM ";
        let names = inserts(&suggest(sql, sql.chars().count(), &schema()));
        assert!(names.contains(&"pedidos".to_string()), "{names:?}");
    }

    /// IF-209: a fronteira de palavra que o popup usa é o **complemento** da que o motor usa.
    ///
    /// É a regra que faltava: o popup pergunta "isto separa palavras?", o motor pergunta "isto faz
    /// parte da palavra?". Invertido, o popup tratava o identificador como se fosse separador — a
    /// palavra virava o espaço anterior, nada abria ao digitar e o texto aceito grudava no anterior.
    /// CONTRACT: IF-209
    #[test]
    fn the_popup_word_boundary_is_the_complement_of_the_engine_one() {
        for character in ['a', 'Z', '7', '_', '.', '"'] {
            assert!(
                !is_word_boundary(character),
                "{character:?} faz parte da palavra"
            );
        }

        for character in [' ', '\t', '\n', ',', '(', ')', '*', '\'', ';'] {
            assert!(is_word_boundary(character), "{character:?} separa palavras");
        }
    }

    /// IF-204: a regra de string/comentário é uma só, e os limites são exatos — o espaço logo depois
    /// do fecha-aspas está **fora**; a aspa escapada não encerra a string.
    /// CONTRACT: IF-204
    #[test]
    fn the_literal_and_comment_boundary_is_exact() {
        let sql = "SELECT 'abc' -- nota
FROM t";
        assert!(
            cursor_in_literal_or_comment(sql, 9),
            "dentro da string (posição 9 = 'b')"
        );
        assert!(
            !cursor_in_literal_or_comment(sql, 12),
            "posição 12 é o espaço depois do fecha-aspas: fora"
        );
        assert!(
            cursor_in_literal_or_comment(sql, 15),
            "posição 15 está dentro do comentário de linha"
        );
        assert!(
            !cursor_in_literal_or_comment(sql, sql.chars().count()),
            "fim do texto está fora"
        );

        let block = "SELECT /* x */ 1";
        assert!(cursor_in_literal_or_comment(block, 9), "dentro do bloco");
        assert!(!cursor_in_literal_or_comment(block, 14), "depois do bloco");

        let escaped = "SELECT 'it''s' FROM t";
        assert!(
            cursor_in_literal_or_comment(escaped, 13),
            "a aspa escapada mantém a string aberta"
        );
        assert!(
            !cursor_in_literal_or_comment(escaped, 15),
            "depois do fecha-aspas real"
        );

        // String não terminada consome o resto: digitar depois continua "dentro".
        let open = "SELECT 'abc";
        assert!(cursor_in_literal_or_comment(open, open.chars().count()));
    }
}
