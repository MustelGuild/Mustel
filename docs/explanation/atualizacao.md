# Por que a atualização é assim

> O updater existe para o dono do Mustel não precisar procurar a release e trocar arquivos à mão.
> Esta página explica as decisões que moldaram o desenho — inclusive as que deixam o recurso mais
> conservador do que pareceria necessário.

## O canal é o GitHub Releases, não o `cargo install`

O updater baixa o **mesmo artefato** que a página de release publica. Instalação por `cargo
install` **não é um canal suportado**: o binário mora no diretório do Cargo, e um
`cargo install --force` posterior pode sobrescrever a troca feita pelo helper. Quem instalou assim
atualiza pelo próprio Cargo.

## Por que um helper copiado

O Windows **não deixa sobrescrever o executável em execução**, e o próprio processo também não
consegue se mover. A saída é copiar o executável atual (não o payload baixado) para um arquivo
temporário ao lado e chamá-lo com `--apply-update`: a cópia não está em execução, então pode fazer
o backup `.old`, mover os arquivos e relançar o app. No Linux o problema não existe, mas o mesmo
caminho é usado — um fluxo só, testável, em vez de dois.

O helper roda **antes** do `clap` e do `eframe`: o modo oculto não aparece na árvore de comandos,
não abre janela e não escreve no log de atividade (não houve execução de usuário).

## Por que os dois `.exe` no Windows

`mustel.exe` e `mustel-gui.exe` são binários independentes que compartilham a biblioteca. Deixar um
para trás produziria duas versões lado a lado, com a mesma janela e comportamentos diferentes. O
updater os trata como **um pacote**: baixa e verifica os dois antes de tocar em qualquer um, e o
helper faz **rollback de todos** se um falhar. Se só um existe ao lado do outro, atualiza só ele.

## Por que o hash vem antes da troca

Um download interrompido ou adulterado não pode virar um executável no lugar do que funciona. O
SHA-256 vem do `digest` da API do GitHub ou, quando ele falta, do `SHA256SUMS` da release; sem
hash, a release é recusada. O arquivo parcial é apagado tanto na falha quanto no cancelamento.

## Por que o toast é sticky

A atualização não é um recado passageiro: ela muda o programa. Um toast de 4 segundos seria perdido
enquanto o usuário trabalha. O toast fica até uma decisão — `Update` baixa e reinicia, `X` adia.
Durante o download, o `X` **cancela** (e apaga os parciais) em vez de só esconder o aviso, e a barra
de tempo vira barra de progresso: o mesmo espaço, a informação que importa naquele momento.

Fechar o aviso de uma versão a silencia por **7 dias**. O número é curto o bastante para não sumir
do radar e longo o bastante para não interromper uma semana de trabalho. O check manual do About
ignora a dispensa — quem clicou quer saber agora.

## Por que um dia UTC

O check automático roda no máximo uma vez por dia **UTC**, a mesma convenção do log e dos nomes de
arquivo. Usar o fuso local faria o dia mudar de lugar quando o usuário viaja ou muda o relógio, e o
O estado ficaria imprevisível. Depois de uma falha, o backoff de 1 hora evita insistir num GitHub
fora do ar.

## Por que só a janela checa

O check é uma conveniência da **janela** (`mustel gui`/`mustel-gui`). O `mustel` do terminal não
consulta o GitHub: um comando de automação não deve abrir rede nem depender da API. Quem usa só o
CLI atualiza pelo canal da release, como qualquer outro binário.

## Por que o check automático é silencioso e desligado em debug

Falha de rede no check automático não é problema do usuário e não merece toast; o manual mostra o
erro porque houve um pedido explícito. Em build `debug`, o executável em uso é o de
desenvolvimento, e substituí-lo no meio do trabalho seria hostil — por isso o check automático não
roda, e o botão do About fica disponível para testar o fluxo (com `MUSTEL_UPDATE_API` apontando
para uma release falsa).

## Por que o fallback em vez de erro

Uma instalação em `/usr/bin` (ou em qualquer pasta do sistema) não é gravável pelo processo. Falhar
seria inútil e pedir `sudo` ao app seria pior. O updater baixa para `<state>/Mustel/updates/<versão>`
e mostra o caminho no toast: o usuário decide quando e como instalar.

## Por que a janela não mostra as notas

Notas de release em HTML exigem parser, sanitização e um layout que brigue com o resto da janela. O
About tem um link para a página da release; quem quer o detalhe abre no navegador. O que a janela
precisa decidir é instalar ou não — e para isso a versão basta.

## Instalação no Windows e o updater

O instalador da 1.1.0 instala só para o usuário, em `%LOCALAPPDATA%\Programs\Mustel`. A escolha
não é estética: em `Program Files` o processo não pode trocar os próprios `.exe` sem UAC, e o
updater cairia no fallback de `<state>/Mustel/updates/<versão>` — que existe, mas é um caminho de
instalação manual. Na pasta por usuário, o helper substitui os dois `.exe` no lugar e o app
reinicia.

O setup é **distribuição**, não atualização: ele instala os dois `.exe` e os documentos, cria o
atalho e o desinstalador; o updater baixa os assets crus `mustel.exe`/`mustel-gui.exe` e **não**
baixa o setup. Por isso a release publica os três: os dois executáveis soltos (payload do updater),
o setup (primeira instalação/reparo) e o `.zip` (portátil). O `SHA256SUMS` cobre todos; o updater
ignora o que não é um dos dois nomes.

O desinstalador remove a pasta, os atalhos, o `PATH` e a entrada de desinstalação, mas preserva
`%APPDATA%\Mustel`, `%LOCALAPPDATA%\Mustel` e o Credential Manager: atualizar ou reinstalar não
pode custar a configuração, os logs ou as senhas.

## O que fica de fora

- **Assinatura de código**: o executável não é assinado. A garantia é TLS até o GitHub, o hash do
  asset e o próprio repositório. O SmartScreen do Windows pode pedir confirmação.
- **Atualização em segundo plano**: o app baixa só quando o usuário manda, e reinicia em seguida.
  Não há download silencioso nem troca sem reinício.
- **Notas e changelog na janela**, como acima.
- **macOS**: não há asset publicado; o botão não é oferecido.

Decisões de engenharia relacionadas: `D17` (canais de distribuição) em
[Decisões de engenharia](decisoes.md).
