# Baseline de performance — FurLab (.NET, oráculo)

Medido com `oracle-perf` (mede o processo completo; substitui o `hyperfine`, que não
está disponível na máquina da migração). Cada linha é a mediana de 30 execuções.

| métrica | mediana | mínimo | máximo | execuções | falhas | notas |
|---|---|---|---|---|---|---|
| startup: --help | 157.2 ms | 155.2 ms | 166.0 ms | 30 | 0 | inclui partida do runtime .NET |
| settings db-servers ls | 201.7 ms | 198.2 ms | 208.7 ms | 30 | 0 | config semeada com 2 servidores |
| erro de uso (comando desconhecido) | 118.2 ms | 116.4 ms | 119.7 ms | 30 | 30 | caminho de erro; exit code 1 |

## Tamanho dos binários

- oráculo (`C:\Users\liphv\Documents\DEV\FurLab\.delta\worktrees\00z7a7nyr8qy\FurLab\oracle\bin\FurLab.CLI.exe`): **158 KB** (162304 bytes)
- `mustel` release (`C:\Users\liphv\Documents\DEV\FurLab\.delta\worktrees\00z7a7nyr8qy\FurLab\target\release\mustel.exe`): **1.1 MB** (1183744 bytes)

> **Cuidado ao comparar tamanho.** O que está publicado em `oracle/bin` é a saída de
> `dotnet publish` **framework-dependent**: o executável é pequeno porque **não** carrega o
> runtime — ele exige o .NET instalado (dezenas de MB) na máquina do usuário. O binário do
> Mustel é auto-contido por natureza. A comparação justa é contra o artefato self-contained
> (`publish.ps1 exe`, um arquivo único de vários MB) e contra o requisito de instalação.

## Pendentes (dependem de Postgres)

- export CSV de 100k linhas — **medido no T36**: ver `oracle/perf/query.md` (2,72 s × 1,07 s e
  318,8 MB × 11,6 MB);
- `database backup` de um dump de fixture — medir em T28.

## Comparação com o Mustel (parcial, T11)

| métrica | oráculo (.NET) | mustel | razão | observação |
|---|---|---|---|---|
| `--help` | 157.2 ms | 4.7 ms | 33.3× | comparação justa: os dois só parseiam e imprimem ajuda |
| `settings db-servers ls` | 201.7 ms | 4.7 ms | 43.0× | comparação justa desde o T16: os dois leem a config e montam a lista |

O comparativo final (.NET × Rust) é o T46, com todas as métricas reais.

A meta do plano para startup é ≤ 1/3 do baseline.
