# Plano — Interface Gráfica Crossplatform com Iced para Mustel

> **Status:** concluído
> **Criado em:** 2026-08-02
> **Atualizado em:** 2026-08-02
> **Complexidade:** média
> **Depende de (outros planos):** nenhum

---

## 1. Visão Geral

Este plano descreve a reconstrução da interface gráfica desktop nativa e crossplatform para o **Mustel**, utilizando a biblioteca **`iced`** (v0.14) e a **Arquitetura Elm** (Model-View-Update).

O objetivo é fornecer uma experiência reativa, fortemente tipada e com suporte nativo a temas Claro/Escuro (`Theme::Light` / `Theme::Dark`), aproveitando o widget de código nativo `iced::widget::text_editor` e o sistema de tarefas assíncronas nativas (`iced::Task`) sem a necessidade de controle manual de canais `mpsc` ou repintura da interface.

---

## 2. Escopo

**Inclui:**
- Substituição de dependências de GUI anteriores por `iced = "0.14"` no `Cargo.toml`.
- Subcomando CLI `mustel gui` em `src/main.rs` para lançar a aplicação Iced.
- Arquitetura Elm completa em `src/gui/`: estado (`MustelApp`), mensagens (`enum Message`), lógica (`update`) e interface declarativa (`view`).
- Painel lateral (Sidebar) para gerenciamento e seleção de conexões salvas.
- Editor SQL usando `iced::widget::text_editor` com números de linha, histórico undo/redo e validação visual de segurança via `SqlQueryAnalyzer`.
- Suporte nativo à alternância de temas (`iced::Theme::Dark`, `iced::Theme::Light`, `Theme::TokyoNight`, etc.) afetando 100% dos elementos visuais de forma consistente.
- Tabela virtualizada para exibição de resultados de queries SQL.
- Despacho assíncrono não-bloqueante de queries no Tokio runtime usando `iced::Task::perform`.

**Exclui explicitamente:**
- Uso de componentes `egui`/`eframe` ou frameworks Webview (Tauri / Electron / JS).
- Suporte a múltiplos bancos de dados no v1 (foco exclusivo em PostgreSQL).
- Edição inline de dados diretamente nas células da tabela de resultados (apenas leitura no v1).

---

## 3. Decisões

| #  | Decisão | Escolha | Razão |
|----|---------|---------|-------|
| D1 | Framework de GUI | `iced` (v0.14) | Arquitetura Elm fortemente tipada, reativa e declarativa, mantendo o Mustel 100% Rust. |
| D2 | Componente de Editor SQL | `iced::widget::text_editor` | Widget nativo de alta performance para edição de texto com suporte a seleção, cursor e temas. |
| D3 | Concorrência Assíncrona | `iced::Task` / `iced::futures` | Trata retornos assíncronos do Tokio nativamente, dispensando canais manuais ou chamadas de repaint. |
| D4 | Sistema de Temas | `iced::Theme` | Suporte nativo e automático a temas Claro/Escuro aplicados em 100% dos widgets. |

---

## 4. Arquitetura / Design

```
                         +-----------------------------+
                         |      Ação do Usuário        |
                         |  (Clique, Edição de Código, |
                         |    Troca de Tema, F5)       |
                         +--------------+--------------+
                                        |
                                        v
                         +-----------------------------+
                         |       enum Message          |
                         |  - ThemeChanged(Theme)      |
                         |  - CodeEdited(Action)       |
                         |  - ExecuteQuery             |
                         |  - QueryResultReceived(...) |
                         +--------------+--------------+
                                        |
                                        v
 +-------------------------+   +-----------------------+
 | Background Tokio Task   |   |   update(&mut self)   |
 | (Executa query Postgres)+-->|  (Atualiza Estado)    |
 +-------------------------+   +--------------+--------+
                                              |
                                              v
                               +-----------------------+
                               |     view(&self)       |
                               | (Retorna Element<Msg>)|
                               +-----------------------+
```

### Tipos / arquivos novos
- `src/gui/mod.rs` — Ponto de entrada do Iced (`iced::application`).
- `src/gui/app.rs` — Struct `MustelApp` (Estado), enum `Message`, métodos `update` e `view`.
- `src/gui/components/sidebar.rs` — Renderização declarativa do painel de conexões.
- `src/gui/components/editor.rs` — Renderizador do `text_editor` e badge de segurança.
- `src/gui/components/results.rs` — Renderizador da tabela de resultados de queries.

