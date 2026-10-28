# Política de segurança

## Versões suportadas

Só a última release publicada recebe correções de segurança. Se você está em uma versão anterior,
atualize antes de reportar.

## Como reportar

Abra um **security advisory privado** em
<https://github.com/MustelGuild/Mustel/security/advisories/new>.

Não abra issue pública com detalhes de exploração, prova de conceito ou dados de terceiros.

Inclua no relatório:

- a versão do Mustel (`mustel --version`) e o sistema operacional;
- o que você esperava e o que aconteceu;
- os passos para reproduzir, com a configuração mínima necessária;
- o impacto que você enxerga e, se houver, uma sugestão de correção.

## O que esperar

O relatório é avaliado e, se confirmado, a correção sai na próxima release. O detalhe técnico é
publicado no aviso depois que a correção estiver disponível. Se você quiser crédito, diga isso no
relatório.

## Escopo

Faz parte do escopo o que o Mustel controla:

- senha exposta em `argv`, log, mensagem de erro ou em arquivo que não seja o `pgpass` pedido pelo
  usuário;
- conexão TLS aceita sem validar cadeia e hostname, ou queda para texto puro fora do `Prefer`;
- nome de banco vindo do servidor que escape do diretório de saída no `backup`;
- fórmula de planilha não escapada quando `--csv-escape-formulas` está ligado;
- argumento injetado em processo externo (`pg_dump`/`pg_restore`) — o Mustel nunca usa shell;
- `baseUrl` de IA que aceite `http` fora de loopback ou credenciais na URL.

Falha no PostgreSQL ou no sistema operacional não é do Mustel: reporte ao projeto de origem. Falha
em dependência também deve ser reportada ao projeto de origem; o Mustel atualiza a dependência e
publica a correção na próxima release.

As decisões de segurança do Mustel, com o teste que prova cada uma, estão em
[`docs/explanation/seguranca.md`](docs/explanation/seguranca.md).
