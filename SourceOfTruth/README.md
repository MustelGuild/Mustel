# Mustel — Source of Truth (Fonte da Verdade)

Bem-vindo à **Source of Truth** do projeto **Mustel**. Este diretório é o repositório centralizado e oficial para toda a documentação, decisões de arquitetura, manuais de recursos e guias de desenvolvimento.

---

## 🗂️ Estrutura da Documentação

| Arquivo | Descrição |
| :--- | :--- |
| [`architecture.md`](./architecture.md) | Decisões de Arquitetura (ADRs), topologia de módulos, integração Tokio/Iced, segurança de credenciais e concorrência. |
| [`features.md`](./features.md) | Guia completo de funcionalidades (Interface Gráfica Iced, CLI, Executor SQL, Exibição de Resultados, Gerenciamento de Servidores, Exportação CSV e Cofre DPAPI). |
| [`getting-started.md`](./getting-started.md) | Guia de início rápido para desenvolvedores, compilação, testes automatizados e comandos de uso. |

---

## 📌 Sobre o Projeto Mustel

O **Mustel** é uma ferramenta de gerenciamento e automação de banco de dados PostgreSQL multiplataforma (escrita 100% em Rust), combinando:
1. **Interface CLI (Terminal)**: Automação via linha de comando para execução massiva de queries paralelas em múltiplos servidores e exportação para CSV.
2. **Interface GUI Desktop (Iced v0.14)**: Aplicação reativa no modelo Elm (Model-View-Update) com editor SQL nativo, suporte a múltiplos temas visuais (Dark/Light/TokyoNight/Dracula/etc.), validação de queries destrutivas em tempo real e exibição de resultados em grade.

---

> **Nota para Desenvolvedores e Agentes**: Sempre que uma nova funcionalidade for adicionada ou uma decisão arquitetural for tomada, este diretório **SourceOfTruth** deve ser atualizado para refletir o estado exato do sistema.
