# Referência: atualização do Mustel

> O que a janela faz quando existe uma versão nova, onde o estado fica e o que cada botão faz.
> Para o caminho de quem só quer atualizar, veja [Atualizar o Mustel](../how-to/atualizar.md); para
> o porquê das escolhas, [Por que a atualização é assim](../explanation/atualizacao.md).

A versão do produto é a de `[workspace.package] version` no `Cargo.toml` da raiz. O updater compara
essa versão com a **tag** da release estável mais recente do GitHub.

## Onde a atualização aparece

| Lugar | O que mostra |
|---|---|
| **Toast** no rodapé da janela | `Mustel X is available.` com os botões `Update` e `X`. Não expira sozinho. |
| **Toast de download** | `Downloading Mustel X...` com a barra de progresso no lugar do tempo restante. |
| **Toast de falha** | `Update failed: …` com `Retry` e `X`. |
| **Settings › About** | Botão `Check for updates` e o link `Release notes`. |
| **Barra de status** | `Checking for updates...`, `Downloading Mustel X...`, `Restarting...` ou `Update failed: …`. |

Na abertura seguinte a uma atualização aplicada, a janela mostra `Updated to X (from Y).`; se o
helper não conseguiu trocar os arquivos, mostra `The update could not be applied: …`.

## Quando o check acontece

| Situação | Comportamento |
|---|---|
| Abertura da janela | No máximo **uma vez por dia UTC**; se já checou hoje, não consulta o GitHub. |
| Falha no check automático | Silenciosa; nova tentativa só depois de **1 hora**. |
| `Check for updates` (About) | Consulta na hora, ignora o dia e o backoff, e mostra o erro (quando há). |
| Versão nova | O toast aparece e fica até o `X` ou o `Update`. |
| `X` no toast de versão disponível | A mesma versão fica silenciosa por **7 dias** (outra versão aparece na hora). |
| Build `debug` | O check automático **não roda**; o botão do About continua funcionando. |

A consulta é `GET https://api.github.com/repos/<owner>/<repo>/releases/latest`, com o repositório
derivado de `CARGO_PKG_REPOSITORY`. Em build `debug`, `MUSTEL_UPDATE_API` troca a URL (usada pelo
smoke com uma release falsa).

O check automático é **só da janela** (`mustel gui`/`mustel-gui`); o `mustel` do terminal não
consulta o GitHub.

## Estado local

`<state>/Mustel/update.json`, onde `<state>` é a pasta de estado do sistema (`~/.local/state` no
Linux; a pasta de dados local no Windows/macOS).

| Campo | Significado |
|---|---|
| `lastCheckDay` | Dia **UTC** (`YYYY-MM-DD`) do último check bem-sucedido. |
| `lastAttemptMs` | Instante da última tentativa (sucesso ou falha), em ms desde a época; alimenta o backoff. |
| `dismissedVersion` | Versão fechada no `X`. |
| `dismissedAtMs` | Quando ela foi fechada, em ms desde a época; os 7 dias contam daqui. |

Arquivo ausente ou corrompido lê como o estado padrão: a janela abre normalmente.

## Assets por plataforma

| Plataforma | Asset consumido |
|---|---|
| Linux | `mustel` (executável **cru**, sem empacotamento) |
| Windows | `mustel.exe` e `mustel-gui.exe`, quando existem ao lado do executável em uso |

O instalador `mustel-<versão>-x86_64-pc-windows-msvc-setup.exe` **não** é um payload do updater:
ele serve para a primeira instalação/reparo. O updater só procura os dois `.exe` crus; o setup e o
`.zip` são ignorados mesmo que apareçam no `SHA256SUMS`.
| macOS | Não há asset; a atualização não é oferecida |

O executável em uso precisa ser um dos alvos; um binário renomeado ou fora do pacote é recusado com
`This executable does not match a release package; update it manually.`

## Verificação e fallback

- O SHA-256 vem do campo `digest` da API do GitHub; para o que faltar, o updater baixa o
  `SHA256SUMS` da mesma release. Release sem hash é **recusada**.
- Hash ou tamanho diferente do esperado apaga o arquivo baixado e falha antes de qualquer troca.
- Se a pasta do executável **não aceita escrita** (por exemplo, uma instalação em `/usr/bin`), os
  arquivos vão para `<state>/Mustel/updates/<versão>/` e o toast avisa o caminho; nada é
  substituído e o app não reinicia.

## Aplicação e reinício

1. Todos os payloads são baixados e verificados **antes** da troca.
2. O app copia **ele mesmo** para um helper ao lado do executável
   (`.mustel-helper-<pid>[.exe]`) e o executa com `--apply-update`.
3. O helper faz backup `.old` de cada alvo, move os payloads e, se um falhar, **restaura todos**.
4. O app é relançado com os mesmos argumentos e `MUSTEL_UPDATED_FROM` (sucesso) ou
   `MUSTEL_UPDATE_FAILED` (falha).

No Windows o helper tenta a troca por até **30 s** (o executável em uso só é liberado quando o
processo antigo termina) e roda sem janela de console. No Linux o alvo é o binário `mustel`.

## Log de atividade

Cada etapa vira uma entrada no log local (aba Settings › Activity):

| Operação | Quando |
|---|---|
| `app.update.check` | Check terminou (com versão nova, sem novidade ou com erro). |
| `app.update.download` | Download iniciado ou cancelado pelo `X`. |
| `app.update.apply` | Helper disparado, update salvo para instalação manual ou falha. |

## Limites conhecidos

- O executável **não é assinado**: a confiança é TLS + GitHub + SHA-256. O SmartScreen/Defender do
  Windows pode pedir confirmação na primeira execução da versão nova.
- A janela não mostra as notas da release; o About linka a página da release.
- Instalações feitas por `cargo install` **não são um canal suportado**: o helper é pensado para a
  pasta do setup/zip, e um binário em `~/.cargo/bin` pode ser sobrescrito pelo próprio `cargo
  install` depois. Atualize com `cargo install --locked mustel --force`.
- Duas instâncias abertas: a que não está sendo trocada pode manter o arquivo travado no Windows;
  depois de 30 s o helper desfaz a troca e mostra o erro.

As decisões de superfície do updater estão no [contrato de interface](../contrato-de-interface.md),
em `IF-268`–`IF-270`, cobertas por `cargo test`.
