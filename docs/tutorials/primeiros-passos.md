# Primeiros passos

Nesta lição você instala o Mustel, cadastra um servidor, roda uma consulta em um banco de verdade e
abre o CSV que ele gera. Leva uns 15 minutos.

A lição tem duas metades: a **primeira não precisa de banco nenhum** (você aprende a interface e vê o
cadastro funcionar); a **segunda sobe um PostgreSQL descartável** em container.

> Se você travar em algum passo, não presuma que o erro é seu: é bug da documentação. Abra uma issue
dizendo em qual passo parou.

## Antes de começar

- O binário `mustel` em algum diretório do seu `PATH` (ver o [README](../README.md#installation)).
- Docker (ou Podman) **só para a segunda metade**.

Confira que ele responde:

```bash
mustel --version
```

Você deve ver algo como `mustel 1.0.0`.

---

## Parte 1 — Sem servidor nenhum

### 1. Veja o que existe

```bash
mustel --help
```

A árvore tem três grupos e a janela:

- `query` — roda SQL e exporta CSV;
- `database` — backup, restore e `pgpass.conf`;
- `settings` — o cadastro de servidores;
- `gui` — a interface gráfica.

### 2. Descubra onde fica a configuração

```bash
mustel settings db-servers ls
```

Na primeira execução, o Mustel cria um `mustel.jsonc` vazio e responde:

```text
No servers configured. Use 'mustel settings db-servers add' to add one.
```

No Windows o arquivo fica em `%APPDATA%\Mustel\mustel.jsonc`; no Linux, em
`~/.config/Mustel/mustel.jsonc`.

### 3. Cadastre um servidor

```bash
mustel settings db-servers add -n tutorial -H localhost -p 55432 -U postgres -d mustel_tutorial
```

A resposta orienta o próximo passo:

```text
Server 'tutorial' added successfully.
Use 'mustel settings db-servers set-password tutorial' to set the password.
```

### 4. Veja o cadastro

```bash
mustel settings db-servers ls
```

Repare no que **não** está lá: nenhuma senha. Ela vai para o cofre do sistema operacional, não para o
arquivo. Você pode abrir o `mustel.jsonc` e conferir.

### 5. Guarde a senha no cofre

```bash
mustel settings db-servers set-password tutorial
```

O Mustel pede a senha sem ecoá-la na tela. Para este tutorial, use `dev`.

---

## Parte 2 — Com um banco de verdade

### 6. Suba um PostgreSQL descartável

```bash
docker run -d --name mustel-tutorial \
  -e POSTGRES_PASSWORD=dev \
  -e POSTGRES_DB=mustel_tutorial \
  -p 55432:5432 \
  postgres:18.6
```

A versão está **fixada** de propósito: o `latest` muda e este texto deixaria de ser verdadeiro.

Confira que subiu:

```bash
docker exec mustel-tutorial pg_isready -U postgres
```

### 7. Rode sua primeira consulta

```bash
mustel query run -c "select 1 as numero, 'olá' as texto"
```

O Mustel pede a seleção de servidores (o `tutorial` já vem marcado), executa e grava os arquivos.

### 8. Abra o CSV

Cada execução escreve numa pasta com o timestamp dela, dentro do diretório de saída (padrão
`./results`):

```text
results/
└── <timestamp>/
    ├── consolidated_<timestamp>.csv   # todos os resultados, coluna Server + Database
    ├── <servidor>_<timestamp>.csv     # os resultados daquele servidor
    └── <timestamp>_log.csv            # uma linha por banco: status, linhas e duração
```

Abra o consolidado. Ele começa com **BOM UTF-8**, usa **CRLF** e tem as colunas `Server`, `Database`,
`numero`, `texto`:

### 9. Faça uma consulta que fala com uma tabela

```bash
mustel query run -c "create table if not exists notas (id int, texto text)"
mustel query run -c "insert into notas values (1, 'primeira')"
mustel query run -c "select * from notas"
```

O segundo comando é destrutivo, então o Mustel **pede confirmação** (com `sim` como resposta padrão).
Em automação, use `--no-confirm`.

> `_erros.csv` só aparece quando algum banco falha; numa execução limpa ele não existe.

### 10. Limpe

```bash
docker rm -f mustel-tutorial
mustel settings db-servers rm -n tutorial
```

## Para onde ir agora

- Quer guardar senhas e rodar em CI? [Credenciais](../explanation/credenciais.md) e
  [Automação](../how-to/automacao.md).
- Quer rodar contra vários bancos de uma vez? [Vários bancos](../how-to/varios-bancos.md).
- Quer o mapa completo de flags? [Referência do `query run`](../reference/query-run.md).
