# Política de segurança

> Relatos são aceitos em português ou inglês. / Reports are welcome in Portuguese or English.

## Versões com suporte

Só a última release publicada recebe correções de segurança. O comando `mustel --version` diz qual
versão está instalada.

## Como relatar uma vulnerabilidade

Use o **relato privado de vulnerabilidade** do GitHub: aba **Security** → **Report a vulnerability**.
Não abra issue pública com detalhes de exploração — o relato público transforma a falha em problema
para quem já usa o Mustel.

Se o botão não estiver disponível no repositório, abra uma issue **sem detalhes técnicos** pedindo um
canal privado.

Inclua, quando possível:

- a versão (`mustel --version`) e a plataforma (Windows ou Linux, com a distribuição);
- os passos para reproduzir e o impacto observado;
- se o caminho envolve credenciais (cofre, `pgpass`, `--password-stdin`), TLS ou backup/restore;
- qualquer prova de conceito, sem dados reais de produção.

## Escopo

Interessam, entre outros:

- o CLI e a GUI (`crates/`);
- o armazenamento de credenciais: cofre do sistema operacional e `pgpass`
  (ver [`docs/explanation/credenciais.md`](docs/explanation/credenciais.md));
- a validação de TLS do driver
  (ver [`docs/explanation/tls-postgres.md`](docs/explanation/tls-postgres.md));
- o classificador de consultas destrutivas;
- a cadeia de suprimentos: SBOM, licenças e actions pinadas por SHA
  (ver [`docs/explanation/seguranca.md`](docs/explanation/seguranca.md)).

Ficam **fora** do escopo falhas do próprio PostgreSQL, do cofre do sistema operacional e das
dependências de terceiros — essas devem ser relatadas aos projetos de origem. A limitação conhecida
do TLS em `database backup`/`restore` é uma trava deliberada, documentada em
[`docs/explanation/tls-postgres.md`](docs/explanation/tls-postgres.md), e não um defeito a relatar.

## O que esperar

Análise e correção em regime de melhor esforço, com divulgação coordenada depois que houver versão
corrigida.
