# Diferenças deliberadas

O Mustel nasceu de uma reescrita, e em vários pontos ele **escolheu diferente** do produto anterior. Esta
página é a memória dessas escolhas: o que muda, por que muda e o que isso significa para a verificação.

Diferença deliberada não é dívida: é decisão registrada. Quem quiser mudar um destes comportamentos
precisa de uma decisão nova, não de um "ajuste". As decisões de engenharia que os originaram estão em
[Decisões de engenharia](decisoes.md).

> **Numeração:** não existe o item **10**. A lacuna é histórica e foi mantida de propósito — as
diferenças `11` a `18` já eram citadas no código e em outros documentos, e renumerar quebraria essas
referências. Quem procurar o item 10 não perdeu nada: ele nunca existiu.

## 1. Diferenças de comportamento

| # | Diferença | Por que o Mustel escolheu assim | Efeito na verificação |
|---|---|---|---|
| 1 | O `mustel.jsonc` **não** tem o campo `encryptedPassword` (nem `password`) — senhas vão para o cofre do sistema | `D6`: keyring nativo, sem migração de DPAPI | O harness remove `encryptedPassword`/`password` antes de comparar artefatos de `settings` |
| 2 | O caminho e o nome da config mudam (`%APPDATA%\Mustel\mustel.jsonc` em vez do arquivo do produto anterior) | `D10` + config nova, sem import do legado | Comparações de artefato usam diretórios isolados por `MUSTEL_CONFIG_DIR` e normalizam o caminho |
| 3 | A leitura de JSONC é **consciente de strings**: o produto anterior removia `//` mesmo dentro de strings (um host `http://db` viraria `http:`) | Correção deliberada na reescrita | Nenhum fixture exercita esse caso; registrar um novo se um dia divergir |
| 4 | `port` é `u16`: valor fora de 0–65535 no arquivo é erro de parse | Tipagem forte; o outro lado aceitava `int` e só validava no uso | Se um fixture tiver porta inválida, a diferença é esperada e está aqui |
| 5 | **Interface dirigida pelos docs** em dois comandos: `settings db-servers add` ganha `-i/--interactive` e `-W/--password`; `database pgpass add` aceita o banco como **posicional** (`--database` mantido por compatibilidade) | O README documentava a interface pretendida e o binário anterior ficou atrás dela | Superfície **extra** é adição aceita, não divergência de nível N1 |
| 6 | `mustel gui` e `mustel --version` não existem no produto anterior | Superfície nova do Mustel (`D5`) | Ignorar na comparação da árvore de comandos |
| 7 | A mensagem de configuração vazia do `settings db-servers ls` orienta `'mustel settings db-servers add'`; o outro lado diz `'fur ...'` | O nome do binário mudou (`D10`) | É N4; o comparador cobre o conteúdo por tokens ASCII, e a troca de nome é adição registrada aqui |
| 8 | No **Unix** o `pgpass.conf` fica com modo `0600`; o outro lado deixa o modo padrão do processo | O arquivo guarda **senha em texto claro**, e `0600` é o que o `libpq` exige no Unix | Nenhum: o comparador olha o **conteúdo**, não as permissões. No Windows as duas implementações usam a ACL herdada |
| 9 | Caminho do `pgpass.conf`: `%APPDATA%\postgresql\pgpass.conf` no Windows e `$XDG_CONFIG_HOME`/`~/.config` no Unix (**inclusive no macOS**), redirecionável por `MUSTEL_PGPASS_FILE` | O caminho é **paridade**, não invenção: é o que `SpecialFolder.ApplicationData` devolve por SO — não é a convenção do `libpq` (`~/.pgpass`) nem `~/Library/Application Support`. A variável de ambiente é alavanca de teste | O harness isola os dois lados: `MUSTEL_PGPASS_FILE` aponta o arquivo do Mustel para `fixtures/workdir/` |
| 11 | `database restore` ganhou **`--create-if-not-exists`** | Sem a flag o comportamento **continua fiel** (restaurar para banco inexistente falha); com ela, o banco é criado **pelo driver** (`D12`) antes do `pg_restore` | O outro lado tentava criar por `psql` num passo que **nunca** dispara (só criaria se o `psql` saísse com 3, e um `SELECT` sem linhas sai com 0), então a flag não tem contraparte a comparar. O caminho padrão segue igual |
| 12 | `query run`: datas/horas e números saem **fixos na convenção en-US** (`10/18/2026 10:52:15 AM`, `1500.75`, `10:52 AM`) | No produto anterior esses textos vinham da **cultura da máquina**, que não tem equivalente em Rust; a máquina da medição é en-US | Numa máquina pt-BR o outro lado escreveria `18/10/2026`, `1500,75` e `10:52`. Vale para `query run`; se a comparação um dia rodar numa máquina assim, a diferença é conhecida |
| 13 | `query run --all`: a **descoberta honra o `-W`** (e o cofre), enquanto o outro lado passa `settings: null` para ela e só obtém senha pelo cadastro — havendo senha no cadastro ele **não** pergunta; não havendo, **pede no console** | O `BuildConnectionStringForServer(server, "postgres", null)` do produto anterior descarta as flags justamente no passo de descoberta. O Mustel resolve a senha pela precedência normal | É o que torna `--all` utilizável fora de um terminal mesmo sem senha gravada. A comparação da descoberta **é possível** com senha válida no cadastro — foi assim que os dois lados foram conferidos, com CSV byte-idêntico. O que não dá é comparar o caminho **sem** senha no cadastro |
| 14 | O **texto do erro** em `<timestamp>_erros.csv` e na coluna `Error` de `<timestamp>_log.csv` vem do driver. Quando o erro **nasce no Postgres**, o texto é o mesmo dos dois lados: `SQLSTATE: texto` e, quando o servidor manda, `DETAIL` (redigido) e `POSITION`, cada um em linha própria depois de uma linha em branco. O `HINT` **não** entra. O que difere é a falha **sem** SQLSTATE (`Failed to connect to 127.0.0.1:1` × `error connecting to server`) | A mensagem é do framework: sem SQLSTATE não há texto do servidor para reproduzir, e adivinhar a redação de cada modo de falha seria pior que divergir. O `DETAIL` sai com a sentença fixa de redação do driver, que é o default dele — reproduzir isso mantém a paridade **e** não vaza dado que o outro lado não mostra | É um **N4** dentro de um artefato: no caminho de erro o harness compara a existência do arquivo, o cabeçalho, o par `Server`/`Database`, o `Status` e o `RowCount`. Dois detalhes ficam **sem medição**: erro com `DETAIL` **e** `POSITION` juntos, e a variante `POSITION` de consulta interna |
| 15 | `query run`: as linhas vão para o CSV **conforme chegam** (blocos de 64), então uma consulta que falha **no meio** deixa no arquivo as linhas já gravadas, e esse caso **não** é retentado; dois bancos do **mesmo** servidor podem intercalar blocos no arquivo dele | Correção exigida pelo critério de memória estável em 1M de linhas: o outro lado materializava o resultado de cada banco antes de escrever e, na falha, descartava tudo; retentar depois de gravar duplicaria linhas | No caminho de **sucesso** não há diferença (os mesmos bytes, conferido em 1, 2 e 3 bancos). O que muda só aparece com consulta que emita linhas e falhe depois — nenhum fixture faz isso. A intercalação troca a ordem de linhas de bancos diferentes, que já não era determinística |
| 16 | Os prompts interativos do `query run` (multi-seleção de servidores e confirmação destrutiva) são do `inquire`, não do Spectre: mesmas mensagens e mesma semântica (todos pré-selecionados; espaço alterna, enter aceita; default **sim**), com desenho e teclas extras diferentes | A UI de terminal é adaptador sobre `inquire` desde o `T8` (`D9`); reproduzir o desenho prompt a prompt significaria reescrever a TUI, e este caminho não é comparável — os dois lados exigem console | É **N4** (desenho) e caminho **sem comparação** no harness. O que compõe contrato — mensagem e decisão — é o mesmo, e o caminho não-interativo (`--no-confirm`) é comparado ponta a ponta |
| 17 | O prompt de senha do `query run` aparece **no máximo uma vez por servidor**: a entrada vai resolvida para o núcleo, que só troca o banco de cada conexão. O outro lado monta a connection string **por conexão**, então sem senha no `-W` nem no cofre ele pergunta uma vez por banco (e mais uma na descoberta) | Consequência direta de `D9`: "o frontend resolve, o núcleo executa". Resolver a senha por banco obrigaria o núcleo a chamar prompt, que é UI | Nenhum caso do harness chega a esse caminho (exige console, como os itens 13 e 16). Onde há `-W` ou senha no cofre — todo caminho comparável — não há prompt em nenhum dos lados |
| 18 | `mustel` **sem subcomando**, com terminal interativo, abre a **GUI**; o outro lado imprime a ajuda e sai 0 | Decisão de produto: quem digita `mustel` ou dá duplo-clique espera a interface; obrigar a lembrar de `mustel gui` seria hostil. O binário único saiu da medição do custo de partida do `egui` | **Sem terminal** (script, pipe, harness, CI) o comportamento é o do produto anterior: ajuda em `stdout` e exit **0** — é o que preserva a paridade comparável |

