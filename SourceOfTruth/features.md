# Guia Completo de Funcionalidades (Features)

Este documento detalha todas as funcionalidades disponíveis no **Mustel**, tanto na Interface Gráfica (GUI) quanto na Interface de Linha de Comando (CLI).

---

## 1. Interface Gráfica Desktop (GUI - Iced)

Para iniciar a interface gráfica:
```bash
mustel gui
# ou via cargo:
cargo run -- gui
```

### Recurso 1.1: Editor SQL Nativo (`text_editor`)
- **Edição de Código**: Suporta digitação com números de linha, seleção com mouse/teclado e histórico de undo/redo (`Ctrl+Z`, `Ctrl+Y`).
- **Análise AST em Tempo Real**: Conforme você digita a consulta SQL, o `SqlQueryAnalyzer` inspeciona a sintaxe e exibe um alerta no topo do editor:
  - 🟢 **`Status: ReadOnly`**: Para consultas seguras de leitura (`SELECT`).
  - ⚠️ **`ALERTA: Query Destrutiva (DROP/DELETE/TRUNCATE)`**: Emite um alerta em destaque vermelho quando a query pode modificar ou apagar dados.

### Recurso 1.2: Tabela Dinâmica de Resultados
- **Execução Real no PostgreSQL**: Ao clicar em **▶ Executar Query (F5)**, a consulta é disparada contra o servidor PostgreSQL selecionado.
- **Formatação de Dados de Qualquer Tipo**: Converte automaticamente colunas e células de tipos PostgreSQL (`TEXT`, `INT`, `FLOAT`, `BOOL`, `UUID`, `TIMESTAMP`, `JSON`, `BYTEA`, `NULL`) em formato legível de texto.
- **Feedback Metrológico**: Exibe a quantidade de linhas retornadas e o tempo exato de execução em milissegundos (ex: `Resultados (3 linhas, 12 ms)`).
- **Relatório Visual de Erros**: Se o PostgreSQL retornar um erro de sintaxe SQL ou falha de rede, a mensagem original do banco é apresentada em destaque vermelho.

### Recurso 1.3: Seletor Dinâmico de Temas Visuais
- Permite alternar instantaneamente entre temas visuais nativos no menu dropdown superior:
  - 🌙 `Dark`
  - ☀️ `Light`
  - 🌌 `TokyoNight`
  - 🧛 `Dracula`
  - 🪵 `GruvboxDark` / `GruvboxLight`
  - ☀️ `SolarizedDark` / `SolarizedLight`

### Recurso 1.4: Painel Lateral de Conexões (Sidebar)
- Lista todos os servidores configurados pelo usuário (`mustel.jsonc`).
- Permite alternar entre instâncias com um único clique, exibindo o status atual da conexão.

---

## 2. Interface de Linha de Comando (CLI)

### Recurso 2.1: Execução de Queries em Lote e Paralela (`mustel query run`)
Executa uma consulta SQL em um ou múltiplos bancos de dados e exporta os resultados diretamente para arquivos CSV.

- **Query Inline**:
  ```bash
  mustel query run -c "SELECT count(*) FROM users;" --servers "prod-db,staging-db"
  ```
- **Query via Arquivo SQL**:
  ```bash
  mustel query run -i ./queries/report.sql --all
  ```
- **Concorrência Controlada**: Utiliza semáforos assíncronos (`tokio::sync::Semaphore`) baseados na configuração `maxParallelism` para evitar sobrecarregar os servidores.

### Recurso 2.2: Exportador CSV Streaming (`CsvExporter`)
- Grava os dados diretamente no disco usando escrita com buffer (`BufWriter`), suportando exportação de grandes volumes de dados com baixo consumo de memória.
- Cria automaticamente a estrutura de diretórios por servidor e banco de dados:
  `./results/<nome_do_servidor>/<nome_do_servidor>_<banco>_<timestamp>.csv`

### Recurso 2.3: Registro de Execução Append-Only (`execution_log.json`)
- Cada execução gera uma entrada detalhada no arquivo de log `execution_log.json`:
  ```json
  {
    "timestamp": "2026-08-02T01:00:00Z",
    "server_name": "prod-db",
    "database_name": "app_production",
    "query_source": "inline query",
    "rows_affected": 1542,
    "duration_ms": 45,
    "output_file": "./results/prod-db/prod-db_app_production_20260802.csv",
    "status": "SUCCESS",
    "error_message": null
  }
  ```

---

## 3. Gerenciamento de Servidores e Descobrimento

### Recurso 3.1: Descoberta Automática de Bancos de Dados (`fetch_active_databases`)
- O Mustel consulta o catálogo de sistema do PostgreSQL (`pg_database`) para listar automaticamente todos os bancos de dados ativos no servidor.
- Filtra bancos de dados que casam com padrões configurados em `excludePatterns` (como `template*` ou `postgres`).

### Recurso 3.2: Gerenciamento Interativo de Servidores
- Adicionar ou listar servidores de banco de dados via CLI:
  ```bash
  mustel settings db-servers list
  ```

---

## 4. Cofre Criptográfico de Credenciais (`CredentialStore`)

- No Windows, as senhas registradas não ficam armazenadas em texto puro no arquivo `mustel.jsonc`.
- O Mustel utiliza a API DPAPI do Windows para criptografar as senhas usando a chave mestra do usuário do sistema operacional.
- Em caso de falha ou suporte a outros sistemas, é mantido fallback de segurança transparente.
