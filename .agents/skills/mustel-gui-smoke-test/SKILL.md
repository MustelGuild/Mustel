---
name: mustel-gui-smoke-test
description: Verifica a GUI do Mustel no Windows dirigindo a janela de verdade — teclado e mouse sintéticos, captura de tela e leitura de estado — em vez de confiar só em teste headless. Use quando precisar provar que um fluxo da GUI (autocomplete, formulários, abas, botões) funciona de fato, ou quando um bug só aparece na janela.
---

# Testar a GUI do Mustel na janela de verdade

Teste headless prova regra; **a janela prova integração**. O egui só entrega `word`, `is_open` e
clique quando há foco real e sistema de janelas por trás, e é aí que os bugs de verdade apareceram
nesta base (o `Ctrl+Space` que abria e fechava em um quadro, a fronteira de palavra invertida).

Ordem: **headless primeiro**, janela depois, e só ela quando o headless não alcança.

## 1. Antes de encostar na janela

```powershell
Stop-Process -Name mustel -ErrorAction SilentlyContinue   # senão o `cargo build` falha com
                                                          # "Access is denied" ao remover mustel.exe
Start-Sleep -Milliseconds 800
cargo build -p mustel --features gui
```

Confirme `git status`: qualquer andaime que você criar (fixture, log, foco forçado) tem de sair antes
do fim.

## 2. Headless: o que dá para provar sem janela

O harness que funciona nesta base está em `crates/mustel-gui/tests/popup_probe.rs`; copie o padrão:

- `egui::Context::default()` + `ctx.run_ui(RawInput, |ctx| CentralPanel::default().show(...))`;
- **foco**: rode um quadro vazio, depois `ctx.memory_mut(|m| m.request_focus(id))` (o id tem de ser
  **estável entre quadros** — `Id::unique("x")` é, porque usa sementes fixas);
- **entrada**: `Event::Text("a")` por caractere e `Event::Key { key, pressed: true, modifiers }`,
  um evento por quadro, como o winit entrega;
- **texturas**: sempre `output.textures_delta.clear()` antes de descartar, senão o `epaint` entra em
  pânico ("Dropped TexturesDelta with 1 unapplied deltas");
- **layout**: dá para conferir ordem de widgets lendo o que o egui pintou:
  `Shape::Text(shape) => shape.galley.text()` e `shape.pos.y` (usei em
  `the_reference_row_comes_before_the_sql_editor`).

Limite conhecido: sem foco real o `TextEdit` não tem cursor, então eventos de tecla chegam mas o
cursor fica no fim do texto. Para testar digitação de verdade, vá para a janela.

## 3. Achar e ler a janela (PowerShell)

```powershell
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$proc = Start-Process -FilePath ".\target\debug\mustel.exe" -ArgumentList "gui" -PassThru
Start-Sleep -Seconds 6
$h = [W]::FindWindow([NullString]::Value, "Mustel")   # [NullString]::Value! `$null` vira "" e não acha
[void][W]::SetForegroundWindow($h)
"foreground ok? $([W]::GetForegroundWindow() -eq $h)"
```

**Armadilhas medidas nesta máquina:**

| Sintoma | Causa |
|---|---|
| `FindWindow` devolve 0 com a janela aberta | `$null` do PowerShell vira `""`; use `[NullString]::Value` |
| janela "maximizada" reporta `2576x1408` numa tela `2560x1440`, em `(-8,-8)` | a borda de redimensionamento do Windows passa do monitor; capture por `GetWindowRect` e não estranhe |
| clique em botão não faz nada | faltou foco: chame `SetForegroundWindow` e **confirme** com `GetForegroundWindow()` antes de clicar; o primeiro clique também pode só ativar a janela |

Captura (leia o PNG **na resolução cheia** e recorte o que for pequeno — a imagem é exibida reduzida
e um `2` pode desaparecer, foi assim que li `local2` como `local`):

```powershell
$r = New-Object W+RECT; [void][W]::GetWindowRect($h, [ref]$r)
$bmp = New-Object System.Drawing.Bitmap ($r.Right-$r.Left), ($r.Bottom-$r.Top)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size); $g.Dispose()
$bmp.Save("$env:DELTA_SCRATCH_DIR\shot.png"); $bmp.Dispose()
```

Salve em `$env:DELTA_SCRATCH_DIR` e leia com `read_file` em `scratch://shot.png`. Para texto pequeno
(status, tooltip), recorte e amplie com `InterpolationMode.NearestNeighbor` antes de concluir
qualquer coisa.

> `EFRAME_SCREENSHOT_TO` **não** funciona aqui: o eframe exige a feature `__screenshot`, que esta base
> não compila. Use `CopyFromScreen`.

## 4. Digitar, clicar e apertar atalho

