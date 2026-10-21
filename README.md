<div align="center">
  <img src="assets/FurLab_icon.png" alt="FurLab Logo" width="160" />

  # FurLab

  A powerful .NET CLI tool to automate common development tasks.
</div>

---

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/K3K41AP4RO)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-yellow?logo=buymeacoffee&logoColor=white)](https://www.buymeacoffee.com/liphvf)

> **Rust rewrite in progress** (branch `feat/rust-migration`). The `query`, `database` and `settings`
> command groups are being rewritten in Rust as **Mustel** — binary `mustel`, installed from the release
> archive (see below). The .NET implementation now lives in `legacy/`: it is the **parity oracle**
> during the migration and remains the product for the other command groups (`file`, `claude`,
> `opencode`, `winget`, `windowsfeatures`, `docker`) until the second wave.
> The migration plan is in [`MustelPlans/migracao-rust.md`](MustelPlans/migracao-rust.md).

> **Status (2026-10-21)**: the three groups are migrated and the GUI is up — `mustel gui`.
> The sections **Mustel (current)** below describe what ships today; **FurLab (.NET, frozen)**
> documents the legacy CLI that still owns the remaining groups.

---

## Mustel (current)

### Description

Mustel is the Rust rewrite of the FurLab CLI: one native binary that runs SQL across several
PostgreSQL servers, exports CSV, backs up and restores databases, and manages the server
registry — from the terminal (`mustel ...`) or from a window (`mustel gui`), over the **same core**.

### Installation

Build the release package for the current platform:

```bash
# Windows
./publish.ps1
# Linux
./publish.sh
```

Each produces `packaging/mustel-<version>-<target>/` (binary, `mustel.example.jsonc`, `LICENSE`,
`README.txt`), an archive (`.zip` on Windows, `.tar.gz` on Unix) and the `.sha256` next to it.
Copy the binary to a directory on your `PATH`.

**Supported targets**: `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`.
macOS is **not** supported for now.

**Runtime dependency (Linux)**: `libdbus-1.so.3` — `sudo apt install libdbus-1-3` on
Debian/Ubuntu. Opening the window additionally needs X11 or Wayland with OpenGL; the CLI alone
still loads libdbus at start-up.

> `cargo install mustel` and the winget package (`Mustel.CLI`) are **not published yet** — see
> [`MustelPlans/release-pendencias.md`](MustelPlans/release-pendencias.md). Until then, use the
> release archive.

### What is migrated

| Group | Commands |
|---|---|
| `query` | `query run` (discovery, parallelism, CSV export, live progress) |
| `database` | `database backup`, `database restore`, `database pgpass add/list/remove` |
| `settings` | `settings db-servers ls/add/rm/test/set-password` |
| `gui` | `mustel gui` — servers registry and `query run` in a window |

The other groups (`file`, `claude`, `opencode`, `winget`, `windowsfeatures`, `docker`) are still
the .NET CLI in `legacy/` — second wave.

### Configuration

`%APPDATA%\Mustel\mustel.jsonc` on Windows, `$XDG_CONFIG_HOME/Mustel/mustel.jsonc`
(usually `~/.config/Mustel/mustel.jsonc`) on Linux. JSONC, with comments. Passwords are **not** in
the file: they go to the OS keyring (Windows Credential Manager / Secret Service), keyed by the
server name.

Start from [`mustel.example.jsonc`](mustel.example.jsonc).

### How to Run from Source

```bash
cargo run -p mustel -- --help
cargo run -p mustel -- gui        # the window
cargo run -p mustel --no-default-features -- --help   # CLI only, no egui in the build
```

---

## FurLab (.NET, frozen — parity oracle)

Everything from here down describes the .NET CLI as it was frozen. It still owns the groups that
have not been migrated, and it is the reference the Rust port is compared against
(`oracle/README.md`).


## Description

FurLab is a cross-platform command-line interface (CLI) built with .NET that helps developers automate repetitive development tasks. It provides commands for database operations, file management, AI tool installation (Claude Code, OpenCode), and Windows package management.

> **Note**: This is a hobby project created for personal use. It may not follow all best practices or have comprehensive testing. Contributions and feedback are welcome, but please keep in mind this was built to solve the author's specific needs.

## Problem It Solves

Developers often perform repetitive tasks that can be automated:
- Combining multiple files into one
- Installing and configuring AI development tools
- Backing up and restoring Windows packages
- Running SQL queries across multiple PostgreSQL servers

FurLab consolidates these tasks into a single, easy-to-use CLI tool.

## Key Features

- **Database Backup**: Backup PostgreSQL databases using pg_dump
- **Query Execution**: Run SQL queries across multiple PostgreSQL servers with interactive server selection, parallel execution, and CSV export
- **Destructive Query Guard Rail**: Automatic detection of INSERT, UPDATE, DELETE, ALTER, DROP, etc. with confirmation prompt
- **Server Management**: Add, list, remove, and test PostgreSQL servers via CLI
- **File (Combine)**: Combine multiple files into one
- **Claude Code Integration**: Install and configure Claude Code CLI
- **OpenCode Integration**: Install and configure OpenCode CLI
- **Winget Manager**: Backup and restore Windows package manager packages

## Tech Stack

- **Framework**: .NET 10
- **Language**: C#
- **CLI Parsing**: System.CommandLine
- **Database**: Npgsql (PostgreSQL)
- **Configuration**: JSONC (furlab.jsonc)
- **UI**: Spectre.Console

## Installation

### Prerequisites

- .NET SDK 10 or later
- Windows (required for Claude, OpenCode, and Winget commands)

### Install as .NET Tool

```bash
dotnet tool install --global FurLab
```

Or install from NuGet:

```bash
dotnet tool install -g FurLab
```

### Build from Source

```bash
git clone https://github.com/your-repo/FurLab.git
cd FurLab
dotnet restore
dotnet build
```

## How to Run Locally

### Run from Source

```bash
dotnet run -- --help
```

## Basic Usage Examples

### Query Execution

```bash
# Run a SQL file across selected servers
FurLab query run --input query.sql

# Run an inline query
FurLab query run --command "SELECT * FROM users"

# Run with output directory
FurLab query run -i query.sql -o ./results

# Generate one CSV per server
FurLab query run -i query.sql --separate-files
```

On execution, FurLab shows an interactive server selection prompt (all configured servers are pre-selected). Results are exported to CSV with columns `Server, Database, <query columns>`.

### Server Management

```bash
# List configured servers
FurLab settings db-servers ls

# Add a server interactively
FurLab settings db-servers add -i

# Add a server with flags
FurLab settings db-servers add -n dev -h localhost -p 5432 -U postgres -W mypass

# Test connection to a server
FurLab settings db-servers test -n dev

# Remove a server
FurLab settings db-servers rm -n dev
```

### Database Backup

```bash
# Backup with default connection settings
FurLab database backup mydb

# Backup with custom connection settings
FurLab database backup mydb --host localhost --port 5432 --username postgres --password mypassword

# Backup with custom output path
FurLab database backup mydb -o "C:\backups\mydb.backup"
```

### File Operations

```bash
# Combine multiple files into one
FurLab file combine -i "C:\temp\*.sql" -o "C:\temp\result.sql"

# Convert file encoding (auto-detect source)
FurLab file convert-encoding -i "src/**/*.cs" --to UTF-8

# Convert with explicit source encoding and backup
FurLab file convert-encoding -i "*.txt" --from Latin1 --to UTF-8 --backup

# Convert and save to a different directory
FurLab file convert-encoding -i "docs/*.md" --to UTF-8 -o "converted_docs/"
```

### Install Claude Code

```bash
FurLab claude install
```

### Winget Backup

```bash
FurLab winget backup -o "C:\backup"
FurLab winget restore -i "C:\backup\backup-winget.json"
```

## Configuration

FurLab stores user configuration in `%LocalAppData%\FurLab\furlab.jsonc` (JSONC format with comments support).

### Example furlab.jsonc

```jsonc
{
  // PostgreSQL servers configuration
  "servers": [
    {
      "name": "dev",           // Unique identifier
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "password": "mypassword",
      "databases": ["mydb", "app_dev"],
      "sslMode": "Prefer",
      "timeout": 30,
      "commandTimeout": 300,
      "maxParallelism": 4
    },
    {
      "name": "prod",
      "host": "prod-db.company.com",
      "port": 5432,
      "username": "readonly",
      "password": "secret",
      "fetchAllDatabases": true,
      "excludePatterns": ["template*", "postgres"],
      "sslMode": "Require"
    }
  ],
  // Default settings
  "defaults": {
    "outputFormat": "csv",
    "outputDirectory": "./results",
    "fetchAllDatabases": false,
    "requireConfirmation": true,
    "maxParallelism": 4
  }
}
```

## Command List

| Command | Description |
|---------|-------------|
| `query run` | Execute SQL queries and export to CSV |
| `settings db-servers ls` | List configured servers |
| `settings db-servers add` | Add a server (interactive or with flags) |
| `settings db-servers rm` | Remove a server |
| `settings db-servers test` | Test server connection |
| `database backup` | Backup PostgreSQL database |
| `file combine` | Combine multiple files into one |
| `claude` | Claude Code integration |
| `opencode` | OpenCode CLI integration |
| `winget` | Windows package manager |

## Query Command Details

### Options

| Option | Description |
|--------|-------------|
| `-i, --input <file>` | SQL input file |
| `-c, --command <sql>` | Inline SQL query (mutually exclusive with `-i`) |
| `-o, --output <path>` | Output file or directory |
| `--separate-files` | One CSV per server (default: single consolidated file) |
| `--all, -a` | Query all databases on server |
| `--exclude <dbs>` | Comma-separated databases to exclude |
| `--no-confirm` | Skip destructive query confirmation |

### CSV Output Format

- **Consolidated** (default): Single file with columns `Server, Database, <query columns>`
- **Separate files** (`--separate-files`): One file per server (`<server>_<timestamp>.csv`)
- Errors are logged to the terminal, not included in CSV

### Destructive Query Detection

Queries containing INSERT, UPDATE, DELETE, ALTER, DROP, CREATE, TRUNCATE, MERGE, GRANT, REVOKE, or SET ROLE trigger a confirmation prompt before execution. Use `--no-confirm` to skip in CI/scripts.

## Documentation

For more detailed information, see:

- [Architecture](./docs/en/ARCHITECTURE.md)
- [Feature Specification](./docs/en/FEATURE_SPECIFICATION.md)
- [Query Command](./docs/QUERY_COMMAND.md)
- [Multi-Server](./docs/MULTI_SERVER.md)

## Contribution

Contributions are welcome! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please ensure all tests pass and code follows the project's coding standards.

## License

This project is licensed under the **GNU General Public License v3.0** (`GPL-3.0-only`) — see the
[LICENSE](./LICENSE) file for details. The Cargo manifests declare it as `license = "GPL-3.0-only"`.

---

🇺🇸 English (default)  
🇧🇷 Portuguese: [README.pt-BR.md](./README.pt-BR.md)
