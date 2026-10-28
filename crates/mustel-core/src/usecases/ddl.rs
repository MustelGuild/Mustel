//! DDL de um banco, para o contexto da IA — lido do **catálogo**, sem `pg_dump`.
//!
//! Por que catálogo e não `pg_dump`: o `pg_dump` exige o binário na máquina e **recusa servidor mais
//! novo que ele** (`server version 18.3; pg_dump version 16.6` já quebrou na prática), além de exigir
//! a senha viajando para um processo filho. O catálogo responde pelo cliente `tokio-postgres` que o
//! app já usa, em qualquer versão de servidor, e o resultado é o mesmo subconjunto que a IA precisa:
//! tabela, coluna, tipo, `NOT NULL`, default, chave primária/estrangeira/única, índice e view.
//!
//! O DDL daqui serve de **contexto para escrever query**, não para recriar o banco: partições,
//! colunas geradas, `CHECK` e comentários ficam de fora de propósito (D1 revisto em 2026-10-25).

use tokio_postgres::Client;

use crate::error::CoreError;
use crate::postgres::client::map_error;

/// O que o painel precisa saber do DDL: o texto e **quando** ele foi gerado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ddl {
    /// Texto no formato `CREATE TABLE`/`ALTER TABLE`/`CREATE INDEX`/`CREATE VIEW`.
    pub text: String,
    /// Data da geração, em ISO 8601 UTC (ex.: `2026-10-24T21:10:00.0000000Z`).
    pub dumped_at: String,
}

/// Uma coluna do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Nome.
    pub name: String,
    /// Tipo como o Postgres escreve (`text`, `numeric(10,2)`).
    pub type_name: String,
    /// `NOT NULL`?
    pub not_null: bool,
    /// Default já renderizado (`nextval(...)`, `'x'::text`), quando houver.
    pub default: Option<String>,
}

/// Uma tabela (ou view) do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// Schema.
    pub schema: String,
    /// Nome.
    pub name: String,
    /// É view (inclui materializada)?
    pub is_view: bool,
    /// Colunas, na ordem do catálogo.
    pub columns: Vec<Column>,
    /// Corpo da view (`SELECT …`), só para `is_view`.
    pub view_sql: Option<String>,
    /// Definições prontas de constraint (`PRIMARY KEY (id)`, `FOREIGN KEY … REFERENCES …`).
    pub constraints: Vec<(String, String)>,
    /// `CREATE INDEX ...` já renderizado pelo servidor.
    pub indexes: Vec<String>,
}

