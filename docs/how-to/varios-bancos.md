# Rodar a mesma consulta em vários bancos

**Situação:** você tem um servidor com vários bancos (ou vários servidores) e quer exportar o mesmo
`SELECT` de todos, num CSV só.

## O jeito curto

```bash
mustel query run -i consulta.sql --all
```

`--all` descobre todos os bancos acessíveis do servidor e roda em cada um. A descoberta pergunta ao
Postgres quais bancos aceitam conexão (`datallowconn = true`), **sem** `ORDER BY` — a ordem é a que o
servidor devolve.

## Excluir bancos

Dois mecanismos, com regras diferentes:

| Mecanismo | Onde se configura | Como casa |
|---|---|---|
| `--exclude` | por execução | **nomes exatos**, sem diferenciar maiúsculas; espaço em volta é ignorado |
| `excludePatterns` | no cadastro do servidor | curinga `*`, comparado com o nome do banco |

```bash
# na execução: nomes exatos
mustel query run -i consulta.sql --all --exclude "postgres, template0"

# no cadastro: padrões (o default já é template*,postgres)
mustel settings db-servers add -n prod -H db.exemplo -U leitor \
  --fetch-all --exclude-patterns "template*,postgres,homolog_*"
```

> `--exclude` **não** aceita curinga. Ele compara o nome inteiro; quem usa padrão é o
> `excludePatterns` do cadastro.

## Escolher servidores

Com **um** servidor no cadastro, o Mustel não pergunta nada. Com dois ou mais, aparece a seleção
interativa — todos pré-selecionados; espaço alterna, enter confirma.

## Controlar o paralelismo

O grau vem do cadastro, por servidor:

```bash
mustel settings db-servers add -n prod -H db.exemplo -U leitor --max-parallelism 8
```

Cada servidor roda seus bancos até esse limite. O resultado sai em arquivos separados por servidor e
num consolidado — ver a [referência do `query run`](../reference/query-run.md#arquivos).

## Quando um banco falha

A falha de um banco **não** aborta os outros: ela vai para `<timestamp>_erros.csv` e para a coluna
`Error` do `_log.csv`, e o comando termina com sucesso se ao menos um banco respondeu. Se **nenhum**
banco for alcançável, ele para antes de escrever arquivos e falha.

## Conferir depois

```bash
mustel query run -i consulta.sql --all -o ./saida
column -s, -t < saida/*/consolidated_*.csv | head
```
