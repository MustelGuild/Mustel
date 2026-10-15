# Oráculo de paridade

O Mustel é uma reescrita: o comportamento atual é **observado no binário .NET** (o oráculo), não
inferido da documentação. Este diretório guarda o processo e as evidências dessa observação.

```
oracle/
├── bin/                  (ignorado) FurLab.CLI publicado pelo harness
├── workdir/              (ignorado) diretório de trabalho das capturas
├── reports/              (ignorado) relatórios de `oracle-compare`
├── baseline/             (versionado) uma captura por caso — o contrato
├── perf/                 números de performance (.NET como baseline)
└── known-divergences.md  divergências aceitas + lista branca de resíduos do nome legado
```

## Como o baseline é capturado

```bash
# publica o legado em oracle/bin e grava/atualiza oracle/baseline/*.json
cargo run -p mustel-testkit --bin oracle-capture

# reaproveita o binário já publicado (mais rápido)
cargo run -p mustel-testkit --bin oracle-capture -- --skip-publish

# usa um binário específico
MUSTEL_ORACLE_BIN=/caminho/para/FurLab.CLI cargo run -p mustel-testkit --bin oracle-capture
```

Cada captura (`oracle/baseline/<slug>.json`) registra: argumentos, **exit code**, `stdout`/`stderr`
(como texto *lossy* para leitura **e** em base64, porque a saída de tabela do legado não é UTF-8) e
os artefatos gerados (hash SHA-256 + conteúdo quando é texto).

O `oracle-capture` **preserva a configuração real do usuário**: ele salva
`%LOCALAPPDATA%\FurLab\furlab.jsonc`, semeia os fixtures, captura e restaura no fim (inclusive em
pânico, via `Drop`).

## Fatos de paridade já observados (T9)

| Observação | Consequência para o Mustel |
|-----------|----------------------------|
| Erros vão para **stdout**; `stderr` ficou vazio em 7/7 casos | `report_error` imprime em stdout |
| Erro de uso (comando/opção desconhecida, argumento ausente) sai com **1** | `handle_parse_error` remapeia o `2` do clap |
| `query run` com arquivo ausente sai com **1** (exceção própria do legado, não o `FileNotFoundException` do `Program.cs`) | T32 deve usar `1` com `SQL input file not found: <path>` |
| `rm` de servidor inexistente: `Error: Server 'x' not found.` com exit **41** | mensagem do `ConfigError::ServerNotFound` casada com esse texto |
| Config do usuário é criada vazia na primeira execução, com **CRLF** no Windows | escrita usa quebras de linha do SO |
| Tabela do `ls` sai em **codepage do console** (não-UTF-8) | o comparador decodifica/normaliza; o Mustel emite UTF-8 (divergência de bytes aceita) |
| Senha ausente em modo não-interativo → o legado **quebra** (`Failed to read input in non-interactive mode`) com exit 1 ou 40 | divergência deliberada: o Mustel falha claro e sugere `-W`/`set-password` |

## Próximo passo

`oracle-compare` (T10) executa o `mustel`, compara com este baseline e classifica as divergências na
escada N1–N4 descrita na seção 4.4 de `MustelPlans/migracao-rust.md`.
