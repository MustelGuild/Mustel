# Referência: instalador do Windows

> Descreve o artefato de instalação do Windows, onde ele põe cada arquivo e o que ele toca no
> sistema. Para o passo a passo, veja [Instalar no Windows](../how-to/instalar-no-windows.md); para
> o porquê das escolhas, [Por que a atualização é assim](../explanation/atualizacao.md) e
> [Decisões de engenharia](../explanation/decisoes.md) (`D31`).

## Artefato

| Item | Valor |
|---|---|
| Nome | `mustel-<versão>-x86_64-pc-windows-msvc-setup.exe` |
| Formato | executável único (Inno Setup) |
| Plataforma | Windows 10 ou 11, x64 |
| Privilégios | **por usuário** (`PrivilegesRequired=lowest`), sem UAC |
| Diretório | `%LOCALAPPDATA%\Programs\Mustel` (fixo; a página de diretório não aparece) |
| Hash | `...-setup.exe.sha256` ao lado, e a linha `./mustel-...-setup.exe` no `SHA256SUMS` da release |
| Assinatura | **não assinado** nesta entrega; o SmartScreen pode avisar |

## Arquivos instalados

| Arquivo | Papel |
|---|---|
| `mustel.exe` | CLI (também abre a janela sem subcomando num terminal) |
| `mustel-gui.exe` | entrada gráfica sem console; é o alvo do atalho do Menu Iniciar |
| `mustel.ico` | ícone do setup, dos atalhos e da entrada de desinstalação |
| `mustel.example.jsonc` | exemplo de configuração (não é copiado para `%APPDATA%`) |
| `LICENSE` | AGPL-3.0-only, mostrada na página de licença |
| `THIRD-PARTY-NOTICES.md` | avisos curtos de terceiros (inclui a Fira Code) |
| `THIRD-PARTY-LICENSES.md` | textos completos das licenças dos componentes compilados |
| `sbom.cdx.json` | SBOM CycloneDX do pacote |
| `README.txt` | instruções curtas do pacote |

O pacote `.zip` do Windows tem o mesmo conteúdo (sem o `.ico` do setup). O pacote Linux é o
`.tar.gz`, que traz `mustel.png` e `mustel.desktop` além do binário e dos documentos.

## Atalhos, PATH e desinstalação

| Efeito | Onde |
|---|---|
| Atalho `Mustel` → `mustel-gui.exe` | `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Mustel` |
| Atalho de desinstalação | mesma pasta do Menu Iniciar |
| Atalho opcional na Área de Trabalho | `%USERPROFILE%\Desktop` (tarefa `desktopicon`, desmarcada por padrão) |
| Entrada de desinstalação | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{A73ECE3A-F2C1-45B5-A5F0-A3EE4D7BB91B}_is1` (criada pelo Inno) |
| `PATH` opcional | `HKCU\Environment\Path` (tarefa `addtopath`, marcada na primeira instalação) |

O `PATH` é alterado **só** na chave do usuário, sem duplicar a entrada, e gravado como
`REG_EXPAND_SZ`: entradas com `%VAR%` que o usuário já tinha continuam expandindo. O desinstalador
remove apenas o diretório do Mustel e preserva o resto do valor. O `AppId` do instalador é um GUID
fixo
(`A73ECE3A-F2C1-45B5-A5F0-A3EE4D7BB91B`): é o que permite detectar a instalação anterior e fazer
upgrade/desinstalação.

## Dados que o instalador não toca

| Dado | Caminho |
|---|---|
| Configuração | `%APPDATA%\Mustel\mustel.jsonc` |
| Logs e relatórios | `%LOCALAPPDATA%\Mustel\logs`, `%LOCALAPPDATA%\Mustel\reports` |
| Estado do updater | `%LOCALAPPDATA%\Mustel\update.json` |
| Credenciais | Credential Manager, serviço do Mustel |

O desinstalador remove a pasta de instalação, os atalhos, a entrada de `PATH` e a entrada de
desinstalação; essas quatro áreas continuam. Para apagar tudo, remova-as manualmente e apague as
credenciais no Credential Manager.

## Instalação silenciosa

```powershell
.\mustel-<versão>-x86_64-pc-windows-msvc-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-
```

| Flag | Efeito |
|---|---|
| `/VERYSILENT` | sem wizard e sem progresso |
| `/SUPPRESSMSGBOXES` | não mostra caixas de erro/confirmação |
| `/NORESTART` | nunca reinicia o Windows |
| `/SP-` | não pede confirmação inicial |
| `/TASKS=""` | não executa as tarefas opcionais (Área de Trabalho e `PATH`) |
| `/LOG=<arquivo>` | grava o log do setup |

O desinstalador (`unins000.exe` na pasta de instalação) aceita as mesmas flags. O código de saída
`0` indica sucesso; erros de instalação/desinstalação saem diferentes de zero.

## Upgrade e app em execução

O setup detecta a instalação anterior pelo `AppId` e usa o diretório anterior
(`UsePreviousAppDir=yes`), preservando as tarefas escolhidas (`UsePreviousTasks=yes`). Com
`CloseApplications=yes`, o Inno pede ao Restart Manager para fechar `mustel.exe`/`mustel-gui.exe`
antes de substituir os arquivos; a GUI é reaberta pelo atalho/`Run`. Se a pasta escolhida for
somente leitura (não é a padrão), o **updater** da 1.1.0 cai no fallback
`%LOCALAPPDATA%\Mustel\updates\<versão>` e pede instalação manual.

## Relação com o updater

O instalador é **distribuição**, não atualização. O updater da janela consulta o GitHub, baixa os
assets crus `mustel.exe` e `mustel-gui.exe` e os troca na pasta de instalação; ele **não** baixa o
setup. O setup também pode aparecer na release e no `SHA256SUMS`, mas o updater o ignora (ele só
procura os dois nomes de executável). O `.zip` portátil funciona do mesmo jeito, desde que a pasta
seja gravável e os dois `.exe` estejam lado a lado.

## Limitações

- **Sem assinatura de código**: o SmartScreen pode avisar; a assinatura é um follow-up (ver
  [Atualização](../explanation/atualizacao.md) e `D31`).
- **Sem winget/MSIX/Store**: os canais são GitHub Releases e crates.io (`D17`/`D22`/`D31`).
- **Somente por usuário**: não há instalação per-machine em `Program Files`; o updater precisa da
  pasta gravável.
- **Windows 10+ x64**: não há instalador ARM64 nem macOS/Linux.
- **Não cria `mustel.jsonc`**: o exemplo fica na pasta de instalação; o app usa defaults até você
  salvar.
- **Sem atalho de terminal**: o Menu Iniciar aponta para `mustel-gui.exe`; o `mustel.exe` vai para
  o `PATH` quando a tarefa é marcada.
