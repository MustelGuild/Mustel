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
  `MERGE`, `GRANT`, `REVOKE`.

`SELECT ... FOR UPDATE` continua **seguro** (o legado não o tratava como escrita), assim como
`EXPLAIN` **sem** `ANALYZE`.

## Quando o parse falha

Uma consulta que o parser não consegue ler não pode ser considerada segura. O Mustel tokeniza e
procura as palavras destrutivas fora de strings e comentários. Se até o tokenizador falhar, usa o
heurístico antigo como último recurso.

## A descrição do prompt

Nos 21 casos do legado, o `Query Type` continua idêntico (`SELECT`, `DELETE`, ...). Nos casos novos,
a descrição mostra a operação real: um `DELETE` escondido em CTE ou `EXPLAIN ANALYZE` aparece como
`DELETE`, não como `SELECT`/`WITH`/`EXPLAIN`. Isso é uma mudança N4, registrada em
[diferenças deliberadas](diferencas-deliberadas.md).

## Testes

`crates/mustel-core/src/query/analyzer.rs` cobre os 21 casos do legado, os três bypasses
(CTE com DML, `EXPLAIN ANALYZE` e `SET ROLE`), falsos positivos (`SELECT 'delete'` e identificadores
citados), CTE múltiplo e falha de parse com palavra destrutiva.