/// Lê o catálogo e devolve o DDL como texto.
///
pub async fn load_ddl(client: &Client) -> Result<String, CoreError> {
    let mut relations = Vec::new();

    // Relações + colunas numa passada. `relkind`: r=tabela, p=particionada, v=view, m=matview.
    let rows = client
        .query(
            "SELECT n.nspname, c.relname, c.relkind::text, a.attname, \
                    format_type(a.atttypid, a.atttypmod), \
                    a.attnotnull, \
                    pg_get_expr(d.adbin, d.adrelid) \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_catalog.pg_attribute a \
                    ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
             LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum \
             WHERE c.relkind IN ('r','p','v','m') \
               AND n.nspname NOT IN ('pg_catalog','information_schema') \
             ORDER BY n.nspname, c.relname, a.attnum",
            &[],
        )
        .await
        .map_err(map_error)?;

    for row in rows {
        let schema: String = row.get(0);
        let name: String = row.get(1);
        let kind: String = row.get(2);
        let is_view = kind == "v" || kind == "m";

        if relations
            .last()
            .is_none_or(|last: &Relation| last.schema != schema || last.name != name)
        {
            relations.push(Relation {
                schema,
                name,
                is_view,
                columns: Vec::new(),
                view_sql: None,
                constraints: Vec::new(),
                indexes: Vec::new(),
            });
        }

        let column: Option<String> = row.get(3);
        let type_name: Option<String> = row.get(4);
        if let (Some(column), Some(type_name)) = (column, type_name)
            && let Some(last) = relations.last_mut()
        {
            last.columns.push(Column {
                name: column,
                type_name,
                not_null: row.get(5),
                default: row.get(6),
            });
        }
    }

    // Constraints: o servidor escreve a definição (`PRIMARY KEY (id)`, `FOREIGN KEY …`).
    let constraints = client
        .query(
            "SELECT n.nspname, c.relname, con.conname, pg_get_constraintdef(con.oid) \
             FROM pg_catalog.pg_constraint con \
             JOIN pg_catalog.pg_class c ON c.oid = con.conrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE con.contype IN ('p','f','u') \
               AND n.nspname NOT IN ('pg_catalog','information_schema') \
             ORDER BY n.nspname, c.relname, con.conname",
            &[],
        )
        .await
        .map_err(map_error)?;

    for row in constraints {
        let schema: String = row.get(0);
        let name: String = row.get(1);
        let constraint: String = row.get(2);
        let definition: String = row.get(3);
        if let Some(relation) = relations
            .iter_mut()
            .find(|relation| relation.schema == schema && relation.name == name)
        {
            relation.constraints.push((constraint, definition));
        }
    }

    // Índices: `pg_get_indexdef` já dá a instrução inteira.
    let indexes = client
        .query(
            "SELECT n.nspname, c.relname, pg_get_indexdef(i.indexrelid) \
             FROM pg_catalog.pg_index i \
             JOIN pg_catalog.pg_class c ON c.oid = i.indrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname NOT IN ('pg_catalog','information_schema') \
             ORDER BY n.nspname, c.relname, i.indexrelid",
            &[],
        )
        .await
        .map_err(map_error)?;

    for row in indexes {
        let schema: String = row.get(0);
        let name: String = row.get(1);
        let definition: String = row.get(2);
        if let Some(relation) = relations
            .iter_mut()
            .find(|relation| relation.schema == schema && relation.name == name)
        {
            relation.indexes.push(definition);
        }
    }

    // Views: o corpo (`SELECT …`), para a IA saber o que ela devolve.
    let views = client
        .query(
            "SELECT n.nspname, c.relname, pg_get_viewdef(c.oid, true) \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('v','m') \
               AND n.nspname NOT IN ('pg_catalog','information_schema')",
            &[],
        )
        .await
        .map_err(map_error)?;

    for row in views {
        let schema: String = row.get(0);
        let name: String = row.get(1);
        let definition: String = row.get(2);
        if let Some(relation) = relations
            .iter_mut()
            .find(|relation| relation.schema == schema && relation.name == name)
        {
            relation.view_sql = Some(definition.trim().to_string());
        }
    }

    Ok(render_ddl(&relations))
}

