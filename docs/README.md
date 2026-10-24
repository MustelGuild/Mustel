# Documentação do Mustel

O Mustel roda SQL em vários servidores PostgreSQL, exporta CSV, faz backup e restore e mantém o
cadastro de servidores — do terminal ou de uma janela.

Esta documentação segue o **[Diátaxis](https://diataxis.fr/)**. A ideia é simples: existem quatro
tipos de documento, cada um respondendo a uma necessidade diferente. Misturar os quatro num arquivo
só é a origem da maior parte da documentação ruim.

## Por onde começar

| Você quer… | Vá para |
|---|---|
| **aprender** fazendo, do zero | [Tutorial: primeiros passos](tutorials/primeiros-passos.md) |
| **resolver um problema** concreto | [How-to guides](how-to/) |
| **consultar** um comando, flag, exit code ou formato de arquivo | [Referência](reference/) |
| **entender por que** algo é assim | [Explicação](explanation/) |

### As páginas de cada quadrante

| Tutorial | How-to | Referência | Explicação |
|---|---|---|---|
| [Primeiros passos](tutorials/primeiros-passos.md) | [Vários bancos](how-to/varios-bancos.md) | [Comandos](reference/comandos.md) | [Credenciais](explanation/credenciais.md) |
| | [Escolher bases na janela](how-to/escolher-bases-na-janela.md) | [`query run`](reference/query-run.md) | [CSV como o Npgsql](explanation/csv-como-o-npgsql.md) |
| | [Automação](how-to/automacao.md) | [Exit codes e ambiente](reference/exit-codes.md) | [Escada de fidelidade](explanation/escada-de-fidelidade.md) |
| | [Backup e restore](how-to/backup-e-restore.md) | [Autocomplete do editor](reference/autocomplete.md) | [Progresso ao vivo](explanation/progresso-ao-vivo.md) |
| | [Senhas no cofre](how-to/senhas-no-cofre.md) | [Aba Settings](reference/settings-aba.md) | |
| | [Sugestões ao escrever SQL](how-to/autocomplete-no-editor.md) | | [Por que `tokio-postgres`](explanation/por-que-tokio-postgres.md) |
| | | [Contrato de interface](contrato-de-interface.md) | [Por que o autocomplete é assim](explanation/autocomplete.md) |
| | | [Seleção de bases](reference/selecao-de-bases.md) | [Por que a seleção de bases é assim](explanation/selecao-de-bases.md) |
| | | | [Fontes e licença](explanation/fontes.md) |

## A bússola

O que separa os quadrantes não é o assunto, é a intenção do leitor:

| O conteúdo… | …e serve para… | …então é |
|---|---|---|
| informa **ação** | **adquirir** habilidade | **tutorial** |
| informa **ação** | **aplicar** habilidade | **how-to** |
| informa **cognição** | **aplicar** habilidade | **referência** |
| informa **cognição** | **adquirir** habilidade | **explicação** |

Na prática: um tutorial ensina; um how-to resolve; a referência descreve; a explicação justifica.

## O que havia aqui antes

Os 11 documentos anteriores eram do produto .NET (e de um plano de GUI em Electron que nunca existiu
no Mustel). Antes de removê-los, cada um foi minerado — esta tabela registra o destino de cada um.

| Documento antigo | Destino |
|---|---|
| `QUERY_COMMAND.md` | [Referência do `query run`](reference/query-run.md) + [Explicação do CSV](explanation/csv-como-o-npgsql.md) (corrigido: `--exclude` casa **nomes exatos**, não curingas) |
| `MULTI_SERVER.md` | [How-to: rodar em vários bancos](how-to/varios-bancos.md) + [Referência do `query run`](reference/query-run.md) |
| `pt-BR/credenciais.md` | [Explicação: credenciais](explanation/credenciais.md) (já era doc do Mustel; movido) |
| `rust-postgres-spike.md` | [Explicação: por que `tokio-postgres`](explanation/por-que-tokio-postgres.md) |
| `pt-BR/FEATURE_SPECIFICATION.md`, `en/FEATURE_SPECIFICATION.md` | Descartados: descrevem features dos grupos não migrados (`file`, `claude`, `opencode`, `winget`) e o blob DPAPI |
| `pt-BR/ARCHITECTURE.md`, `en/ARCHITECTURE.md` | Descartados: arquitetura .NET (DI, `System.CommandLine`); a do Mustel está no `AGENTS.md` |
| `pt-BR/GUI_REFACTORING_PLAN.md`, `en/GUI_REFACTORING_PLAN.md` | Descartados: plano de GUI em Electron/Angular, abandonado em favor do egui (D5) |
| `pt-BR/SESSION_SUMMARY_2025-03-17.md` | Descartado: diário de sessão, sem valor de contrato |

O contrato vigente não está aqui: está em [`contrato-de-interface.md`](contrato-de-interface.md) e é
verificado por `cargo test --workspace`.
