# Fixtures de contrato

Este diretório guarda **dados de teste**: as capturas congeladas que o gate reexecuta contra o
binário atual. Nada aqui é documentação de produto — o **porquê** de cada comportamento está em
[`docs/`](../docs/README.md).

```
fixtures/
└── baseline/   (versionado) uma captura por caso — o contrato congelado
```

> **O baseline é contrato congelado:** não há como re-gravá-lo, porque o binário que o produziu não
existe mais. Mudar comportamento significa mudar um teste, com revisão explícita — não re-capturar.

## O que uma captura registra

Cada `fixtures/baseline/<slug>.json` guarda: argumentos (`argv`), **exit code**, `stdout`/`stderr`
(como texto *lossy* para leitura **e** em base64, porque a saída de tabela do produto anterior não era
UTF-8), o estado inicial da config e do `pgpass.conf` (`config_seed`, `pgpass_seed`) e os artefatos
gerados (hash SHA-256 + conteúdo quando é texto).

O estado inicial faz parte do caso: a presença de senha salva muda o comportamento do binário, então
sem `config_seed` a captura não seria reproduzível.

## Como o gate usa

```bash
cargo test --workspace                  # o gate: replay das 47 capturas + manifesto + superfície
cargo run -p mustel-testkit --bin oracle-compare -- --report   # diagnóstico caso a caso
```

- `crates/mustel/tests/contract_replay.rs` gera **um teste por captura** — lista vazia ou caso sem
teste quebra a suíte;
- `contract_surface.rs` exige que todo comando tenha caso e que toda flag do `--help` seja exercitada;
- a comparação é a mesma do diagnóstico (`mustel_testkit::compare`), em sandbox isolado.

Do lado do Mustel, configuração e `pgpass.conf` são sempre isolados (`MUSTEL_CONFIG_DIR`,
`MUSTEL_PGPASS_FILE`): nenhum arquivo real do usuário é tocado por teste ou comparação. O diretório de
trabalho do harness (`fixtures/workdir/`) é ignorado pelo Git.

## Onde ler o resto

| Assunto | Página |
|---|---|
| o que cada nível de paridade exige | [Escada de fidelidade](../docs/explanation/escada-de-fidelidade.md) |
| onde o Mustel escolheu diferente e por quê | [Diferenças deliberadas](../docs/explanation/diferencas-deliberadas.md) |
| por que cada decisão foi tomada | [Decisões de engenharia](../docs/explanation/decisoes.md) |
