<div align="center">
  <img src="assets/Mustel_icon.png" alt="Mustel Logo" width="160" />

  # Mustel

  One native binary to run SQL across PostgreSQL servers, export CSV, back up and restore — from the
  terminal or from a window.
</div>

---

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/K3K41AP4RO)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-yellow?logo=buymeacoffee&logoColor=white)](https://www.buymeacoffee.com/liphvf)

Mustel is a Rust rewrite of a .NET CLI that used to be called FurLab. The three command groups of the
product — `query`, `database` and `settings` — are migrated, and the graphical interface ships in the
same binary. The old .NET implementation was removed from the repository; its observable behaviour is
frozen as a **test contract** under `oracle/baseline/`, and every case runs in `cargo test`.

## Description

Mustel runs SQL against several PostgreSQL servers at once, writes the results as CSV, backs up and
restores databases with `pg_dump`/`pg_restore`, and keeps the server registry — from the terminal
(`mustel ...`) or from a window (`mustel gui`), over the **same core**.

## Installation

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

## Commands

| Group | Commands |
|---|---|
| `query` | `query run` (discovery, parallelism, CSV export, live progress) |
| `database` | `database backup`, `database restore`, `database pgpass add/list/remove` |
| `settings` | `settings db-servers ls/add/rm/test/set-password` |
| `gui` | `mustel gui` — SQL editor with autocomplete and syntax highlighting, servers registry and `query run` in a window |

### Examples

```bash
# Run a SQL file across the selected servers
mustel query run -i query.sql

# Inline query, all databases on the server
mustel query run -c "select count(*) from pedidos" --all --exclude postgres

# Servers registry
mustel settings db-servers add -n dev -H localhost -p 5432 -U postgres -d mydb
mustel settings db-servers set-password dev
mustel settings db-servers ls

# Backup and restore
mustel database backup mydb -o ./backups
mustel database restore mydb ./backups/mydb.dump

# pgpass.conf
mustel database pgpass add mydb --host localhost --username postgres
```

## Configuration

`%APPDATA%\Mustel\mustel.jsonc` on Windows, `$XDG_CONFIG_HOME/Mustel/mustel.jsonc`
(usually `~/.config/Mustel/mustel.jsonc`) on Linux. JSONC, with comments. Passwords are **not** in
the file: they go to the OS keyring (Windows Credential Manager / Secret Service), keyed by the
server name.

Start from [`mustel.example.jsonc`](mustel.example.jsonc).

## How to Run from Source

```bash
cargo run -p mustel -- --help
cargo run -p mustel                  # no subcommand: opens the window
cargo run -p mustel -- gui           # same, explicitly
cargo run -p mustel --no-default-features -- --help   # CLI only, no egui in the build
```

Running `mustel` with **no subcommand** opens the GUI when there is an interactive terminal (typing
it, or double-clicking the binary). Without a terminal — a script, a pipe, CI — it prints the usage
and exits 0, so nothing breaks for automation.

The GUI lives in the same binary, so every command pays its start-up cost (~55 ms). If you only ever
script it, build with `--no-default-features` to get the lean CLI (7.8 ms instead of 63 ms).

## The contract

Behaviour is pinned by two layers inside `cargo test`:

- `oracle/baseline/*.json` — 47 frozen captures of the old binary, replayed against `mustel`
  (`contract_replay`);
- [`docs/contrato-de-interface.md`](docs/contrato-de-interface.md) — one line per interface decision,
  each pointing at the test that proves it (`contract_manifest`).

The gate is `cargo test --workspace`. `oracle-compare` remains as a diagnostic that shows the N4
text differences case by case.

## Documentation

Start at [`docs/README.md`](docs/README.md), organised in four quadrants (tutorial, how-to, reference,
explanation).

## Contribution

Contributions are welcome:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

The repository guide for contributors and agents is [`AGENTS.md`](AGENTS.md). Please run
`cargo fmt`, `cargo clippy -- -D warnings` and `cargo test --workspace` before opening a PR.

## License

This project is licensed under the **GNU General Public License v3.0** (`GPL-3.0-only`) — see the
[LICENSE](./LICENSE) file for details. The Cargo manifests declare it as `license = "GPL-3.0-only"`.

---

🇺🇸 English (default)  
🇧🇷 Portuguese: [README.pt-BR.md](./README.pt-BR.md)
