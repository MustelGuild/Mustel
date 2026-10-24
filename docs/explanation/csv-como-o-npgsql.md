# Por que o CSV sai como o Npgsql

O `query run` exporta CSV. O formato não foi escolhido por gosto: foi **observado** byte a byte no
binário .NET anterior e congelado no baseline. Esta página explica as decisões que parecem estranhas.

## BOM + CRLF

Todo CSV começa com **BOM UTF-8** e usa **CRLF**, inclusive no Linux. É o que o `CsvHelper` do .NET
fazia com as opções usadas; um arquivo "mais limpo" (sem BOM, LF) quebraria a comparação e mudaria o
que o Excel faz ao abrir.

## Tipos que viram texto do .NET

O export usava `reader.GetValue(i)?.ToString()`, então o que aparece é a representação do **tipo
.NET**, não a do Postgres:

| Tipo | Texto |
|---|---|
| `bool` | `True` / `False` (maiúscula) |
| `bytea` | `System.Byte[]` (o nome do tipo, não o conteúdo) |
| arrays | `System.String[]` |
| `numeric` | `0.00` (com as casas do tipo) |
| `float8` | `2` (sem `.0` quando inteiro) |
| `float4` | formatado com precisão de **f32** (não de f64) |
| `timestamptz` | `10/18/2026 10:52:15 AM` |
| `date` | `10/18/2026` |
| `time` | `10:52 AM` |
| `uuid` | minúsculo com hífens |
| `NULL` | campo vazio |

`float4` é o caso mais traiçoeiro: converter para f64 e imprimir muda o último dígito em valores como
`0.1`. O Mustel guarda o valor como f32 e formata a partir dele.

## Cultura fixa en-US

No .NET, `DateTime.ToString()` usa a **cultura da máquina**. O Mustel fixa en-US, que é o que a
máquina de referência produzia. Numa máquina pt-BR, o produto anterior escreveria `18/10/2026` e
`1500,75`; o Mustel **não** acompanha — é divergência aceita e registrada.

## Por que não usar o writer do `csv` crate direto

Usamos — mas com as opções acima e uma regra de aspas restrita: aspas **só** quando o campo contém
vírgula, aspas ou quebra de linha. Campos com espaços nas pontas ficam sem aspas, como no original.

## Escrita conforme chega, e a intercalação

As linhas vão para o arquivo do servidor em **blocos de 64**, à medida que o driver entrega — o
resultado inteiro nunca é materializado em memória. Isso é o que mantém a memória estável num export
de 1M de linhas.

Duas consequências visíveis no arquivo:

1. **Falha no meio deixa o parcial.** Uma consulta que emite linhas e falha depois deixa no arquivo
daquele banco o que já foi gravado, e **não** é retentada — retentar duplicaria linhas.
2. **Dois bancos do mesmo servidor intercalam.** Eles escrevem no mesmo arquivo e a ordem dos blocos
segue quem termina primeiro. A ordem das linhas de bancos diferentes **não é determinística**; dentro
de um banco, é a ordem da consulta.

No caminho de sucesso não há diferença em relação ao produto anterior: com 1, 2 ou 3 bancos o arquivo
sai byte a byte igual. A diferença só aparece nos dois casos acima, e ambos estão registrados em
[Diferenças deliberadas](diferencas-deliberadas.md) (item 15).

O consolidado é montado **no fim**, a partir dos arquivos por servidor, unindo as colunas na ordem em
que aparecem pela primeira vez.

## Onde isso é garantido

`crates/mustel-core/src/postgres/value.rs` (tipo → texto) e
`crates/mustel-core/src/query/csv_writer.rs` (BOM, CRLF, aspas, cabeçalhos). Ambos têm teste por
caso, e o replay do baseline confere o arquivo consolidado.
