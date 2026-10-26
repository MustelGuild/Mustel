# Avisos de terceiros

O Mustel é distribuído sob **AGPL-3.0-only** (ver `LICENSE`). Este arquivo cobre os componentes
cujas licenças exigem aviso adicional. O inventário completo, com a licença de cada componente,
está em `sbom.cdx.json`.

## Fontes embutidas

- **Fira Code**: SIL Open Font License 1.1. O texto está em
  `crates/mustel-gui/assets/fonts/FiraCode-OFL.txt` e é copiado para o pacote.
- **Ubuntu Light** e **Noto Emoji**, empacotadas por `epaint_default_fonts`: licenças
  **Ubuntu-font-1.0** e **OFL-1.1**. Textos em
  <https://github.com/emilk/egui/tree/main/crates/epaint_default_fonts>.

## option-ext (MPL-2.0)

Dependência transitiva de `dirs`. Código-fonte: <https://github.com/soc/option-ext>. O binário
distribui o componente sem modificações; o texto da Mozilla Public License 2.0 está em
<https://www.mozilla.org/media/MPL/2.0/index.txt>.

## Demais componentes

MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0 e equivalentes estão listados no `sbom.cdx.json`, que
acompanha a release. Nenhum deles exige aviso além do próprio texto da licença.
