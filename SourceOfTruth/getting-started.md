# Guia de Início Rápido (Getting Started)

Este guia fornece instruções para configurar o ambiente de desenvolvimento, compilar, executar testes e utilizar o **Mustel**.

---

## 1. Pré-requisitos

- **Rust**: Versão 1.88 ou superior (com `cargo`).
- **Sistema Operacional**: Windows 10/11, Linux ou macOS.
- **Banco de Dados**: PostgreSQL 12+ (opcional para testes com banco real).

---

## 2. Compilação e Verificação

### Verificar código sem gerar binário:
```bash
cargo check
```

### Executar a suíte de testes de unidade:
```bash
cargo test
```

### Compilar a versão de desenvolvimento:
```bash
cargo build
```

### Compilar a versão otimizada de produção:
```bash
cargo build --release
```
O binário final estará disponível em `target/release/mustel.exe` (Windows) ou `target/release/mustel` (Linux/macOS).

---

## 3. Formas de Execução

### 3.1 Executar a Interface Gráfica Desktop (GUI)
```bash
cargo run -- gui
```
> **Dica**: Na interface gráfica, você pode alterar temas no menu superior, visualizar o alerta de segurança AST enquanto digita e executar a query contra o PostgreSQL clicando em **▶ Executar Query (F5)**.

### 3.2 Executar Consultas via CLI
```bash
# Executar query inline no servidor padrão
cargo run -- query run -c "SELECT 1 as id, 'Mustel CLI' as name;"

# Exibir ajuda do comando query
cargo run -- query run --help
```

### 3.3 Gerenciar Configurações de Servidores
```bash
cargo run -- settings db-servers --help
```

---

## 4. Arquivo de Configuração (`mustel.jsonc`)

O arquivo de configuração do Mustel fica localizado por padrão em:
- **Windows**: `%LocalAppData%\Mustel\mustel.jsonc`
- **Linux/macOS**: `~/.config/mustel/mustel.jsonc`

### Exemplo de Estrutura de Configuração:
```jsonc
{
  "servers": [
    {
      "name": "localhost",
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "password": "sua_senha_aqui",
      "databases": ["postgres"],
      "fetchAllDatabases": false,
      "excludePatterns": ["template*"]
    }
  ],
  "defaults": {
    "outputFormat": "csv",
    "outputDirectory": "./results",
    "fetchAllDatabases": false,
    "requireConfirmation": true,
    "maxParallelism": 4
  }
}
```