```powershell
# texto: SendKeys é o mais simples para frases inteiras
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait("select * from ")

# atalho com modificador: keybd_event, não SendKeys (Ctrl+Espaço é ambíguo no SendKeys)
[W]::keybd_event(0x11,0,0,[IntPtr]::Zero)          # Ctrl down
Start-Sleep -Milliseconds 150
[W]::keybd_event(0x20,0,0,[IntPtr]::Zero)          # Space down
Start-Sleep -Milliseconds 90
[W]::keybd_event(0x20,0,2,[IntPtr]::Zero)          # Space up
[W]::keybd_event(0x11,0,2,[IntPtr]::Zero)          # Ctrl up

# clique: posicione, pressione, solte — com folga entre os eventos
[void][W]::SetCursorPos($r.Left + 421, $r.Top + 253)
Start-Sleep -Milliseconds 300
[W]::mouse_event(0x0002,0,0,0,[IntPtr]::Zero)      # left down
Start-Sleep -Milliseconds 130
[W]::mouse_event(0x0004,0,0,0,[IntPtr]::Zero)      # left up
Start-Sleep -Milliseconds 800
```

Virtual keys usados: `0x11` Ctrl, `0x20` Espaço, `0x09` Tab, `0x1B` Esc, `0x2E` Delete.

## 5. Quando o fluxo precisa de dados que a máquina não tem

Não escreva no cadastro nem no cofre de quem está testando. Suba uma **build temporária** com o dado
injetado, por variável de ambiente, e remova depois:

```rust
// TEMPORÁRIO (verificação manual) — apagar depois.
if std::env::var_os("MUSTEL_FAKE_SCHEMA").is_some() {
    ui.memory_mut(|m| m.request_focus(sql_completion::id()));   // foco sem clique
}
```

Padrões que já funcionaram: injetar `Schema` fictício no lugar do carregado; começar direto na tela
ou no formulário que se quer ver (`Screen::Servers`, `form: Some(Form::new())`); escrever o estado num
arquivo de log.

**Ao terminar:** `grep -rn "TEMPOR\|MUSTEL_FAKE" crates/` tem de voltar vazio, `cargo fmt`, e
`git diff` sem resíduo. Já custou tempo esquecer isso.

Se o fluxo precisa de **dados reais**, há um servidor `Local` (`127.0.0.1:5432`, banco `dev`) no
cadastro desta máquina, com senha no cofre — dá para carregar o esquema pelo `⟳` sem andaime. O que
não se faz: apagar ou alterar os servidores do dono, e nunca escrever senha no cofre dele.

## 6. Quando a captura não explica, logue o estado

Percalço real: o `Ctrl+Space` "não funcionava" e a leitura do código estava certa. Um `writeln!` num
arquivo, por quadro, com `force`, `is_open`, `word` e `candidates`, mostrou a sequência
`is_open=true` → `is_open=false`: o popup abria e fechava em ~16 ms.

Duas lições que valem ouro:

- **instrumente o estado no ponto do desenho** antes de suspeitar de canal, repaint ou `drain`. Numa
  investigação eu quase "consertei" o que estava certo, porque li o status errado na captura.
- **prove que o teste pega a regressão**: inverta a condição de propósito, rode o teste, veja falhar,
  e desfaça. Se ele continuar verde, o teste não vale nada.

## 7. Limites da automação (não invente conclusões)

| O que falhou | O que fazer |
|---|---|
| clicar em `selectable_value` de **aba** não troca de tela, embora botões e combos ao lado respondam | **intermitente**: aconteceu em várias rodadas e funcionou em outras. Antes de acusar o app, confirme o foco (`GetForegroundWindow`) e clique **duas vezes**; se persistir, suba build temporária já na tela desejada e diga no relatório que a troca por clique não foi verificada |
| captura de tela mostra o layout, não o teclado | estado de teclado/atalho só vale com teste headless ou log |
| o app pode estar redesenhando com um quadro de atraso enquanto você captura | espere ~1 s e capture de novo; compare o status com o log antes de chamar de bug |
| `Esc` no popup do autocomplete silencia a lista **pela palavra** | se capturar depois de um `Esc`, o comportamento pode ser o do descarte, não o do gatilho |
| a imagem devolvida vem reduzida | recorte e amplie (NearestNeighbor) ou leia o PNG com ferramenta de imagem antes de tratar texto como prova |
| medir alinhamento numa captura ampliada deu número falso | duas armadilhas: o recorte incluía um separador escuro (contaminando o “fim da tinta”) e cortava descendentes. Capture **1:1**, ache a faixa do widget contando tinta por linha e só então meça o intervalo |
| achar widget por estimativa de coordenada | erra fácil: capture 1:1 e **localize o widget na imagem**, não deduza pela captura ampliada. Clique no centro do que você vê |

## 8. Fechar o ciclo

1. `Stop-Process -Name mustel` e confirme que sobrou **zero** processo (senão o próximo build falha).
2. `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
   `cargo fmt --all --check`.
3. Se a mudança é de superfície, linha no `docs/contrato-de-interface.md` apontando o teste.
4. Relatório: o que foi verificado **na janela**, o que foi verificado só por teste, e o que **não**
   foi verificado. Não troque uma coisa pela outra.
