# Baseline de performance — FurLab (.NET, oráculo)

Medido com `oracle-perf` (mede o processo completo; substitui o `hyperfine`, que não
está disponível na máquina da migração). Cada linha é a mediana de 30 execuções.

| métrica | mediana | mínimo | máximo | execuções | falhas | notas |
|---|---|---|---|---|---|---|
| startup: --help | 160.9 ms | 158.1 ms | 174.1 ms | 30 | 0 | inclui partida do runtime .NET |
| settings db-servers ls | 200.5 ms | 198.4 ms | 213.2 ms | 30 | 0 | config semeada com 2 servidores |
| erro de uso (comando desconhecido) | 117.8 ms | 115.4 ms | 126.7 ms | 30 | 30 | caminho de erro; exit code 1 |

## Tamanho dos binários

- oráculo (`C:\Users\liphv\Documents\DEV\FurLab\.delta\worktrees\00z7a7nyr8qy\FurLab\oracle\bin\FurLab.CLI.exe`): **158 KB** (162304 bytes)
- `mustel` release (`C:\Users\liphv\Documents\DEV\FurLab\.delta\worktrees\00z7a7nyr8qy\FurLab\target\release\mustel.exe`): **12.9 MB** (13502464 bytes)

> **Cuidado ao comparar tamanho.** O que está publicado em `fixtures/bin` é a saída de
> `dotnet publish` **framework-dependent**: o executável é pequeno porque **não** carrega o
> runtime — ele exige o .NET instalado (dezenas de MB) na máquina do usuário. O binário do
> Mustel é auto-contido por natureza. A comparação justa é contra o artefato self-contained
> (`publish.ps1 exe`, um arquivo único de vários MB) e contra o requisito de instalação.

## Pendentes (dependem de Postgres)

Tudo o que dependia de Postgres foi **medido**: o export de 100k linhas está em [`query.md`](query.md)
(T36) e o `database backup` em [`final.md`](final.md) (T46). Este arquivo guarda só as métricas que
não precisam de banco.

## Comparação com o Mustel (parcial, T11)

| métrica | oráculo (.NET) | mustel | razão | observação |
|---|---|---|---|---|
| `--help` | 160.9 ms | 63.6 ms | 2.5× | comparação justa: os dois só parseiam e imprimem ajuda |
| `settings db-servers ls` | 200.5 ms | 63.4 ms | 3.2× | comparação justa desde o T16: os dois leem a config e montam a lista |

O comparativo final (.NET × Rust) está em [`final.md`](final.md) — é lá que o T46 registra as cinco
métricas e a decisão que saiu do startup com GUI.

A meta do plano para startup é ≤ 1/3 do baseline. **Com a GUI no binário a meta não é cumprida**
(63,4 ms × 53,6 ms); sem GUI é 7,8 ms (20,6×). Ver `final.md`.
