<div align="center">
  <img src="assets/FurLab_icon.png" alt="FurLab Logo" width="160" />

  # FurLab

  Uma poderosa ferramenta CLI .NET para automatizar tarefas comuns de desenvolvimento.
</div>

---

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/K3K41AP4RO)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-yellow?logo=buymeacoffee&logoColor=white)](https://www.buymeacoffee.com/liphvf)

> **Reescrita em Rust em andamento** (branch `feat/rust-migration`). Os grupos `query`, `database` e
> `settings` estão sendo reescritos em Rust como **Mustel** — binário `mustel`, instalável via
> `cargo install mustel` ou winget (`Mustel.CLI`). A implementação .NET agora vive em `legacy/`:
> é o **oráculo de paridade** durante a migração e continua sendo o produto dos demais grupos
> (`file`, `claude`, `opencode`, `winget`, `windowsfeatures`, `docker`) até a segunda onda.
> O restante deste documento ainda descreve o CLI .NET.


## Descrição

FurLab é uma interface de linha de comando (CLI) multiplataforma construída com .NET que ajuda desenvolvedores a automatizar tarefas repetitivas de desenvolvimento. Ela fornece comandos para operações de banco de dados, gerenciamento de arquivos, instalação de ferramentas de IA (Claude Code, OpenCode) e gerenciamento de pacotes Windows.

> **Nota**: Este é um projeto hobby criado para uso pessoal. Pode não seguir todas as melhores práticas ou ter testes abrangentes. Contribuições e feedback são bem-vindos, mas por favor tenha em mente que isso foi criado para resolver as necessidades específicas do autor.

## Problema que Resolve

Desenvolvedores frequentemente executam tarefas repetitivas que podem ser automatizadas:
- Combinar múltiplos arquivos em um só
- Instalar e configurar ferramentas de IA para desenvolvimento
- Fazer backup e restaurar pacotes do Windows
- Executar queries SQL em múltiplos servidores PostgreSQL

FurLab consolida essas tarefas em uma única ferramenta CLI fácil de usar.

## Principais Funcionalidades

- **Database Backup**: Faz backup de bancos de dados PostgreSQL usando pg_dump
- **Execução de Queries**: Executa queries SQL em múltiplos servidores PostgreSQL com seleção interativa, execução paralela e exportação CSV
- **Guard Rail para Queries Destrutivas**: Detecção automática de INSERT, UPDATE, DELETE, ALTER, DROP, etc. com prompt de confirmação
- **Gerenciamento de Servidores**: Adiciona, lista, remove e testa servidores PostgreSQL via CLI
- **File (Combine)**: Combina múltiplos arquivos em um único
- **Integração com Claude Code**: Instala e configura CLI do Claude Code
- **Integração com OpenCode**: Instala e configura CLI do OpenCode
- **Gerenciador Winget**: Faz backup e restaura pacotes do gerenciador de pacotes Windows

## Tecnologias

- **Framework**: .NET 10
- **Linguagem**: C#
- **CLI Parsing**: System.CommandLine
- **Banco de Dados**: Npgsql (PostgreSQL)
- **Configuração**: JSONC (furlab.jsonc)
- **UI**: Spectre.Console

## Instalação

### Pré-requisitos

- .NET SDK 10 ou superior
- Windows (necessário para comandos Claude, OpenCode e Winget)

### Instalar como Ferramenta .NET

```bash
dotnet tool install --global FurLab
```

Ou instalar pelo NuGet:

```bash
dotnet tool install -g FurLab
```

### Compilar a Partir do Código Fonte

```bash
git clone https://github.com/seu-repositorio/FurLab.git
cd FurLab
dotnet restore
dotnet build
```

## Como Executar Localmente

```bash
dotnet run -- --help
```

## Exemplos de Uso Básico

### Execução de Queries

```bash
# Executar arquivo SQL nos servidores selecionados
FurLab query run --input query.sql

# Executar query inline
FurLab query run --command "SELECT * FROM users"

# Executar com diretório de saída
FurLab query run -i query.sql -o ./results

# Gerar um CSV por servidor
FurLab query run -i query.sql --separate-files
```

Na execução, FurLab exibe um prompt interativo de seleção de servidores (todos pré-selecionados). Os resultados são exportados para CSV com colunas `Server, Database, <colunas da query>`.

### Gerenciamento de Servidores

```bash
# Listar servidores configurados
FurLab settings db-servers ls

# Adicionar servidor interativamente
FurLab settings db-servers add -i

# Adicionar servidor com flags
FurLab settings db-servers add -n dev -h localhost -p 5432 -U postgres -W minhasenha

# Testar conexão com servidor
FurLab settings db-servers test -n dev

# Remover servidor
FurLab settings db-servers rm -n dev
```

### Database Backup

```bash
# Backup com configurações padrão
FurLab database backup meubanco

# Backup com configurações personalizadas
FurLab database backup meubanco --host localhost --port 5432 --username postgres --password minhasenha

# Backup com caminho de saída personalizado
FurLab database backup meubanco -o "C:\backups\meubanco.backup"
```

### Operações de Arquivo

```bash
# Combinar múltiplos arquivos em um só
FurLab file combine -i "C:\temp\*.sql" -o "C:\temp\resultado.sql"

# Converter encoding de arquivos (detecção automática da origem)
FurLab file convert-encoding -i "src/**/*.cs" --to UTF-8

# Converter com encoding de origem explícito e criar backup
FurLab file convert-encoding -i "*.txt" --from Latin1 --to UTF-8 --backup

# Converter e salvar em um diretório diferente
FurLab file convert-encoding -i "docs/*.md" --to UTF-8 -o "converted_docs/"
```

### Instalar Claude Code

```bash
FurLab claude install
```

### Backup/Restaurar Winget

```bash
FurLab winget backup -o "C:\backup"
FurLab winget restore -i "C:\backup\backup-winget.json"
```

## Configuração

FurLab armazena a configuração do usuário em `%LocalAppData%\FurLab\furlab.jsonc` (formato JSONC com suporte a comentários).

### Exemplo furlab.jsonc

```jsonc
{
  // Configuração de servidores PostgreSQL
  "servers": [
    {
      "name": "dev",           // Identificador único
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "password": "minhasenha",
      "databases": ["meubanco", "app_dev"],
      "sslMode": "Prefer",
      "timeout": 30,
      "commandTimeout": 300,
      "maxParallelism": 4
    },
    {
      "name": "prod",
      "host": "prod-db.empresa.com",
      "port": 5432,
      "username": "readonly",
      "password": "secreta",
      "fetchAllDatabases": true,
      "excludePatterns": ["template*", "postgres"],
      "sslMode": "Require"
    }
  ],
  // Configurações padrão
  "defaults": {
    "outputFormat": "csv",
    "outputDirectory": "./results",
    "fetchAllDatabases": false,
    "requireConfirmation": true,
    "maxParallelism": 4
  }
}
```

## Lista de Comandos

| Comando | Descrição |
|---------|-----------|
| `query run` | Executa queries SQL e exporta para CSV |
| `settings db-servers ls` | Lista servidores configurados |
| `settings db-servers add` | Adiciona servidor (interativo ou com flags) |
| `settings db-servers rm` | Remove servidor |
| `settings db-servers test` | Testa conexão com servidor |
| `database backup` | Faz backup de banco de dados PostgreSQL |
| `file combine` | Combina múltiplos arquivos em um único |
| `claude` | Integração com Claude Code |
| `opencode` | Integração com CLI do OpenCode |
| `winget` | Gerenciador de pacotes Windows |

## Detalhes do Comando Query

### Opções

| Opção | Descrição |
|-------|-----------|
| `-i, --input <arquivo>` | Arquivo SQL de entrada |
| `-c, --command <sql>` | Query SQL inline (mutuamente exclusivo com `-i`) |
| `-o, --output <caminho>` | Arquivo ou diretório de saída |
| `--separate-files` | Um CSV por servidor (padrão: arquivo consolidado) |
| `--all, -a` | Consultar todas as databases do servidor |
| `--exclude <dbs>` | Databases para excluir, separadas por vírgula |
| `--no-confirm` | Pular confirmação de query destrutiva |

### Formato do CSV

- **Consolidado** (padrão): Arquivo único com colunas `Server, Database, <colunas da query>`
- **Arquivos separados** (`--separate-files`): Um arquivo por servidor (`<server>_<timestamp>.csv`)
- Erros são logados no terminal, não incluídos no CSV

### Detecção de Queries Destrutivas

Queries contendo INSERT, UPDATE, DELETE, ALTER, DROP, CREATE, TRUNCATE, MERGE, GRANT, REVOKE ou SET ROLE acionam um prompt de confirmação antes da execução. Use `--no-confirm` para pular em CI/scripts.

## Documentação

Para informações mais detalhadas, consulte:

- [Arquitetura](./docs/pt-BR/ARCHITECTURE.md)
- [Especificação de Funcionalidades](./docs/pt-BR/FEATURE_SPECIFICATION.md)
- [Comando Query](./docs/QUERY_COMMAND.md)
- [Multi-Server](./docs/MULTI_SERVER.md)

## Contribuição

Contribuições são bem-vindas! Por favor, siga estes passos:

1. Fork o repositório
2. Crie uma branch de funcionalidade (`git checkout -b feature/nova-funcionalidade`)
3. Commit suas mudanças (`git commit -m 'Adiciona nova funcionalidade'`)
4. Push para a branch (`git push origin feature/nova-funcionalidade`)
5. Abra um Pull Request

Por favor, certifique-se de que todos os testes passam e que o código segue os padrões de codificação do projeto.

## Licença

Este projeto está licenciado sob a Licença MIT - veja o arquivo [LICENSE](./LICENSE) para detalhes.

---

🇺🇸 English: [README.md](./README.md)  
🇧🇷 Português (padrão)

---

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/K3K41AP4RO)
<a href="https://www.buymeacoffee.com/liphvf"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" height="36" /></a>
