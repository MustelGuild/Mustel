# Arquitetura e Decisões de Design (Architecture & ADRs)

Este documento registra a arquitetura do **Mustel**, o design dos módulos internos e as Decisões de Arquitetura (ADRs) fundamentais.

---

## 1. Visão Geral da Arquitetura

O Mustel adota uma arquitetura em camadas totalmente assíncrona, construída sobre o ecossistema **Tokio** e **Iced** em Rust.

```text
+-------------------------------------------------------------------------+
|                              INTERFACE                                  |
|   +---------------------------------+   +---------------------------+   |
|   |   CLI (clap 4.6 / inquire UI)   |   |  GUI (iced 0.14 - Elm MVU)|   |
|   +----------------+----------------+   +-------------+-------------+   |
+--------------------|----------------------------------|-----------------+
                     |                                  |
                     +-----------------+----------------+
                                       |
                                       v
+-------------------------------------------------------------------------+
|                            SERVIÇOS CORE                                |
|   +-----------------------+  +-------------------+  +---------------+   |
|   |  UserConfigService    |  | SqlQueryAnalyzer  |  |  CsvExporter  |   |
|   | (mustel.jsonc/config) |  | (sqlparser 0.62)  |  | (export/logs) |   |
|   +-----------------------+  +-------------------+  +---------------+   |
+--------------------------------------|----------------------------------+
                                       |
                                       v
+-------------------------------------------------------------------------+
|                        SEGURANÇA & CONEXÃO                              |
|   +---------------------------+       +-----------------------------+   |
|   |     CredentialStore       |       |         DbExecutor          |   |
|   | (Windows DPAPI / Crypt)   |       | (tokio-postgres / TLS)      |   |
|   +---------------------------+       +--------------+--------------+   |
+------------------------------------------------------|------------------+
                                                       |
                                                       v
                                            +---------------------+
                                            |  Servidores Postgres|
                                            +---------------------+
```

---

## 2. Organização dos Módulos em `src/`

```text
src/
├── main.rs                 # Ponto de entrada CLI & inicialização do logger tracing
├── error.rs                # Enum unificado de erro (MustelError) com thiserror
├── cli/                    # Parser de argumentos CLI e comandos de terminal
│   ├── mod.rs              # Definição do CliApp e enum Commands (Query, Settings, Gui)
│   ├── ui.rs               # Prompts interativos (inquire) e barras de progresso (indicatif)
│   └── commands/
│       ├── query/          # Subcomandos query run, executor e analisador AST
│       └── settings/       # Subcomandos para gerenciar servidores cadastrados
├── gui/                    # Aplicação Desktop Grafica com Iced
│   ├── mod.rs              # Boot do Iced (iced::application)
│   ├── app.rs              # MustelApp (State), enum Message, update(), view()
│   └── components/
│       ├── editor.rs       # Editor SQL usando iced::widget::text_editor
│       ├── sidebar.rs      # Painel lateral de servidores e conexões
│       └── results.rs      # Tabela responsiva e formatador de linhas Postgres
├── database/               # Camada de comunicação com o banco PostgreSQL
│   ├── mod.rs              # Exposição dos módulos de banco
│   ├── connection.rs       # DbConnectionBuilder (configuração de TLS e timeout)
│   ├── executor.rs         # DbExecutor (reconexão com retry, NoTls fallback, auto-discovery)
│   └── security.rs         # Sanitize de identificadores e caminhos
├── config/                 # Gerenciamento de arquivos de configuração
│   ├── mod.rs              # Módulo de configurações do usuário
│   ├── models.rs           # Structs de configuração (ServerConfigEntry, UserDefaults)
│   └── user_config.rs      # UserConfigService (%LocalAppData%/Mustel/mustel.jsonc)
└── security/               # Cofre de credenciais seguro
    ├── mod.rs              # Módulo de segurança
    └── credential_store.rs # Criptografia DPAPI no Windows + AES-256-GCM
```

---

## 3. Decisões de Arquitetura (ADRs)

### ADR-01: Framework de GUI — `iced` (v0.14)
- **Decisão**: Utilizar a biblioteca `iced` para a interface gráfica desktop.
- **Contexto**: Precisávamos de uma solução nativa em Rust, reativa, fortemente tipada e com suporte a temas visuais.
- **Consequência**: A arquitetura segue o modelo Elm (**Model-View-Update**). O estado (`MustelApp`) reage a mensagens fortemente tipadas (`enum Message`), gerando elementos declarativos (`Element<Message>`).

### ADR-02: Integração de Concorrência entre Iced e Tokio
- **Decisão**: Armazenar o `tokio::runtime::Handle::current()` no estado da GUI e utilizar `handle.spawn(...)` dentro de `iced::Task::perform`.
- **Contexto**: O `iced` executa tarefas de UI e `Task::perform` na sua própria pool de threads (sem o reactor I/O/Timer do Tokio ativo). Chamar temporizadores ou I/O de rede do `tokio-postgres` diretamente causava o pânico `there is no reactor running`.
- **Consequência**: Toda execução de query SQL roda de forma 100% segura na pool de threads do Tokio com seu reactor de rede ativo, retornando o resultado assincronamente para a UI sem travamentos ou travamento de telas.

### ADR-03: Análise de Segurança SQL baseada em AST — `SqlQueryAnalyzer`
- **Decisão**: Parsear consultas SQL utilizando a crate `sqlparser` (v0.62) em AST (Abstract Syntax Tree) antes da execução.
- **Contexto**: Expressões regex simples falham em identificar comandos destrutivos dentro de comentários SQL ou strings multilhas.
- **Consequência**: Identificação precisa de comandos destrutivos (`DROP`, `DELETE`, `TRUNCATE`, `ALTER`) com solicitação de confirmação explícita no CLI e badges de alerta visual em vermelho no Editor SQL da GUI.

### ADR-04: Armazenamento Seguro de Senhas via DPAPI (Windows)
- **Decisão**: Proteger credenciais de banco de dados armazenadas em disco utilizando a API nativa do Windows **DPAPI** (`CryptProtectData` / `CryptUnprotectData`) combinada com AES-256-GCM.
- **Contexto**: Evitar o armazenamento de senhas em texto puro em arquivos de configuração (`mustel.jsonc`).
- **Consequência**: As senhas são vinculadas de forma transparente ao usuário logado no SO Windows.

### ADR-05: Suporte Nativo a Temas na GUI (`iced::Theme`)
- **Decisão**: Utilizar o enum `iced::Theme` no topo do estado da aplicação GUI.
- **Contexto**: Interfaces simples costumam falhar em fornecer alternância limpa entre modo claro e escuro.
- **Consequência**: O Mustel oferece alternância em tempo real entre temas como `Dark`, `Light`, `TokyoNight`, `Dracula`, `GruvboxDark`, `GruvboxLight`, `SolarizedDark` e `SolarizedLight`, afetando instantaneamente o editor de texto, botões, painéis laterais e tabelas de resultados.
