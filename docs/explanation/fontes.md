# Fontes e licença

A janela deixa escolher a família do texto e do editor, e guarda isso na seção `gui` do
`mustel.jsonc`. Para o que cada campo faz, veja a [referência da aba Settings](../reference/settings-aba.md).

## Por que a fonte escolhida não substitui as embutidas

O egui entrega uma família como **lista ordenada**: ele procura o glifo na primeira fonte, depois na
segunda, e assim por diante. A escolha do usuário entra **na frente**; as embutidas — incluindo as
duas de emoji — continuam **atrás**.

Isso resolve um problema concreto desta interface: os botões usam `⟳`, `▼`, `📂` e `🗂`, e as fontes
de texto comuns não têm esses glifos.

## Por que os ícones das abas são desenhados

A primeira versão usava emoji (`🔍 Query`, `🖥 Servers`, `⚙ Settings`). Funcionava, mas os ícones
ficavam **acima** da linha do texto — medido na janela: centro do ícone 2,5 px acima do centro do
texto a 13 px de fonte.

A causa não é o nosso código: no Windows o emoji vem do **Segoe UI Emoji**, por fallback do sistema
(`SystemFontProvider`), e a métrica dessa fonte é dela. O `FontTweak` do epaint só alcança fonte que
nós declaramos — não alcança glifo de fallback do sistema. E o desalinhamento varia por máquina e por
versão do sistema.

Solução: os ícones das abas são **formas vetoriais** (`crates/mustel-gui/src/tabs.rs`), desenhadas
dentro de um retângulo e centradas no **mesmo** `center.y` do texto. Não depende de fonte, de sistema
nem de emoji instalado, e a cor acompanha o tema. Efeito colateral bom: o clique na aba passou a ser
um só `Response`, em vez de um `selectable_value` com texto composto.

Os símbolos **dentro do texto** (`⟳` no botão de recarregar, `📂`/`🗂` nos botões de pasta) continuam
sendo glifos com fallback — lá não há risco de desalinhamento com o texto, porque o botão centraliza o
conjunto.

## Por que há uma fonte embutida de editor

A fonte do editor precisa cobrir dois casos que o sistema pode não ter:

- máquina limpa, sem nenhuma fonte monoespaçada agradável instalada;
- resultado **igual em qualquer plataforma**, já que a janela roda em Windows e Linux.

Por isso a **Fira Code** vai embutida no binário, como opção sempre disponível. O resto das opções vem
do sistema, via `fontique` (a mesma biblioteca que o `egui` já usa para fallback de escrita).

### Licença da Fira Code

A Fira Code é distribuída sob a **SIL Open Font License 1.1**, que permite expressamente
*bundled, embedded, redistributed and/or sold with any software*. As obrigações são: manter o
copyright e o texto da licença junto de cada cópia, não vender a fonte sozinha, e não relicenciar a
fonte. O Mustel é GPL-3.0; a fonte continua OFL-1.1 — as duas convivem porque a fonte é um **asset
separado**, com a licença ao lado:

- `crates/mustel-gui/assets/fonts/FiraCode-Regular.ttf`
- `crates/mustel-gui/assets/fonts/FiraCode-OFL.txt`

## Por que o teto do tamanho é 1,25x (e não um número redondo qualquer)

O limite foi **medido**, não estimado. A linha mais larga da tela de consulta (rótulo da referência,
combo de servidor, combo de banco, `⟳` e rótulo de estado) foi medida com a fonte embutida e com as
fontes do sistema, com nomes típicos, num teste descartável:

| Fonte | Largura da linha @13 px | Teto de escala em 640 px |
|---|---|---|
| Ubuntu-Light (embutida) | 332 px | 1,77 |
| Segoe UI | 330 px | 1,77 |
| Arial | 336 px | 1,74 |
| Calibri | 302 px | 1,93 |
| **Lucida Console** (a mais larga testada) | **413 px** | **1,41** |

O limite matemático da pior fonte é 1,41; **1,25** é onde ainda sobra folga para o rótulo de estado
mais longo (`carregado outro banco`) e para nomes um pouco maiores que o típico.

Com nomes muito longos (20 caracteres), a linha já passa de 640 px **hoje**, com qualquer fonte: é
limite antigo do layout, não algo que a escolha de fonte cria.

## Por que o mínimo da janela acompanha a escala

Escalar a fonte sem escalar a janela apertaria o layout: 1,25x com janela de 880 px é o mesmo que
1,0x numa janela de 704 px. Então o mínimo vira `880×560 × escala` (a 1,25x: 1100×700). Assim a
folga relativa fica constante — o usuário nunca ganha uma janela que cabe menos coisa do que antes.

O ponto de partida era 640×480; subiu para **880×560** quando a sidebar passou a ter 400 px (a
área principal precisa de ~450 para o SQL e a tabela continuarem usáveis, e 1366×768 é o notebook
mais apertado que o dono usa). A conta tem uma ressalva conhecida: a 125% de escala do sistema
**com** `fontScale` 1,25 o mínimo passa de 1100×700 e pode não caber nesse 1366×768.

## O que não é feito

- **Não** mexemos em `pixels_per_point`: isso reescalaria a janela inteira (bordas, ícones, espaçamento)
  em vez de só o texto.
- **Não** aplicamos fonte por widget: a escolha vale para as famílias `Proportional` e `Monospace` do
  egui, e o realce de sintaxe do editor herda `Monospace` sem código novo.
- **Não** pedimos permissão ao banco: aparência é local e vale antes de qualquer conexão.
