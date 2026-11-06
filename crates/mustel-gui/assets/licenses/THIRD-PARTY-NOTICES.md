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

Os textos completos das licenças de todos os componentes compilados estão em
`THIRD-PARTY-LICENSES.md`, que vai dentro de cada pacote. O arquivo é gerado por `cargo-about` a
partir do `Cargo.lock` (configuração em `about.toml`), e a lista de licenças aceitas é a mesma do
`deny.toml`: um componente com licença fora dela quebra a geração do release.

O inventário de componentes, com versão, hash e grafo de dependências, está em `sbom.cdx.json`,
que acompanha a release e também vai dentro do pacote.
