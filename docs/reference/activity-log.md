# Referência: log de atividade

O Mustel mantém um **log local** do que executou e do que falhou. Ele alimenta a aba
[Settings › Activity](#a-aba-activity) e o relatório anonimizado — nada é enviado sem a ação
explícita do usuário.

## Onde fica

| Sistema | Diretório |
|---|---|
| Linux | `$XDG_STATE_HOME/Mustel/logs` (normalmente `~/.local/state/Mustel/logs`) |
| Windows | `%LOCALAPPDATA%\Mustel\logs` |
| macOS | `~/Library/Application Support/Mustel/logs` |

`MUSTEL_LOG_DIR` sobrescreve o diretório inteiro (é o que isola os testes e o harness de paridade do
log real do usuário). O diretório é criado no primeiro evento; se o SO não expuser pasta de estado, a
aba mostra o aviso e o log não é gravado.

Os relatórios gerados pelo botão `Report…` ficam em `reports/`, dentro da mesma raiz.

## Formato

Um arquivo por dia: `activity-AAAA-MM-DD.jsonl`, **uma linha JSON por evento**. Exemplo:

```json
{"at_ms":1762433591000,"area":"query","op":"query.run","status":"ok","target":"Local","detail":"42 row(s) · 1 ok, 0 failed","duration_ms":812}
```

| Campo | Tipo | O que é |
|---|---|---|
| `at_ms` | inteiro | instante em milissegundos desde 1970-01-01T00:00:00Z (UTC) |
| `area` | texto | `app`, `query`, `database`, `connections`, `settings`, `ai` ou `selector` |
| `op` | texto | identificador estável da operação (`query.run`, `connection.test`, …) |
| `status` | texto | `ok`, `error`, `cancelled` ou `info` |
| `target` | texto? | alvo (servidor, perfil, `servidor/base`) |
| `detail` | texto? | resumo (`42 rows`, `3 databases`) |
| `error` | texto? | mensagem de erro |
| `sql` | texto? | prévia do SQL (**só local**, no máximo 200 caracteres em uma linha) |
| `duration_ms` | inteiro? | duração, quando o chamador mede |

Os campos opcionais são omitidos quando vazios. O arquivo é **append-only**; uma linha malformada
(corte de energia, escrita interrompida) é ignorada na leitura e não derruba a aba.

## Retenção

Arquivos diários são mantidos por **30 dias**; os mais antigos são apagados no arranque da janela e
de cada comando do CLI. A tabela da aba carrega a janela inteira; a leitura do relatório também.

## O que é registrado

| Origem | Entradas |
|---|---|
| janela | cada resultado de job (consulta, esquema, lista de bases, teste de conexão, servidor salvo/removido, senha, seletor, IA, falha genérica), com a duração |
| janela | `app.start` (versão e SO), `app.stop`, `app.config` (erro de configuração na abertura) |
| CLI | o comando executado (área, operação, alvo, duração), o status e o erro; erros de linha de comando |
| qualquer thread | `app.panic` com a mensagem, o local e o **backtrace** |

O log é **silencioso**: gravá-lo não muda o stdout/stderr do CLI nem o comportamento dos comandos.

## A aba Activity

A categoria **Activity** (aba Settings) mostra as entradas em uma tabela virtualizada:

- colunas `Time (UTC)`, `Area`, `Operation`, `Result`, `Target` e `Detail`;
- filtro **All** / **Errors**;
- clique na linha mostra o detalhe (alvo, erro completo, prévia do SQL local) e um `Copy`;
- `Refresh` relê o disco — é assim que execuções feitas pelo CLI com a janela aberta aparecem;
- `Open folder` abre o diretório do log no gerenciador de arquivos;
- `Report…` abre o relatório anonimizado.

## O relatório

`Report…` monta o texto a partir do **disco** (todo o log retido, sem truncamento), com cabeçalho
(versão, SO/arquitetura, data de geração, período), tabela cronológica e uma seção `ERRORS` com as
mensagens completas. A prévia mostra:

- o **corpo do e-mail** (mensagem padrão com versão e SO), editável;
- o **anexo `.log`** completo — em modo de leitura, com `Edit` para alterar;
- a **legenda** do que foi anonimizado, marcada como **não enviada**.

`Save` e `Send` gravam `reports/mustel-report-AAAAMMDD-HHMMSS.log` com exatamente o texto da prévia e
abrem o cliente de e-mail para `support@mustelguild.dev`. O anexo é **melhor esforço**: no Linux o
`xdg-email --attach` anexa de verdade; no Windows/macOS o `mailto:` leva o parâmetro `attach=`, que
Thunderbird/Evolution honram e Outlook/Mail.app ignoram. O arquivo salvo fica no disco para anexar à
mão quando o cliente não cooperar.

O que sai da máquina é só o que está na prévia. A legenda (com os valores reais) e o SQL **nunca**
entram no texto do relatório.

## Limites conhecidos

- Um encerramento abrupto (kill, queda de energia) **não** gera uma entrada de "interrompido"; o
  panic hook cobre panics, não abortos.
- O SQL completo não é guardado: o log local tem uma prévia de 200 caracteres, e o relatório não leva
  SQL nenhum.
- O log local não é criptografado — ele vive na pasta de estado do usuário, com as permissões dela.