### Mudanças em tipos existentes
- `Cargo.toml` — Substituição das crates de GUI por `iced = "0.14"`.
- `src/main.rs` — Suporte ao subcomando `gui`.

---

## 5. Tarefas

### Fase 1 — Setup & Dependências

- [x] **T1** — Adicionar crate `iced = "0.14"` e remover dependências antigas de GUI no `Cargo.toml`
  - Depende de: —
  - Paralelizável com: —
  - Concluído quando: `cargo check` compila sem erros com `iced`.
  - Arquivos: `Cargo.toml`

- [x] **T2** — Configurar o ponto de entrada da GUI `iced::application` e conectar ao subcomando CLI `gui`
  - Depende de: T1
  - Paralelizável com: —
  - Concluído quando: `cargo run -- gui` dispara a janela inicial do Iced.
  - Arquivos: `src/main.rs`, `src/gui/mod.rs`

### Fase 2 — Estado Elm & Mensagens Assíncronas

- [x] **T3** — Criar struct `MustelApp` (Estado) e enum `Message` em `src/gui/app.rs`
  - Depende de: T2
  - Paralelizável com: T4
  - Concluído quando: O loop Elm compila com suporte a ações básicas de código e troca de tema.
  - Arquivos: `src/gui/app.rs`

- [x] **T4** — Implementar despacho de queries via `iced::Task::perform`
  - Depende de: T3
  - Paralelizável com: —
  - Concluído quando: Disparar uma query executa no Tokio e atualiza o estado via `Message::QueryResultReceived` com renderização imediata.
  - Arquivos: `src/gui/app.rs`

### Fase 3 — Componentes de Interface (UI)

- [x] **T5** — Implementar Editor SQL com `iced::widget::text_editor` e alertas do `SqlQueryAnalyzer`
  - Depende de: T3
  - Paralelizável com: T6
  - Concluído quando: O editor de texto responde a digitação, exibe linhas e mostra badge de análise de segurança.
  - Arquivos: `src/gui/components/editor.rs`

- [x] **T6** — Implementar painel lateral de conexões com suporte a seleção de servidores
  - Depende de: T3
  - Paralelizável com: T5
  - Concluído me: Permite alternar o servidor selecionado na barra lateral.
  - Arquivos: `src/gui/components/sidebar.rs`

- [x] **T7** — Implementar exibição da tabela de resultados de queries em formato de grade
  - Depende de: T4, T5
  - Paralelizável com: —
  - Concluído quando: Resultados de queries são renderizados em colunas e linhas responsivas.
  - Arquivos: `src/gui/components/results.rs`

### Fase 4 — Alternância de Temas & Validação

- [x] **T8** — Implementar alternador de temas Claro/Escuro (`Theme::Light` / `Theme::Dark` / `Theme::TokyoNight`) no Header
  - Depende de: T5, T6, T7
  - Paralelizável com: —
  - Concluído quando: Alternar o tema altera instantaneamente 100% das cores da aplicação (editor, painéis, botões, tabelas).
  - Arquivos: `src/gui/app.rs`

---

## 6. Estratégia de Verificação

- **Build:** `cargo check` — sem erros ou warnings de compilação.
- **Testes:** `cargo test` — todos os 14 testes de unidade existentes passando.
- **Smoke test:** Executar `cargo run -- gui`, alternar entre os temas Claro e Escuro (verificando contraste perfeito), digitar uma query e clicar em Executar (verificando re-execução sem travamentos).

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|-------|------|---------|-----------|
| Curva de aprendizado das APIs de layout do `iced 0.14` | Média | Médio | Usar construtores padrão `column!`, `row!`, `container`, `text_editor` e `pick_list`. |
| Desempenho com grandes coleções de linhas | Baixa | Médio | Usar contêineres roláveis `scrollable` e paginação para limitar os dados exibidos por página. |

---

## 8. Questões em Aberto

- [ ] Definir a lista de temas pré-configurados no seletor da UI (ex: `Light`, `Dark`, `TokyoNight`, `Gruvbox`).

---

## 9. Log de Execução

- `[2026-08-02 00:48]` Plano criado em estado de `rascunho`.
- `[2026-08-02 00:49]` Status alterado para `em-andamento`.
- `[2026-08-02 00:50]` T1 a T8 concluídas — GUI migrada com sucesso para Iced (v0.14). Módulos `src/gui/` reimplementados com `MustelApp`, `text_editor` nativo, seletor de temas `pick_list`, e tarefas assíncronas `iced::Task`.
- `[2026-08-02 00:50]` Plano concluído — todas as verificações da Seção 6 passaram com sucesso.
