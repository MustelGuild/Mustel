# Por que o log de atividade é assim

O log nasceu de uma pergunta prática: "se a aplicação der erro ou fechar sozinha, sobra alguma coisa
para eu saber o que aconteceu?". Antes dele, a resposta era não: o erro virava toast de poucos
segundos, o CLI imprimia no terminal e um panic no `mustel-gui` do Windows não deixava rastro nenhum
(sem console, não há stderr).

## Por que um arquivo estruturado, e não `tracing`

O `AGENTS.md` dizia "logs: `tracing`", mas o `tracing` nunca entrou no grafo — a decisão era antiga e
nunca foi implementada. O requisito que veio depois não é só *gravar*: é **mostrar** o que rodou,
**reler** no arranque e **anonimizar** campo a campo para um relatório. Texto solto não diz qual
trecho é o servidor, qual é a base e qual é o erro; o export teria de adivinhar.

Por isso o log é uma `Entry` tipada, serializada em JSONL: a tabela da aba lê, o relatório
reconstrói e o redator substitui com precisão. O `tracing` continua bom para diagnóstico interno
(spans, níveis), mas não substitui o registro que o usuário lê — e dois sistemas de log seriam pior
do que um.

## Por que local, e por que nada sai sozinho

O log é do usuário. Ele fica na pasta de **estado** do SO (não na de configuração: log não é
preferência), com os nomes reais — é o que torna a aba útil. Nada é enviado a lugar nenhum: o que
sai é só o **relatório**, e mesmo assim depois de uma prévia que mostra o corpo e o anexo completos,
permite editar e exige o clique em `Send`.

O endereço do relatório (`support@mustelguild.dev`) aparece no About como um link que pede ao app
para abrir o cliente de e-mail — e, quando o sistema não tem handler de `mailto:`, copia o endereço
em vez de abrir o navegador —, para o destino não ser uma caixa-preta.

## Anonimização: o que some, o que fica

Duas camadas, nesta ordem:

1. **Literais conhecidos** — nome, host, `host:porta` e usuário de cada servidor; nome e `id` de cada
   perfil; bases citadas nos alvos ou no cadastro; o diretório do usuário e o de configuração. Viram
   tokens estáveis (`server-1`, `host-1`, `user-1`, `db-1`, `profile-1`, `<home>`, `<config>`).
2. **Padrões** — o que sobrar em texto livre: identificadores entre aspas (`"clientes"` →
   `"…"`), URLs e connection strings, e-mails, IPv4/IPv6, caminhos Windows e POSIX.

O **SQL não entra**: o log local guarda uma prévia de 200 caracteres (para reconhecer a consulta), e
o relatório descarta o campo inteiro. Mensagens de erro do Postgres costumam citar tabelas e colunas
entre aspas; por isso o conteúdo entre aspas é apagado mesmo quando o nome não está no cadastro.

O que **fica** é o que diagnostica: versão, SO/arquitetura, operação, alvo tokenizado, duração,
contagem de linhas, o miolo da mensagem (`does not exist`, `connection refused`) e o backtrace de um
panic.

A **legenda** com os valores reais existe só na tela, marcada como "não enviada": ela é o que deixa
o usuário conferir a substituição. A prévia é editável, então o consentimento é explícito — o que
estiver nela é exatamente o que vai no corpo e no `.log`.

## O anexo e o `mailto:`

O pedido era anexar o `.log` ao e-mail. Isso não tem solução portátil: o protocolo `mailto:` não
define anexos, e compor com anexo exigiria uma API nativa por SO (FFI, barrado por
`unsafe_code = "forbid"`) ou um shell (barrado pela regra de processos externos).

O desenho faz o possível e é honesto sobre o resto:

- **Linux**: `xdg-email --attach <arquivo> …` — anexa de verdade; se o `xdg-email` não existir, cai
  no `xdg-open` com a URL `mailto:` e o arquivo fica salvo.
- **Windows**: `rundll32 url.dll,FileProtocolHandler` entrega a URL `mailto:` com `attach=` ao
  cliente padrão, sem shell. Clientes que honram o parâmetro (Thunderbird/Evolution) anexam;
  Outlook ignora.
- **macOS**: `open` com a mesma URL; o Mail.app ignora o `attach=`.

Em todos os casos o `.log` é **sempre salvo** em `reports/`, e a tela do relatório mostra o caminho
com `Open folder`, além de `Copy report` e `Copy e-mail`. Essa é a saída para quem usa **webmail**
ou não tem handler de `mailto:` (o caso em que o sistema responde "Could not read file mailto:…"):
cole o texto no navegador e anexa o `.log` à mão. O botão `Open e-mail app` é conveniência, não
requisito — o relatório nunca fica preso a ele.

## Retenção e disco

Arquivos diários (`activity-AAAA-MM-DD.jsonl`) com **30 dias** de retenção; a poda roda no arranque e
a leitura do relatório é feita do disco, não do conjunto que a tabela está exibindo — assim uma
execução do CLI feita com a janela aberta também entra no relatório. Linhas malformadas são
ignoradas: o log é melhor esforço, não um contrato de integridade.

## O que ficou de fora, de propósito

- **Kill abrupto/queda de energia**: não há pareamento `start`/`finish` por job, então não existe uma
  entrada "interrompido". O panic hook cobre panics, não abortos.
- **SQL completo**: só a prévia local; o relatório nunca leva SQL.
- **Criptografia do log local**: ele não é criptografado; no Unix, o diretório `logs/` é `0700` e os
  arquivos `0600`; no Windows, a proteção é a ACL de `%LOCALAPPDATA%`.
- **Envio automático/telemetria**: nada é enviado sem o `Send` da prévia.
