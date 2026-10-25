# Por que o autocomplete funciona assim

O autocomplete da janela parece um detalhe de interface, mas cada decisão dele responde a uma
restrição do produto: o Mustel não é um cliente SQL genérico, e a janela não tem prompt de senha.

Para o que existe, tecla por tecla, veja a [referência](../reference/autocomplete.md).

## Metadados do catálogo, não DDL

O que o editor precisa é de **nomes e tipos**, não da definição das tabelas. Ler `pg_class`,
`pg_attribute` e `pg_proc` custa duas consultas e devolve exatamente isso; gerar DDL
(`pg_dump --schema-only`) custaria muito mais e traria chaves, índices e privilégios que não entram em
nenhuma sugestão.

Consequência assumida: o autocomplete **não** completa colunas de CTE nem nomes de constraints. Quem
quer a estrutura, usa o `pg_dump` — que já faz isso bem e continua sendo a ferramenta certa.

## Um banco de referência, e não todos

O retrato descreve **um** banco: o escolhido na linha "Load autocomplete". A alternativa —
carregar o catálogo de todos os servidores marcados — multiplicaria o custo de cada `⟳` pelo número de
bancos e encheria a lista de homônimos que não se relacionam.

A aposta é a do uso em massa: quem roda a mesma consulta em vários bancos costuma ter bancos com o
mesmo esquema, então um molde serve para todos. Quem tem bancos diferentes digita sem ajuda — e o
rótulo `another database loaded` existe para deixar claro que o molde não bate com a escolha atual.

## Uma cópia em memória, atualizada à mão

A cada tecla, o motor lê um retrato guardado em memória. Isso é o que faz a sugestão ser instantânea e
é o motivo de não haver ida ao banco enquanto você digita.

O preço é que o retrato **envelhece**: DDL feito fora da janela só aparece depois de outro `⟳`. A
escolha foi não recarregar sozinho — atualizar sem avisar custaria conexão com o banco em momentos
imprevisíveis (e pode pedir senha que a janela não tem). O rótulo de estado e o botão `⟳` são o
consolo: o que está em memória é sempre o que você mandou carregar.

## O motor é uma função pura, no núcleo

`mustel_core::completion::suggest(texto, cursor, &schema)` recebe o texto, a posição do cursor e o
esquema já em memória, e devolve candidatos. Não abre conexão, não desenha, não conhece egui.

Duas razões:

- **teste sem janela** — as regras (contexto, alias, CTE, qualificação por schema, ranking) são
  exercitadas com um esquema fictício, sem banco e sem clique;
- **a regra de string/comentário vive num lugar só** — o realce de sintaxe da GUI delega para ela. Antes
  havia duas implementações, e a do GUI errava o limite (tratava o espaço depois do fecha-aspas como
  "dentro"); hoje é uma.

A GUI fica com o que é dela: teclado, foco e o popup.

## O popup é do egui; as regras não são

O `CompletionPopup` do egui 0.37 já consome `Tab`, `Enter` e as setas **antes** do `TextEdit`, guarda a
seleção e insere o texto aceito. Reescrever isso seria reimplementar teclado, foco e posicionamento
para ganhar nada.

O que o egui **não** decide — e por isso mora em `mustel-gui/src/sql_completion.rs` — são três coisas:

1. **onde começa a palavra**: o popup pergunta quais caracteres *separam* palavras, o inverso do que o
   motor pergunta. Passar a lista invertida foi um bug real (ver abaixo);
2. **quando abrir**: no começo de um token, na primeira letra, ou por `Ctrl+Space`;
3. **quanto tempo o pedido vale**: o popup é reavaliado a cada quadro, então atalho e aceite precisam
   de memória entre quadros.

## Duas lições que só o uso real deu

O primeiro desenho passou nos testes e falhou na janela. Vale registrar, porque o erro foi de
**método**, não de digitação:

- **A fronteira de palavra estava invertida.** O popup recebia os caracteres que *formam* a palavra
  onde ele esperava os que a *separam*. Resultado: a "palavra" era o espaço anterior ao identificador
  — nada abria ao digitar, e aceitar grudava o texto (`select` + `nome` = `selectnome`). O mínimo de
  "2 letras" também passou a medir espaços, o que explica o sintoma de "só abre no terceiro espaço".
- **`Ctrl+Space` abria e fechava em um quadro.** O atalho vale no quadro da tecla e o popup era
  reavaliado no quadro seguinte, quando o pedido já tinha sumido. O log da janela mostrou
  `is_open=true` seguido de `is_open=false`.

A causa comum: o teste headless conferia **um** quadro. Os testes de hoje digitam como o winit entrega
(um evento por caractere) e conferem os quadros seguintes, inclusive o fechamento depois do aceite.

## Por que `Esc` e o meio da palavra incomodam

Duas limitações registradas na [referência](../reference/autocomplete.md#limitações-conhecidas) não
são decisões de produto:

- o `Esc` é guardado pelo egui **pela palavra** (`dismissed_word`), não pela posição. Fechar numa
  palavra vazia silencia a lista em toda palavra vazia até você digitar uma letra;
- aceitar com o cursor no meio da palavra substitui só o trecho à esquerda do cursor, porque é o
  contrato do `CompletionPopup` (`delete_char_range` + `insert_text_at`).

Consertar o segundo exige apagar o resto do identificador depois do aceite e reposicionar o cursor —
mexer no estado do `TextEdit`, não no motor. Ficou registrado como pendência em vez de escondido.

## De onde vem a lista de bancos

O combo de banco não lê o cadastro: ele lê o **servidor**. Escolher um servidor dispara uma descoberta
em background (`pg_database`, a mesma consulta do `settings db-servers test`), porque o cadastro pode
listar só parte dos bancos — ou nenhum, quando o servidor é `fetchAllDatabases`.

A mesma ida ao servidor responde se o cofre tem senha, então é uma só: o `⟳` só habilita quando há
senha guardada, e nunca há prompt. Sem descoberta (conexão recusada, por exemplo), a lista cai para o
cadastro e a janela continua utilizável.

## O que ainda falta

Nada bloqueante conhecido: as duas lacunas que esta página registrava — a descoberta de bancos e o
botão habilitado sem senha — foram fechadas. O que resta é o que está na lista de limitações da
[referência](../reference/autocomplete.md#limitações-conhecidas): os dois defeitos (o `Esc` do egui e o
aceite no meio da palavra), que são o próximo trabalho se alguém quiser encarar.
