# How-to: instalar o Mustel no Windows

> Você quer instalar o Mustel no Windows sem copiar arquivos à mão e quer um desinstalador que não
> leve junto as suas configurações. O caminho recomendado é o **setup** da release.

## Pelo instalador (recomendado)

1. Baixe `mustel-<versão>-x86_64-pc-windows-msvc-setup.exe` na
   [página de releases](https://github.com/MustelGuild/Mustel/releases/latest); o `.sha256` fica ao
   lado, se você quiser conferir o download.
2. Execute o setup. Ele instala **só para o seu usuário**, em
   `%LOCALAPPDATA%\Programs\Mustel`, e **não** pede administrador.
3. Aceite a licença (AGPL-3.0-only), escolha se quer `mustel` no `PATH` e se quer um atalho na Área
   de Trabalho. O atalho do Menu Iniciar (`Mustel`) é sempre criado e abre a janela por
   `mustel-gui.exe`, sem console.
4. No fim, deixe marcado **Launch Mustel** (ou abra pelo Menu Iniciar).

O `.exe` não é assinado; o SmartScreen pode mostrar "Windows protected your PC". Nesse caso use
**More info → Run anyway** depois de conferir o hash da release.

> O instalador **não** cria nem altera `mustel.jsonc`: o app usa os defaults até você salvar uma
> configuração. O `mustel.example.jsonc` fica na pasta de instalação como referência.

## Atualizar depois

A partir da 1.1.0 o próprio app verifica e instala atualizações: o toast `Mustel X is available` e o
botão `Update` baixam os executáveis e reiniciam. O setup serve para a **primeira instalação** e para
reparo manual; ele não é o caminho de atualização. Veja
[Atualizar o Mustel](atualizar.md) e a [referência da atualização](../reference/updates.md).

## Desinstalar

Use **Configurações → Aplicativos → Mustel → Desinstalar** ou o atalho de desinstalação no Menu
Iniciar. O desinstalador remove `{app}`, os atalhos, a entrada de `PATH` e a entrada de
desinstalação. **Não** remove:

- `%APPDATA%\Mustel\mustel.jsonc` (configuração);
- `%LOCALAPPDATA%\Mustel` (logs, relatórios e `update.json`);
- as credenciais no Credential Manager.

Se quiser apagar tudo, remova essas pastas e as entradas do cofre depois da desinstalação.

## Instalação silenciosa (automação)

```powershell
.\mustel-<versão>-x86_64-pc-windows-msvc-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-
```

Para não alterar o `PATH` numa instalação automatizada, acrescente `/TASKS=""` (nenhuma tarefa
opcional). O desinstalador aceita os mesmos `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-`.

## Se preferir o `.zip`

O pacote `mustel-<versão>-x86_64-pc-windows-msvc.zip` continua disponível. Extraia e copie
`mustel.exe`/`mustel-gui.exe` para uma pasta **gravável** e crie os atalhos à mão. O updater
funciona enquanto os dois `.exe` estiverem lado a lado nessa pasta; se ela não for gravável, ele
salva a atualização no diretório de estado e pede instalação manual.

O `cargo install --locked mustel` continua disponível, mas **não é um canal suportado pelo
instalador nem pelo updater**: o helper é pensado para a pasta do setup/zip, e um binário em
`%USERPROFILE%\.cargo\bin` pode ser sobrescrito pelo próprio `cargo install` depois. Atualize por
`cargo install --locked mustel --force` (veja
[Por que a atualização é assim](../explanation/atualizacao.md)).

## Problemas comuns

| Sintoma | O que fazer |
|---|---|
| SmartScreen bloqueia o setup | **More info → Run anyway** após conferir o SHA-256; o instalador não é assinado |
| `mustel` não aparece num terminal já aberto | Feche e reabra o terminal: o `PATH` é lido na abertura |
| O app não atualiza sozinho | Confira `Settings › About › Check for updates`; o check automático roda no máximo uma vez por dia |
| Quero instalar para todos os usuários | Não é suportado nesta entrega: a instalação é por usuário, para o updater poder trocar os `.exe` sem UAC |

Para os caminhos, arquivos e flags exatos, veja a
[referência do instalador do Windows](../reference/instalador-windows.md).
