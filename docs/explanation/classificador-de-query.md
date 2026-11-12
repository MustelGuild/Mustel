# Por que o classificador de consulta é assim

O `query run` pede confirmação antes de executar uma consulta destrutiva. A classificação precisa
ser **conservadora**: é melhor perguntar por uma consulta segura do que executar um `DELETE` sem
aviso.

## O problema do heurístico antigo

O produto anterior olhava só a primeira palavra da consulta (com uma regra especial para o primeiro
CTE). Isso deixava passar:

```sql
WITH antigos AS (SELECT * FROM users),
     apagados AS (DELETE FROM users RETURNING *)
SELECT * FROM apagados;
```

A primeira palavra é `WITH`; o primeiro CTE é `SELECT`; a classificação era **segura**, mas a query
apaga dados. O mesmo acontecia com:

```sql
EXPLAIN ANALYZE DELETE FROM users;
```

`EXPLAIN` era tratado como leitura, embora `EXPLAIN ANALYZE` **execute** o comando.

## O que o Mustel faz agora

O classificador usa `sqlparser` com o dialeto PostgreSQL. Ele percorre a AST inteira:

- qualquer CTE que contenha `INSERT`, `UPDATE`, `DELETE`, `MERGE` ou DDL do legado;
- o corpo da `Query`, inclusive `SetOperation` e DML representado como `SetExpr`;
- `EXPLAIN ANALYZE` de uma operação destrutiva;
- `SET ROLE`;
- as dez palavras do legado: `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`, `CREATE`, `TRUNCATE`,
  `MERGE`, `GRANT`, `REVOKE`;
- as operações que o legado deixava passar: `COPY ... FROM`, `COPY ... PROGRAM` (nos dois sentidos),
  `CALL`, `SELECT ... INTO`, `VACUUM`, `REINDEX`, `CLUSTER`, `REFRESH MATERIALIZED VIEW` e `DO`.

`COPY ... TO STDOUT`/arquivo continua **seguro**: ele lê o banco. `SELECT ... FOR UPDATE` também
segue **seguro** (o legado não o tratava como escrita), assim como `EXPLAIN` **sem** `ANALYZE`.

> O `DO` e os comandos de manutenção sem variante própria na AST (`REINDEX`, `CLUSTER`,
> `REFRESH`) são reconhecidos pelo tokenizador quando o parse falha. A lista de palavras do
> fallback inclui `COPY`, `CALL`, `DO`, `VACUUM`, `REINDEX`, `CLUSTER`, `REFRESH` e `INTO`.

## Quando o parse falha

Uma consulta que o parser não consegue ler não pode ser considerada segura. O Mustel tokeniza e
procura as palavras destrutivas fora de strings e comentários. Se até o tokenizador falhar, usa o
heurístico antigo como último recurso.

## A descrição do prompt

Nos 21 casos do legado, o `Query Type` continua idêntico (`SELECT`, `DELETE`, ...). Nos casos novos,
a descrição mostra a operação real: um `DELETE` escondido em CTE ou `EXPLAIN ANALYZE` aparece como
`DELETE`, não como `SELECT`/`WITH`/`EXPLAIN`. Isso é uma mudança N4, registrada em
[diferenças deliberadas](diferencas-deliberadas.md).

## Modo somente leitura (aba Settings)

O checkbox **`Only run read-only queries`** grava `gui.readOnlyQueries` e vem **marcado por padrão**
na primeira execução; desmarcar grava `false`. Com ele ligado, a classificação continua a mesma, mas
a decisão muda:

- consulta **destrutiva**: em vez do modal com `Run anyway`, a janela abre
  **`READ-ONLY MODE: QUERY NOT RUN`** e não executa;
- consulta **segura**: executa normalmente, com `BEGIN TRANSACTION READ ONLY` na conexão de cada
  banco. O servidor recusa escrita com SQLSTATE `25006`.

O `BEGIN READ ONLY` é a segunda camada: ele pega o que o classificador não vê. `SELECT ... FOR
UPDATE` e `nextval()` são `Safe` para o `analyzer` (o legado não os tratava como escrita), mas o
PostgreSQL os recusa na transação read-only. O mesmo vale para uma função que escreve em tabela.

O que esta camada **não** bloqueia: `COPY ... TO STDOUT` (é leitura), escrita em tabela temporária,
`pg_notify`/advisory locks, funções `SECURITY DEFINER` e `dblink` para outro servidor. Bloquear
`COPY` exige uma whitelist no cliente (fase seguinte). O modo também é **só da janela**: o CLI não
o lê.

## Testes

`crates/mustel-core/src/query/analyzer.rs` cobre os 21 casos do legado, os três bypasses
(CTE com DML, `EXPLAIN ANALYZE` e `SET ROLE`), falsos positivos (`SELECT 'delete'` e identificadores
citados), CTE múltiplo, falha de parse com palavra destrutiva e as operações novas (`COPY`, `CALL`,
`SELECT INTO`, `VACUUM`, `REINDEX`, `CLUSTER`, `REFRESH` e `DO`).