/// Monta o texto final: `CREATE TABLE` com PK inline, `ALTER TABLE` das demais constraints,
/// `CREATE INDEX` e `CREATE VIEW`.
///
/// Função **pura**: é aqui que o formato é decidido, e por isso é ela que os testes exercitam.
pub fn render_ddl(relations: &[Relation]) -> String {
    let mut statements: Vec<String> = Vec::new();

    for relation in relations {
        let qualified = format!("{}.{}", quote(&relation.schema), quote(&relation.name));

        if relation.is_view {
            let body = relation.view_sql.clone().unwrap_or_default();
            statements.push(format!("CREATE VIEW {qualified} AS\n{body};"));
            continue;
        }

        // Chaves primárias entram **inline**, como no `pg_dump` — é o que o modelo lê melhor.
        let primary: Vec<&str> = relation
            .constraints
            .iter()
            .filter(|(_, definition)| definition.starts_with("PRIMARY KEY"))
            .map(|(_, definition)| definition.as_str())
            .collect();

        let mut lines: Vec<String> = Vec::new();
        for column in &relation.columns {
            let mut line = format!("    {} {}", quote(&column.name), column.type_name);
            if column.not_null {
                line.push_str(" NOT NULL");
            }
            if let Some(default) = &column.default {
                line.push_str(&format!(" DEFAULT {default}"));
            }
            lines.push(line);
        }
        for definition in &primary {
            lines.push(format!("    {definition}"));
        }

        statements.push(format!(
            "CREATE TABLE {qualified} (\n{}\n);",
            lines.join(",\n")
        ));

        // As demais constraints (FK/UNIQUE) viram `ALTER TABLE`, como o Postgres faz.
        for (name, definition) in &relation.constraints {
            if definition.starts_with("PRIMARY KEY") {
                continue;
            }
            statements.push(format!(
                "ALTER TABLE ONLY {qualified} ADD CONSTRAINT {} {definition};",
                quote(name)
            ));
        }

        for index in &relation.indexes {
            statements.push(format!("{index};"));
        }
    }

    let mut text = statements.join("\n\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// Aspas de identificador, escapando aspas internas (nomes de tabela/coluna podem ser qualquer coisa).
fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Lê o DDL do cache ou gera do catálogo, conforme o pedido.
///
/// Regras (D2/D3):
/// - sem cache, ou `refresh = true` → lê o catálogo e regrava o cache;
/// - com cache e `refresh = false` → **não** consulta o servidor;
/// - falha na leitura deixa o cache anterior valendo (o erro sobe para a tela).
pub async fn ensure_ddl(
    client: &Client,
    cache_dir: &std::path::Path,
    server: &str,
    database: &str,
    refresh: bool,
) -> Result<Ddl, CoreError> {
    let stem = crate::config::paths::ddl_file_stem(server, database);
    let sql_path = cache_dir.join(format!("{stem}.sql"));
    let meta_path = cache_dir.join(format!("{stem}.json"));

    if !refresh && let Some(cached) = read_cache(&sql_path, &meta_path) {
        return Ok(cached);
    }

    let text = load_ddl(client).await?;
    let dumped_at = crate::query::csv_writer::iso8601_for(std::time::SystemTime::now());
    let ddl = Ddl {
        text,
        dumped_at: dumped_at.clone(),
    };

    write_atomically(&sql_path, ddl.text.as_bytes())?;
    let meta = format!("{{\n  \"dumpedAt\": \"{}\"\n}}\n", escaped(&dumped_at));
    write_atomically(&meta_path, meta.as_bytes())?;

    Ok(ddl)
}

/// Lê o par `.sql` + `.json`; qualquer coisa estranha vira `None` (cai para o catálogo).
fn read_cache(sql_path: &std::path::Path, meta_path: &std::path::Path) -> Option<Ddl> {
    let text = std::fs::read_to_string(sql_path).ok()?;
    let meta = std::fs::read_to_string(meta_path).ok()?;
    let dumped_at = meta
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"dumpedAt\":"))
        .map(|value| {
            value
                .trim()
                .trim_end_matches(',')
                .trim_matches('"')
                .to_string()
        })?;
    Some(Ddl { text, dumped_at })
}

