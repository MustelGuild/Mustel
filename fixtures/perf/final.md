# Performance final — .NET (oráculo) × Rust (Mustel)

Comparativo consolidado do escopo migrado (`query`, `database`, `settings`), medido na **mesma
máquina**, com o mesmo método dos relatórios parciais: processo inteiro, mediana de N execuções,
sem instrumentação dentro do binário. Este documento fecha o T46 e cita os parciais em vez de
repetir o método deles.

**Resumo**: o Mustel é mais rápido em tudo que foi medido e usa uma fração da memória, mas **a meta
de startup do plano deixou de ser cumprida depois que a GUI entrou no mesmo binário** — ver a seção
“Achado principal” antes de qualquer número.

---

## 1. Achado principal — o startup regrediu 13× com a GUI

O critério do T46 pedia `--help` em **≤ 1/3 do baseline** (.NET = 160,9 ms → **≤ 53,6 ms**).

| build | `--version` | `--help` | binário |
|---|---|---|---|
| `mustel` **com GUI** (default) | **63,3 ms** | **63,4 ms** | 13,5 MB (13 502 464 B) |
| `mustel` **sem GUI** (`--no-default-features`) | **7,7 ms** | **7,8 ms** | 5,6 MB (5 594 624 B) |
| oráculo (.NET) | — | **160,9 ms** | 158 KB (framework-dependent) |

O `--version` e o `--help` custam **o mesmo** dentro de cada build, então a diferença de 55,6 ms é
**partida de processo**, não parsing de argumento: é a inicialização estática de `egui`/`winit`
(novos globais, fontes, registro de backends) que vem junto quando o `eframe` é ligado ao binário,
mesmo para quem só roda `mustel settings ...`.

Consequência para o critério: **o build padrão (63,4 ms) NÃO atende 1/3 do baseline**; o build só
CLI (7,8 ms) atende com folga (**20,6×** mais rápido). O número está registrado como está — a
decisão de produto que saiu dele (feature `gui` opcional no release) está no plano, no T46.

---

## 2. Métricas por comando

| métrica | oráculo (.NET) | mustel | razão | fonte |
|---|---|---|---|---|
| startup `--help` (só CLI) | 160,9 ms | **7,8 ms** | **20,6×** | medido aqui, 15 execuções |
| startup `--help` (com GUI) | 160,9 ms | 63,4 ms | 2,5× | medido aqui, 15 execuções |
| `settings db-servers ls` (com GUI) | 200,5 ms | **63,1 ms** | **3,2×** | medido aqui, 15 execuções, config com 2 servidores |
| export CSV 100k linhas | 2,72 s | **1,07 s** | **2,5×** | `query.md` (T36, 10 execuções) |
| `database backup` (50k linhas, dump de 1 378 463 B) | 375 ms | **188 ms** | **2,0×** | medido aqui, 7 execuções |
| pico de RSS no export 100k | 318,8 MB | **11,6 MB** | **27,5×** | `query.md` (T36) |

## 3. Tamanho dos binários

| artefato | bytes |
|---|---|
| `fur.exe` — .NET **self-contained** single-file (`legacy/publish.ps1 exe`) | **81 027 791** (81,0 MB) |
| `mustel` release **com GUI** | **13 502 464** (13,5 MB) |
| `mustel` release **só CLI** | **5 594 624** (5,6 MB) |
| `FurLab.CLI.exe` framework-dependent (referência do harness) | 162 304 (158 KB) |

Comparação justa é contra o `fur.exe`: o executável framework-dependent é pequeno porque **exige o
runtime .NET instalado** (dezenas de MB na máquina do usuário), enquanto o Mustel é auto-contido por
natureza. Contra o artefato que o usuário de fato instala: **6× menor com GUI**, **14,5× menor sem
GUI**.

## 4. Como foi medido

**Startup** (`--help`/`--version`), PowerShell, 15 execuções, mediana:

```powershell
$t = 1..15 | ForEach-Object { (Measure-Command { & $exe --help | Out-Null }).TotalMilliseconds } | Sort-Object
"mediana {0:N1} ms" -f $t[7]
```

**`database backup`**: Postgres 16 em container (`docker run -d --name mustel-t46-pg -e
POSTGRES_PASSWORD=dev -e POSTGRES_DB=mustel_t46 -p 55433:5432 postgres:16`), tabela de fixture com
50 000 linhas (`id`, `texto`, `hash md5`, `valor numeric`), 7 execuções, **saída apagada entre
elas** e o tamanho do `.dump` conferido em cada rodada:

```powershell
(Measure-Command { & $exe database backup mustel_t46 -H 127.0.0.1 -p 55433 -U postgres -W dev | Out-Null }).TotalMilliseconds
```

Os dois lados produziram o **mesmo dump de 1 378 463 bytes** (conferido por tamanho; o conteúdo de
um `pg_dump` custom não é estável byte a byte por causa do carimbo de criação).

**Export 100k**: método e fixture em `query.md` (não repetido aqui).

## 5. Ressalvas

1. **Startup com GUI não cabe em 1/3 do baseline** (63,4 ms × 53,6 ms). O número não foi ajustado
   para caber: ele é o motivo da decisão de tornar a `gui` opcional no artefato de release.
2. **RSS de `--help` não foi medido**: um processo de 8–63 ms não permite amostrar
   `PeakWorkingSet64` de fora com confiança (a amostragem a cada 5 ms pega 1–2 pontos, e depois da
   saída o valor se perde). O RSS que importa — export grande — está em `query.md` (27,5× menos).
3. **`database backup` do oráculo ignora `-o`**: ele grava `<banco>.dump` no diretório atual. A
   medição usou a saída padrão dos dois para não medir caminhos diferentes.
4. **A máquina é a mesma dos dois lados**, com o Docker rodando só para o Postgres de fixture.
5. **`settings db-servers ls`**: o lado .NET vem de `baseline.md` (200,5 ms, mediana de 30); o lado
   Mustel foi medido com o mesmo método e a mesma config semeada — **63,1 ms** com o build padrão,
   o que é mais um caso do imposto de startup da GUI (o comando em si é o mesmo de 8 ms).