## 2. Formatação (N4) aceita

Nenhuma além das registradas acima. Divergência de formatação (`--help`, cores, tabelas, redação de
erros) **não bloqueia** entrega, mas precisa de linha nesta página. A escala está em
[Escada de fidelidade](escada-de-fidelidade.md).

## 3. Comportamento (N1–N3) — proibido divergir

Nenhuma. Divergência em N1–N3 bloqueia a entrega do que ela afeta.

## 4. O que foi observado no binário anterior

Fatos medidos durante a migração que explicam por que certos caminhos do código são como são. Não são
diferenças: são o **comportamento de referência** que o Mustel reproduz.

| Observação | Consequência no Mustel |
|---|---|
| Erros vão para **stdout**; `stderr` ficou vazio em 7/7 casos | `report_error` imprime em stdout |
| Erro de uso (comando/opção desconhecida, argumento ausente) sai com **1** | `handle_parse_error` remapeia o `2` do `clap` |
| `query run` com arquivo ausente sai com **1** (exceção própria, não o `FileNotFoundException` do `Program.cs`) | exit `1` com `SQL input file not found: <path>` |
| `rm` de servidor inexistente: `Error: Server 'x' not found.` com exit **41** | `ConfigError::ServerNotFound` casa com esse texto |
| Config do usuário é criada vazia na primeira execução, com **CRLF** no Windows | a escrita usa a quebra de linha do SO |
| Tabela do `ls` sai em **codepage do console** (não-UTF-8) | o comparador decodifica/normaliza; o Mustel emite UTF-8 (diferença de bytes aceita) |
| Senha ausente em modo não-interativo → o outro lado **quebra** (`Failed to read input in non-interactive mode`), exit 1 ou 40 | diferença deliberada: o Mustel falha claro e sugere `-W`/`set-password` |
| `database restore` devolve **2** para arquivo ausente e path traversal | exit 2 existe no comando, não só no parser — mas não é alcançável no cenário determinístico, porque sem senha o outro lado quebra antes |
| Com a configuração real do usuário presente (senha salva) o comportamento muda | todo caso do baseline fixa o estado da config (`config_seed`); sem isso a captura não é reproduzível |
| `database pgpass add` **não** aceita o banco como posicional: responde `Error: Unknown command 'mydb'.` (exit 1) | a forma posicional do Mustel é adição deliberada (item 5), então nenhum caso do baseline pode usá-la — o argv de um caso roda nos dois binários |
| O exit **2** de `pgpass add` (banco ausente, host inválido, porta inválida) vem do **corpo** do comando, não do parser | o Mustel devolve `Ok(2)` ali, e não passa pelo mapeamento de `CoreError` (que daria 41) |

