# Oráculo de paridade

O Mustel é uma reescrita: o comportamento do produto anterior foi **observado no binário .NET** e
congelado aqui. Este diretório guarda as evidências dessa observação.

> **Desde 2026-10-22 o diretório `legacy/` não existe mais.** O baseline é **contrato congelado**: não
> há como re-gravá-lo. Mudar comportamento agora significa mudar um teste, com revisão explícita — não
> re-capturar.

```
oracle/
├── baseline/             (versionado) uma captura por caso — o contrato congelado
├── perf/                 números de performance (.NET como baseline)
└── known-divergences.md  divergências aceitas + lista branca de resíduos do nome legado
```

## Como o contrato é verificado hoje

```bash
# o gate: replay das 47 capturas + manifesto + superfície, sem .NET
cargo test --workspace

# diagnóstico: mostra as diferenças de texto (N4) caso a caso
cargo run -p mustel-testkit --bin oracle-compare -- --report
```

Cada captura (`oracle/baseline/<slug>.json`) registra: argumentos, **exit code**, `stdout`/`stderr`
(como texto *lossy* para leitura **e** em base64, porque a saída de tabela do antigo não era UTF-8) e
os artefatos gerados (hash SHA-256 + conteúdo quando é texto).

Do lado do Mustel, a configuração e o `pgpass.conf` são sempre isolados (`MUSTEL_CONFIG_DIR`,
`MUSTEL_PGPASS_FILE`) — nenhum arquivo real do usuário é tocado por teste ou comparação.


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

## Política de comparação (T10)

| Item | Nível | Regra |
|------|-------|-------|
| exit code | **contrato** | igual ao oráculo, sempre |
| artefatos (config) | **contrato** | JSON equivalente, com `encryptedPassword`/`password` removidos |
| artefatos (pgpass) | **contrato** | conteúdo **byte a byte** (o `add` anexa e o `remove` reescreve com a quebra de linha do SO) |
| flags longas nos casos de ajuda | **contrato** | toda flag do oráculo precisa existir no Mustel; extras são informativas |
| stdout/stderr | aviso N4 | redação difere por framework; pulado quando a saída do oráculo não é UTF-8 |
| comando ainda não migrado | pulado | não é falha enquanto o stub existir |

O baseline é reproduzível **na mesma máquina**: duas capturas seguidas geram os mesmos bytes
(caminhos viram `<WORKDIR>`/`<REPO>`/`<HOME>`/`<LEGACY_CONFIG>`). Entre sistemas operacionais
diferentes o texto **não** é idêntico — o console do legado quebra linha pela largura, e o
comprimento do caminho muda a quebra. Entre máquinas vale o que é contrato: exit code e artefatos.

Fatos adicionais aprendidos na comparação:

- `database restore` devolve **2** (`return Task.FromResult(2)`) para `FurLabFileNotFoundException`
  e `PathTraversalException` — exit 2 existe no comando, não só no parser; ainda assim não é
  alcançável no cenário determinístico, porque sem senha o legado quebra antes.
- Com a configuração real do usuário presente o comportamento muda (existe senha salva): por isso
  **todo caso do baseline fixa o estado da config** (`config_seed`), senão o baseline não é
  reproduzível.
- `database pgpass add` **não** aceita o banco como posicional: `fur database pgpass add mydb`
  responde `Error: Unknown command 'mydb'.` (exit 1). A forma posicional do Mustel é adição
  deliberada (item 5 de `known-divergences.md`), então nenhum caso do baseline pode usá-la — o argv
  de um caso roda nos **dois** binários.
- O exit **2** de `pgpass add` (banco ausente, host inválido, porta inválida) vem do **corpo** do
  comando legado (`return Task.FromResult(2)`), não do parser: o mustel precisa devolver `Ok(2)` ali,
  e não passar pelo mapeamento de `CoreError` (que daria 41).

## Como o baseline é usado hoje

`oracle-compare` executa o `mustel`, compara com este baseline e classifica as diferenças na escada
N1–N4 descrita em `docs/explanation/escada-de-fidelidade.md`. Ele é **diagnóstico**: o gate é
`cargo test --workspace`, que roda o mesmo `compare_case` nos testes de contrato.
