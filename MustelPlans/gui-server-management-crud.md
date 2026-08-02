# Plano — Navegação por Abas e CRUD de Servidores na GUI (Iced)

> **Status:** concluído
> **Criado em:** 2026-08-02
> **Atualizado em:** 2026-08-02
> **Complexidade:** média
> **Depende de (outros planos):** `gui-iced-implementation.md`

---

## 1. Visão Geral

Este plano descreve a adição de um sistema de navegação por **Abas no Header** da interface gráfica desktop (Iced v0.14) do **Mustel**, dividindo a aplicação em dois módulos principais:
1. **Aba 1 (Execução de Queries)**: Interface existente com Editor SQL, barra lateral de conexões ativas e tabela dinâmica de resultados real.
2. **Aba 2 (Gerenciamento de Servidores / CRUD)**: Nova interface dedicada ao cadastro, edição, teste de conexão em tempo real (`DbExecutor::connect`) e exclusão de servidores PostgreSQL persistidos em `mustel.jsonc` (com senhas criptografadas via DPAPI).

---

## 2. Escopo

**Inclui:**
- Enum `ActiveTab` no estado da aplicação `MustelApp` para alternância de visualização.
- Botões de alternância de abas no Header superior (`[ 🔍 Executar Queries ]` | `[ ⚙️ Gerenciar Servidores ]`).
- Novo componente visual `src/gui/components/server_manager.rs` para renderizar o painel CRUD de conexões.
- Formulário com campos: Nome Amigável, Host/IP, Porta, Usuário, Senha, Banco Padrão e Modo SSL (`Disable`, `Allow`, `Prefer`, `Require`).
- Botão **🔌 Testar Conexão** no formulário para validar credenciais no PostgreSQL em segundo plano via `iced::Task::perform`.
- Botões **💾 Salvar** (cria/atualiza servidor) e **🗑️ Excluir** (remove servidor).
- Sincronização em tempo real da lista de servidores salvos entre a Aba 2 (CRUD) e a Aba 1 (Sidebar).

**Exclui explicitamente:**
- Suporte a múltiplos tipos de bancos de dados no v1 (foco exclusivo em PostgreSQL).
- Importação/Exportação em lote de arquivos de conexão `.json` externos (apenas persistência no `mustel.jsonc`).

---

## 3. Decisões

| #  | Decisão | Escolha | Razão |
|----|---------|---------|-------|
| D1 | Posicionamento das Abas | Topo (Header Superior) | Mantém a navegação visível, limpa e alinhada com padrões modernos de desktop. |
| D2 | Validação de Conexão | Botão "🔌 Testar Conexão" | Permite validar host, porta e autenticação no Postgres antes de persistir as configurações. |
| D3 | Armazenamento de Senhas | Criptografia DPAPI / `UserConfigService` | Mantém credenciais protegidas nativamente sem salvar texto puro no arquivo `mustel.jsonc`. |
| D4 | Reatividade das Abas | Estado Elm Reativo (`ActiveTab`) | Sincroniza instantaneamente mudanças de servidores na Aba 2 com a Sidebar da Aba 1. |

---

## 4. Arquitetura / Design

```text
+-----------------------------------------------------------------------------------------+
|  Mustel Database Toolkit    [ 🔍 Executar Queries ]   [ ⚙️ Gerenciar Servidores ]  [ Dark 🔻]|
+-----------------------------------------------------------------------------------------+
                                          |
                        +-----------------+-----------------+
                        |                                   |
                        v                                   v
             +--------------------+               +--------------------+
             | Aba 1: Executar    |               | Aba 2: CRUD de     |
             | Queries (Existente)|               | Servidores (Novo)  |
             +--------------------+               +--------------------+
```

### Tipos / arquivos novos
- `src/gui/components/server_manager.rs` — Estado do formulário de servidor (`ServerFormState`), validação de campos e renderização do painel CRUD.

### Mudanças em tipos existentes
- `src/gui/app.rs`:
  - Adição do enum `ActiveTab` (`QueryExecutor`, `ServerManagement`).
  - Adição das mensagens `TabSelected(ActiveTab)`, `ServerFormChanged`, `TestServerConnection`, `SaveServer`, `DeleteServer` no `enum Message`.
- `src/gui/components/sidebar.rs`:
  - Método `reload_servers(&mut self, servers: Vec<ServerConfigEntry>)` para atualizar a lista reativamente ao salvar/remover um servidor.
- `src/gui/components/mod.rs`:
  - Exposição do submódulo `pub mod server_manager;`.

---

## 5. Tarefas

### Fase 1 — Estrutura de Abas & Navegação