Sobre reprodutibilidade: o baseline é reproduzível **na mesma máquina** (caminhos viram `<WORKDIR>`,
`<REPO>`, `<HOME>`, `<LEGACY_CONFIG>`). Entre sistemas operacionais o texto **não** é idêntico — o
console do outro lado quebra linha pela largura, e o comprimento do caminho muda a quebra. Entre
máquinas vale o que é contrato: exit code e artefatos.

## 5. Lista branca de resíduos do nome antigo

O produto se chamava FurLab, e o nome ainda aparece em pontos onde é **rastreabilidade**, não descuido.
O critério: fora de `docs/`, `.git/` e `MustelPlans/`, o termo aparece apenas nos lugares abaixo.

| Caminho | Por que permanece |
|---|---|
| `crates/**` (doc-comments) | citam o FurLab/FurLab.CLI para rastrear o porte ("contrato `Program.cs` do FurLab", "Porte de `PgPassService.cs`"). Apagar tornaria mais difácil conferir o código contra o contrato |
| `fixtures/baseline/*.json` | são **capturas congeladas**: a saída inclui o nome do binário e da config antigos, e não podem ser reescritas |
| `AGENTS.md`, `README.md`, `README.pt-BR.md` | descrevem de onde o Mustel veio |
| `fixtures/baseline/help-root.json` | **captura congelada**: a ajuda do binário antigo listava `opencode`, `claude` e `file`; o arquivo é o contrato |
