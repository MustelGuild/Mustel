# Teste do instalador no Linux (Wine + Inno Setup)

Este diretório é o **ambiente de teste** do instalador Windows no Linux. Ele compila um `.iss`
sintético com o `ISCC.exe` sob Wine e instala o setup gerado no prefixo Wine, tudo dentro de um
container podman, com **Xvfb** — nenhuma janela abre no desktop do dono.

> **Não é o instalador de release.** O release continua sendo construído no `windows-latest`
> (`publish-installer.ps1` + Inno Setup 6.7.1 pré-instalado). Este harness é para provar o
> toolchain/`.iss` localmente e **não é gate**.

## Uso

```bash
sh installer/wine-test/run.sh
```

O script:

1. constrói a imagem `mustel-inno-wine` (Ubuntu 24.04 pinado por digest + Wine 9 + Inno Setup
   **6.7.4 x86**, conferido por SHA-256);
2. monta este diretório em `/test` e `target/wine-test` em `/out`;
3. roda o `ISCC.exe` sobre `smoke.iss`;
4. executa o `wine-test-setup.exe` em silêncio e confere o arquivo marcador em
   `C:\WineTest\dummy.txt` do prefixo.

Saída: `target/wine-test/wine-test-setup.exe` (2,1 MB; ignorado pelo Git).

## Teste do instalador real

Depois de ter um stage (`packaging\mustel-<versão>-<triple>` do `publish.ps1` ou um stage sintético):

```bash
sh installer/wine-test/real.sh packaging/mustel-1.1.0-x86_64-pc-windows-msvc
```

Ele compila o `installer/mustel.iss` real com `WineTest` (o prefixo Wine é **win32**; o release é
**x64**), instala, confere arquivos/PATH/dados e desinstala. Aceita as mesmas variáveis do `run.sh`
(`VERSION`, `TRIPLE`, `INNO_URL` e `INNO_SHA256`). O Wine **não cria `.lnk`** e, neste
prefixo, o desinstalador **não removeu a entrada de `PATH`**; esses dois pontos ficam para a
validação no Windows (T8). O resto do fluxo — compilação, instalação, dados, `.old` e
`uninstall.exe` — é exercitado de verdade.

## Versões e licença

- O padrão é **Inno Setup 6.7.4 x86** (mesma linha do 6.7.1 do runner). O instalador x86 vem de
  `https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe` e o build
  confere o SHA-256 `a549929319f9120716677c685535ca4671e4b937b5fdc87e2585f32d8eb30e8c` antes de
  executar.
- Para testar o **7.1.1 x86** (o que o site chama de Inno Setup 7):
  ```bash
  INNO_URL=https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x86.exe \
  INNO_SHA256=03446e46f5da932e408881168c01afc59cc667cb5bc927d839c5526ace5cc0d3 \
      sh installer/wine-test/run.sh
  ```
- Ao trocar `INNO_URL`, informe o `INNO_SHA256` do asset: o hash padrão é o do 6.7.4 e o build
  falha se não casar.
- **Licença:** o Mustel é open source sem fins lucrativos; a JRSoftware não solicita licença do
  Inno Setup para uso não comercial ([FAQ](https://jrsoftware.org/isorder.php)). A
  [licença oficial](https://raw.githubusercontent.com/jrsoftware/issrc/main/license.txt) também
  permite uso comercial (“including commercial applications”); nesse caso, a compra é solicitada,
  não exigida. O uso do 6.7.4/7.1.1 no teste é gratuito.
- O `https://jrsoftware.org/download.php/is.exe` devolve uma **página HTML**, não o instalador;
  use os assets do GitHub acima.

## O que este teste prova (e o que não prova)

**Prova:** o compilador e o `.iss` rodam no Linux; o setup `.exe` gerado é um PE32 válido; a
instalação silenciosa, os arquivos, os atalhos, o `PATH` e a chave de desinstalação podem ser
inspecionados no prefixo Wine.

**Não prova:** SmartScreen, Explorer/Menu Iniciar reais, Credential Manager, locks de arquivo do
Windows e o helper/updater de verdade. Para isso, o smoke no Windows (`smoke/smoke.ps1`) e a
máquina do dono continuam sendo o juiz.

## Limpeza

```bash
podman rmi mustel-inno-wine
rm -rf target/wine-test
```