- [x] **T1** — Adicionar enum `ActiveTab` e atualizar `MustelApp` e `enum Message` em `src/gui/app.rs`
  - Depende de: —
  - Paralelizável com: —
  - Concluído quando: Botões no Header alternam reativamente a tela exibida entre Aba 1 e Aba 2.
  - Arquivos: `src/gui/app.rs`

### Fase 2 — Componente CRUD de Servidores (`server_manager.rs`)

- [x] **T2** — Criar struct `ServerFormState` e formulário de campos em `src/gui/components/server_manager.rs`
  - Depende de: T1
  - Paralelizável com: —
  - Concluído quando: É possível digitar Nome, Host, Porta, Usuário, Senha, Banco Padrão e selecionar Modo SSL.
  - Arquivos: `src/gui/components/server_manager.rs`, `src/gui/components/mod.rs`

- [x] **T3** — Implementar lógica do botão **🔌 Testar Conexão** via `iced::Task::perform`
  - Depende de: T2
  - Paralelizável com: —
  - Concluído quando: Clicar em Testar Conexão executa o teste assíncrono com o PostgreSQL em background e exibe sucesso em verde ou erro detalhado em vermelho.
  - Arquivos: `src/gui/app.rs`, `src/gui/components/server_manager.rs`

- [x] **T4** — Implementar salvamento de servidor via `UserConfigService` (com criptografia DPAPI)
  - Depende de: T2
  - Paralelizável com: T5
  - Concluído quando: Clicar em **💾 Salvar** grava ou atualiza a entrada do servidor no `mustel.jsonc` com senha protegida.
  - Arquivos: `src/gui/app.rs`, `src/gui/components/server_manager.rs`

- [x] **T5** — Implementar exclusão de servidor (**🗑️ Excluir**) via `UserConfigService`
  - Depende de: T2
  - Paralelizável com: T4
  - Concluído quando: Clicar em Excluir remove o servidor do `mustel.jsonc` e limpa o formulário.
  - Arquivos: `src/gui/app.rs`, `src/gui/components/server_manager.rs`

### Fase 3 — Sincronização entre Abas & Polimento

- [x] **T6** — Sincronizar a lista de servidores salvos com a barra lateral (`ConnectionSidebarState`) da Aba 1
  - Depende de: T4, T5
  - Paralelizável com: —
  - Concluído quando: Salvar ou excluir um servidor na Aba 2 atualiza instantaneamente os botões da barra lateral na Aba 1.
  - Arquivos: `src/gui/app.rs`, `src/gui/components/sidebar.rs`

- [x] **T7** — Validação com `cargo check` e `cargo test`
  - Depende de: T6
  - Paralelizável com: —
  - Concluído me: `cargo check` compila limpo sem erros e todos os 14+ testes passam.
  - Arquivos: `Cargo.toml`, `src/gui/`

---

## 6. Estratégia de Verificação

- **Build:** `cargo check` — sem erros ou warnings.
- **Testes:** `cargo test` — 14+ testes de unidade passando.
- **Smoke test:**
  1. Executar `cargo run -- gui`.
  2. Alternar para a aba **⚙️ Gerenciar Servidores**.
  3. Preencher um novo servidor (ex: `teste-local`, `127.0.0.1`, `5432`, `postgres`).
  4. Clicar em **🔌 Testar Conexão** e verificar o resultado.
  5. Clicar em **💾 Salvar**.
  6. Alternar de volta para a aba **🔍 Executar Queries** e confirmar que o novo servidor aparece na barra lateral.

---

## 7. Riscos e Mitigação

| Risco | Prob | Impacto | Mitigação |
|-------|------|---------|-----------|
| Senhas salvas em texto claro no formulário | Média | Alto | Utilizar o método de criptografia DPAPI `UserConfigService::set_encrypted_password`. |
| Dessincronização entre o estado da Aba 2 e a Sidebar da Aba 1 | Baixa | Médio | Atualizar a lista `server_list` no estado da Sidebar em cada evento de salvamento/exclusão. |

---

## 8. Questões em Aberto

- [ ] N/A — Todas as decisões de posicionamento, teste de conexão e formulário foram alinhadas e travadas durante a exploração.

---

## 9. Log de Execução

- `[2026-08-02 01:12]` Plano criado em estado de `rascunho`.
- `[2026-08-02 01:12]` Status alterado para `em-andamento`.
- `[2026-08-02 01:13]` T1 a T7 concluídas — Sistema de navegação por abas (`ActiveTab`) implementado no Header. Formulário CRUD (`server_manager.rs`), teste de conexão assíncrono via Tokio (`DbExecutor::connect`), salvamento criptografado via DPAPI e sincronização em tempo real com a Sidebar finalizados com sucesso.
- `[2026-08-02 01:13]` Plano concluído — `cargo check` e `cargo test` 100% aprovados.
