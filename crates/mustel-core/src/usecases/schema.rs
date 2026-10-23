//! Leitura do **esquema** de um banco, para o autocomplete da GUI.
//!
//! Uma passada no catálogo do Postgres devolve o que a janela precisa para sugerir nomes: tabelas,
//! views, colunas (com tipo) e funções (com assinatura). Nada de UI, nada de DDL — o `CREATE` é
//! problema do `pg_dump`, não deste módulo.
//!
//! Quem decide **qual** banco ler é o frontend: o caso de uso recebe um cliente já conectado, como os
//! outros casos de uso recebem a conexão resolvida.

use crate::error::CoreError;
use crate::postgres::client::map_error;

/// O que existe em um banco, para efeito de sugestão.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    /// Tabelas, views, materializadas e particionadas, na ordem do catálogo.
    pub tables: Vec<Table>,
    /// Funções e procedures, **uma entrada por nome** (sobrecargas agrupadas).
    pub functions: Vec<Function>,
}

/// Tipo de relação do catálogo (`pg_class.relkind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    /// Tabela comum (`r`).
    Table,
    /// View (`v`).
    View,
    /// View materializada (`m`).
    MaterializedView,
    /// Tabela particionada (`p`).
    Partitioned,
}

impl RelationKind {
    /// Traduz o `relkind` de uma letra; `None` para o que não interessa ao autocomplete.
    fn from_relkind(kind: &str) -> Option<Self> {
        match kind {
            "r" => Some(Self::Table),
            "v" => Some(Self::View),
            "m" => Some(Self::MaterializedView),
            "p" => Some(Self::Partitioned),
            _ => None,
        }
    }
}

/// Uma relação (tabela ou view) com as colunas dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Schema (`public`, `vendas`, ...).
    pub schema: String,
    /// Nome da relação.
    pub name: String,
    /// Tipo da relação.
    pub kind: RelationKind,
    /// Colunas, na ordem de `attnum`.
    pub columns: Vec<Column>,
}

/// Uma coluna com o tipo como o Postgres o escreve (`text`, `numeric(10,2)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Nome da coluna.
    pub name: String,
    /// Tipo formatado por `format_type`.
    pub type_name: String,
}

/// Uma função, com as assinaturas agrupadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Schema da função.
    pub schema: String,
    /// Nome da função.
    pub name: String,
    /// Assinaturas distintas, na forma `text, integer`. `None` quando não há argumentos.
    ///
    /// Sobrecargas viram uma lista separada por ` | `; a partir de três, o resto é resumido em `…`
    /// para a descrição não virar um parágrafo.
    pub arguments: Option<String>,
}

/// Quantas assinaturas distintas cabem na descrição antes de resumir.
const MAX_SIGNATURES: usize = 3;

/// Lê o esquema do banco ao qual `client` está conectado.
///
/// Duas consultas: relações+colunas numa passada, e funções com assinatura. Nenhuma delas olha
/// `pg_catalog`/`information_schema` do próprio sistema — o que se pode ver já é filtrado pelas
/// permissões do usuário conectado.
pub async fn load(client: &tokio_postgres::Client) -> Result<Schema, CoreError> {
    let relation_rows = client
        .query(
            "SELECT n.nspname, c.relname, c.relkind::text, a.attname, \
                    format_type(a.atttypid, a.atttypmod) \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_catalog.pg_attribute a \
                    ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
             WHERE c.relkind IN ('r','v','m','p') \
               AND n.nspname NOT IN ('pg_catalog','information_schema') \
             ORDER BY n.nspname, c.relname, a.attnum",
            &[],
        )
        .await
        .map_err(map_error)?;

    let mut tables: Vec<Table> = Vec::new();
    for row in relation_rows {
        let schema_name: String = row.get(0);
        let table_name: String = row.get(1);
        let relkind: String = row.get(2);
        let Some(kind) = RelationKind::from_relkind(&relkind) else {
            continue;
        };

        // As linhas vêm ordenadas por schema/nome: a tabela corrente é a última da lista.
        if tables
            .last()
            .is_none_or(|last| last.schema != schema_name || last.name != table_name)
        {
            tables.push(Table {
                schema: schema_name,
                name: table_name,
                kind,
                columns: Vec::new(),
            });
        }

        // O `LEFT JOIN` produz uma linha sem coluna para relação sem atributos utilizáveis.
        let column_name: Option<String> = row.get(3);
        let type_name: Option<String> = row.get(4);
        if let (Some(name), Some(type_name)) = (column_name, type_name)
            && let Some(table) = tables.last_mut()
        {
            table.columns.push(Column { name, type_name });
        }
    }

    let function_rows = client
        .query(
            "SELECT n.nspname, p.proname, pg_get_function_arguments(p.oid) \
             FROM pg_catalog.pg_proc p \
             JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
             WHERE n.nspname NOT IN ('pg_catalog','information_schema') \
             ORDER BY 1, 2",
            &[],
        )
        .await
        .map_err(map_error)?;

    let mut functions: Vec<Function> = Vec::new();
    let mut signatures: Vec<Vec<String>> = Vec::new();
    for row in function_rows {
        let schema_name: String = row.get(0);
        let function_name: String = row.get(1);
        let arguments: String = row.get(2);

        if functions
            .last()
            .is_none_or(|last| last.schema != schema_name || last.name != function_name)
        {
            functions.push(Function {
                schema: schema_name,
                name: function_name,
                arguments: None,
            });
            signatures.push(Vec::new());
        }

        // Sobrecarga: a mesma função aparece uma vez por assinatura.
        if let Some(list) = signatures.last_mut()
            && !arguments.trim().is_empty()
            && !list.contains(&arguments)
        {
            list.push(arguments);
        }
    }

    for (function, list) in functions.iter_mut().zip(signatures) {
        function.arguments = summarize_signatures(&list);
    }

    Ok(Schema { tables, functions })
}

/// Resume as assinaturas para a descrição do item de autocomplete.
fn summarize_signatures(signatures: &[String]) -> Option<String> {
    if signatures.is_empty() {
        return None;
    }

    let mut text = signatures
        .iter()
        .take(MAX_SIGNATURES)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");

    if signatures.len() > MAX_SIGNATURES {
        text.push_str(" | …");
    }

    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relkind_translates_only_what_the_autocomplete_needs() {
        assert_eq!(RelationKind::from_relkind("r"), Some(RelationKind::Table));
        assert_eq!(RelationKind::from_relkind("v"), Some(RelationKind::View));
        assert_eq!(
            RelationKind::from_relkind("m"),
            Some(RelationKind::MaterializedView)
        );
        assert_eq!(
            RelationKind::from_relkind("p"),
            Some(RelationKind::Partitioned)
        );
        assert_eq!(
            RelationKind::from_relkind("i"),
            None,
            "índice não sugere nada"
        );
    }

    #[test]
    fn a_single_signature_shows_as_is() {
        let signatures = vec!["text, integer".to_string()];
        assert_eq!(
            summarize_signatures(&signatures),
            Some("text, integer".to_string())
        );
    }

    #[test]
    fn overloads_are_listed_up_to_the_cap() {
        let signatures: Vec<String> = (0..5).map(|index| format!("arg{index}")).collect();
        let summary = summarize_signatures(&signatures).expect("tem assinatura");
        assert_eq!(summary, "arg0 | arg1 | arg2 | …");
    }

    #[test]
    fn a_function_without_arguments_has_none() {
        assert_eq!(summarize_signatures(&[]), None);
    }
}
