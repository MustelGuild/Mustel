# Divergências conhecidas e lista branca

Este arquivo é a memória do que é **aceito** na migração para Rust: as divergências de formatação
(nível N4) entre o Mustel e o FurLab (.NET, o oráculo) e as ocorrências de nome legado que permanecem
no repositório de propósito.

## 1. Lista branca de resíduos `furlab` / `fur`

O critério da seção 6 do plano (`MustelPlans/migracao-rust.md`) exige que, fora de `legacy/`,
`openspec/`, `docs/`, `.git/` e `MustelPlans/`, o termo apareça apenas nos lugares abaixo:

| Caminho | Por que permanece |
|---------|-------------------|
| `Cargo.toml`, `crates/*/Cargo.toml` | `repository` aponta para `liphvf/FurLab` (D18: o repositório só é renomeado depois) |
| `crates/mustel/src/main.rs`, `crates/mustel-gui/src/lib.rs`, `crates/mustel-testkit/src/lib.rs` | docs de módulo citam o FurLab como produto legado / oráculo |
| `AGENTS.md`, `CLAUDE.md` | descrevem o legado congelado e as regras de paridade |
| `README.md`, `README.pt-BR.md` | aviso de migração: o que permaneceu no .NET |
| `winget-publish.ps1` | URL de release do repositório `liphvf/FurLab` (D18) |
| `.github/workflows/build.yml`, `.github/workflows/publish.yml` | workflows do legado (substituídos/renomeados em T10 e T44) |
| `.github/workflows/ci.yml` | job `legacy` que garante que o oráculo continua compilando em Release |
| `oracle/perf/baseline.md`, `oracle/README.md`, `oracle/known-divergences.md` | documentam o oráculo por nome |
| `.vscode/launch.json`, `.vscode/tasks.json` | configs de debug do legado (`Legacy .NET ...`) |
| `.claude/skills/furlab-command-manager/**`, `.opencode/skills/furlab-command-manager/**` | skill de convenções C# do legado — `CLAUDE.md` declara que vale **somente** para `legacy/` |
| `.opencode/skills/gh-pr-create/SKILL.md` | exemplos citam o repositório pelo nome atual |

## 2. Divergências N4 (formatação) aceitas

Nenhuma registrada até aqui.

## 2.1 Divergências de comportamento aceitas por decisão de produto