/// Escapa o mínimo para o JSON de uma linha.
fn escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escreve via arquivo temporário + `rename`.
fn write_atomically(path: &std::path::Path, bytes: &[u8]) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        crate::private_fs::create_dir(parent)?;
    }
    let temp = path.with_extension("tmp");
    crate::private_fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, type_name: &str, not_null: bool, default: Option<&str>) -> Column {
        Column {
            name: name.to_string(),
            type_name: type_name.to_string(),
            not_null,
            default: default.map(str::to_string),
        }
    }

    fn table() -> Relation {
        Relation {
            schema: "public".to_string(),
            name: "pedidos".to_string(),
            is_view: false,
            columns: vec![
                column(
                    "id",
                    "integer",
                    true,
                    Some("nextval('pedidos_id_seq'::regclass)"),
                ),
                column("cliente_id", "integer", false, None),
                column("total", "numeric(10,2)", false, None),
            ],
            view_sql: None,
            constraints: vec![
                ("pedidos_pkey".to_string(), "PRIMARY KEY (id)".to_string()),
                (
                    "pedidos_cliente_id_fkey".to_string(),
                    "FOREIGN KEY (cliente_id) REFERENCES clientes(id)".to_string(),
                ),
            ],
            indexes: vec![
                "CREATE INDEX idx_pedidos_cliente ON public.pedidos USING btree (cliente_id)"
                    .to_string(),
            ],
        }
    }

    /// A tabela sai com colunas, `NOT NULL`, default, PK **inline** e as chaves no `ALTER TABLE`.
    #[test]
    fn a_table_renders_with_columns_keys_and_indexes() {
        let ddl = render_ddl(&[table()]);

        assert!(
            ddl.contains("CREATE TABLE \"public\".\"pedidos\" ("),
            "{ddl}"
        );
        assert!(
            ddl.contains("\"id\" integer NOT NULL DEFAULT nextval('pedidos_id_seq'::regclass)")
        );
        assert!(ddl.contains("\"total\" numeric(10,2)"), "{ddl}");
        assert!(ddl.contains("    PRIMARY KEY (id)"), "a PK é inline: {ddl}");
        assert!(
            ddl.contains(
                "ALTER TABLE ONLY \"public\".\"pedidos\" ADD CONSTRAINT \"pedidos_cliente_id_fkey\" FOREIGN KEY (cliente_id) REFERENCES clientes(id);"
            ),
            "a FK vira ALTER TABLE: {ddl}"
        );
        assert!(
            ddl.contains("CREATE INDEX idx_pedidos_cliente ON public.pedidos"),
            "o índice vem pronto do servidor: {ddl}"
        );
        // A PK **não** se repete no ALTER TABLE.
        assert_eq!(ddl.matches("PRIMARY KEY").count(), 1, "{ddl}");
    }

    /// View sai como `CREATE VIEW … AS` com o corpo do servidor.
    #[test]
    fn a_view_renders_its_body() {
        let view = Relation {
            schema: "public".to_string(),
            name: "vendas_mes".to_string(),
            is_view: true,
            columns: vec![column("mes", "date", false, None)],
            view_sql: Some(
                "SELECT date_trunc('month', created_at) AS mes\n  FROM pedidos;".to_string(),
            ),
            constraints: Vec::new(),
            indexes: Vec::new(),
        };

        let ddl = render_ddl(&[view]);

        assert!(
            ddl.contains("CREATE VIEW \"public\".\"vendas_mes\" AS"),
            "{ddl}"
        );
        assert!(ddl.contains("date_trunc('month', created_at)"), "{ddl}");
        assert!(!ddl.contains("CREATE TABLE"), "view não vira tabela: {ddl}");
    }

    /// Identificador com aspas ou maiúsculas é citado (e a aspa interna é escapada).
    #[test]
    fn identifiers_are_quoted() {
        let odd = Relation {
            schema: "Vendas".to_string(),
            name: "Nota \"Fiscal\"".to_string(),
            is_view: false,
            columns: vec![column("Valor Total", "numeric", false, None)],
            view_sql: None,
            constraints: Vec::new(),
            indexes: Vec::new(),
        };

        let ddl = render_ddl(&[odd]);

        assert!(ddl.contains("\"Vendas\".\"Nota \"\"Fiscal\"\"\""), "{ddl}");
        assert!(ddl.contains("\"Valor Total\" numeric"), "{ddl}");
    }

    /// Catálogo vazio vira texto vazio (não um `\n` solto).
    #[test]
    fn an_empty_catalog_renders_nothing() {
        assert_eq!(render_ddl(&[]), "");
    }

    /// O cache evita gerar de novo: com `.sql` + `.json` no disco, `ensure_ddl` devolve o texto.
    #[test]
    fn the_cache_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ddl = Ddl {
            text: "CREATE TABLE \"public\".\"t\" (\n    \"id\" integer\n);\n".to_string(),
            dumped_at: "2026-10-25T10:00:00.0000000Z".to_string(),
        };

        let path = dir.path().join("Local__dev.sql");
        let meta = dir.path().join("Local__dev.json");
        write_atomically(&path, ddl.text.as_bytes()).expect("sql");
        write_atomically(
            &meta,
            format!("{{\n  \"dumpedAt\": \"{}\"\n}}\n", ddl.dumped_at).as_bytes(),
        )
        .expect("meta");

        let cached = read_cache(&path, &meta).expect("cache");
        assert_eq!(cached, ddl);
    }

    /// Cache incompleto (só o `.sql`) é ignorado: melhor gerar de novo que servir texto sem data.
    #[test]
    fn an_incomplete_cache_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.sql");
        std::fs::write(&path, "CREATE TABLE t (id integer);").expect("escreve");

        assert!(read_cache(&path, &dir.path().join("a.json")).is_none());
    }
}