| # | Divergência | Decisão | Efeito na comparação |
|---|-------------|---------|----------------------|
| 1 | O `mustel.jsonc` **não** tem o campo `encryptedPassword` (nem `password`) — senhas vão para o keyring | D6 (keyring, sem migração de DPAPI) | O harness normaliza/remove `encryptedPassword` antes de comparar artefatos de `settings` |
| 2 | O caminho/nome da config muda (`%APPDATA%\Mustel\mustel.jsonc` em vez de `%LOCALAPPDATA%\FurLab\furlab.jsonc`) | D10 + Q8 (config nova, sem import do legado) | Comparações de artefato usam diretórios isolados por `MUSTEL_CONFIG_DIR` e normalizam o caminho |
| 3 | Leitura de JSONC é **consciente de strings**: o legado removia `//` mesmo dentro de strings (host `http://db` viraria `http:`) | Correção deliberada na reescrita; fixture de paridade não exercita esse caso | Sem impacto nos fixtures; registrar novo fixture se um dia divergir |
| 4 | `port` é `u16` no Mustel: valor fora de 0–65535 no arquivo é erro de parse | Tipagem forte; o legado aceitava `int` e só validava no uso | Se um fixture tiver porta inválida, a divergência é esperada e documentada aqui |
| 5 | **Interface dirigida pelos docs** em dois comandos (decisão do usuário, 2026-10-14): `settings db-servers add` ganha `-i/--interactive` e `-W/--password`; `database pgpass add` aceita o banco como **posicional** (`--database` mantido por compatibilidade, formas mutuamente exclusivas) | O README documentava a interface pretendida e o binário legado ficou atrás | O harness deve tratar essa superfície extra como **adição aceita** e não como divergência N1 em `settings db-servers add` e `database pgpass add` |
| 6 | `mustel gui` e `mustel --version` não existem no legado | Superfície nova do produto Mustel (D5/T6) | Ignorar na comparação da árvore de comandos |
| 7 | A mensagem de configuração vazia do `settings db-servers ls` orienta `'mustel settings db-servers add'`; o legado diz `'fur settings db-servers add'` | Nome do binário mudou (D10) | Texto é N4; o comparador cobre o conteúdo por tokens ASCII, e "mustel"/"fur" são adições registradas aqui |
| 8 | No **Unix** o `pgpass.conf` fica com modo `0600`; o legado deixa o modo padrão do processo (0644 com umask comum) | É um arquivo de **senha em texto claro**, e `0600` é o que o `libpq` exige para aceitá-lo no Unix | Nenhum: o comparador olha o **conteúdo**, não as permissões. No Windows as duas implementações usam a ACL herdada |
| 9 | Caminho do `pgpass.conf`: `%APPDATA%\postgresql\pgpass.conf` no Windows e `$XDG_CONFIG_HOME`/`~/.config` no Unix (**inclusive no macOS**), redirecionável por `MUSTEL_PGPASS_FILE` | O caminho é **paridade**, não divergência: é o que `SpecialFolder.ApplicationData` do .NET devolve por SO — não é a convenção do `libpq` (`~/.pgpass`) nem `~/Library/Application Support` no macOS. Já a variável de ambiente é alavanca de teste/harness, como `MUSTEL_CONFIG_DIR` (D10) | O harness isola os dois lados: `MUSTEL_PGPASS_FILE` aponta o arquivo do Mustel para `oracle/workdir/` e o arquivo do legado (que **não** respeita variável de ambiente) é semeado/restaurado |
| 11 | `database restore` ganhou **`--create-if-not-exists`** (não existe no legado) | Decisão do usuário (2026-10-18): sem a flag o comportamento **continua fiel** — restaurar para banco inexistente falha; com a flag, o banco é criado **pelo driver** (D12) antes do `pg_restore` | O legado tentava criar num passo por `psql` que **nunca** dispara (ele só cria quando o `psql` sai com código 3, e um `SELECT` sem linhas sai com 0), então a flag não tem contraparte a comparar. O caminho padrão segue comparável e igual |
| 12 | `query run`: datas/horas e números saem **fixos na convenção en-US** (`10/18/2026 10:52:15 AM`, `1500.75`, `10:52 AM`) | No .NET esses textos vêm da **cultura da máquina** (`DateTime.ToString()`, `DateOnly`, `TimeOnly`), que não tem equivalente em Rust. A máquina onde o oráculo foi medido é en-US | Numa máquina pt-BR o legado escreveria `18/10/2026`, `1500,75` e `10:52`. Vale para `query run`; se algum dia a comparação rodar numa máquina assim, a diferença é esperada e conhecida |
| 13 | `query run --all`: a **descoberta honra o `-W`** (e o cofre), enquanto o legado passa `settings: null` para ela e volta a **pedir a senha no console** | O `BuildConnectionStringForServer(server, "postgres", null)` do legado é o bug: ele descarta as flags justamente no passo de descoberta. O Mustel resolve a senha pela precedência normal | É o que torna `--all` utilizável fora de um terminal (o legado trava no prompt). **Consequência para a verificação**: não dá para capturar o `--all` do oráculo de forma não-interativa, então esse caminho fica sem comparação N1–N3 — o que se compara são os outros (T32) e a descoberta é conferida pela query/filtros portados com teste |
| 14 | O **texto do erro** gravado em `<timestamp>_erros.csv` e na coluna `Error` do `<timestamp>_log.csv` vem do driver. Quando o erro **nasce no Postgres** o texto é o mesmo dos dois lados: o Npgsql monta `SQLSTATE: texto` (medido: `3D000: database "nao_existe" does not exist`) e o Mustel passou a montar igual — conferido byte a byte no T35. O que difere é a falha que **não** tem SQLSTATE, aí a redação é do driver (medido com servidor inalcançável: `Failed to connect to 127.0.0.1:1` no oráculo × `error connecting to server` no Mustel) | A mensagem é do framework, não do produto: sem SQLSTATE não há texto do servidor para reproduzir, e adivinhar a redação do Npgsql para cada modo de falha (conexão, autenticação, timeout) seria pior que divergir | É um **N4** (redação) dentro de um artefato, e por isso está aqui em vez de virar falha: no caminho de erro o que o harness compara é a existência do arquivo, o cabeçalho, o par `Server`/`Database`, o `Status` e o `RowCount`; o texto do driver fica de fora (o do Postgres, agora igual, pode ser comparado) |
| 15 | `query run`: as linhas vão para o CSV **conforme chegam** (blocos de 64), então uma consulta que falha **no meio** deixa no arquivo do servidor as linhas já gravadas, e esse caso **não** é retentado; dois bancos do **mesmo** servidor podem intercalar blocos no arquivo dele | Correção deliberada, exigida pelo critério do T34 (memória estável em 1M linhas): o legado materializa o resultado de cada banco (`List<Dictionary>`) antes de escrever, e na falha descarta tudo; retentar depois de gravar duplicaria linhas | No caminho de **sucesso** não há diferença (os mesmos bytes, conferido com o oráculo em 1, 2 e 3 bancos). O que muda só aparece com uma consulta que emita linhas e falhe depois — nenhum fixture faz isso hoje. O intercalar-se troca a ordem das linhas de bancos diferentes **dentro** de um mesmo arquivo, que já não é determinística (quem termina primeiro escreve primeiro) |

## 3. Divergências N1–N3 (proibidas)

Nenhuma. Divergência em N1–N3 bloqueia a conclusão da tarefa correspondente.
