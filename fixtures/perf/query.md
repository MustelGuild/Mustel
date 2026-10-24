# Performance — `query run` (export de 100k linhas)

Medição do T36, o gate do `query run`. Compara o oráculo (.NET) com o `mustel` release no mesmo
banco, com a mesma consulta e o mesmo método. **Não há ferramenta para isto**, ao contrário do
`baseline.md`: o oráculo **exige console** para a tabela ao vivo (`AnsiConsole.Live`) — com stdout
redirecionado ele morre com `The handle is invalid` (exit 20) —, então a medição roda com o console
herdado e o pico de RSS é amostrado de fora do processo.

## Como reproduzir

1. Banco descartável com a tabela do fixture (Postgres local, ver a seção 11 do plano):

```sql
CREATE DATABASE mustel_t36;
CREATE TABLE bench AS
  SELECT g AS id, 'linha ' || g AS texto, (g % 7) AS resto, (g % 1000) / 100.0 AS valor
  FROM generate_series(1, 100000) AS g;
```

2. Medir cada binário 10 vezes (PowerShell; `Start-Process -NoNewWindow` **sem** redirecionar, para
   o console existir, e `PeakWorkingSet64` amostrado a cada 10 ms):

```powershell
$conn = "Host=127.0.0.1;Port=55432;Database=mustel_t36;Username=postgres;SslMode=Disable"
$args = @('query','run','--npgsql-connection-string',"`"$conn`"",'-W','dev',
          '-c','"SELECT * FROM bench"','-o',$out)
```

O `--npgsql-connection-string` é deliberado: ele **dispensa o `furlab.jsonc`** (servidor ad-hoc), o
que torna a medição independente da configuração do usuário — e a senha vai no `-W` porque o
caminho ad-hoc **descarta a senha da string** (pegadinha medida no T32).

## Resultado

Mediana de 10 execuções, cada linha o processo inteiro (partida, consulta, escrita dos CSVs,
consolidado e resumo).

| métrica | oráculo (.NET) | mustel (release) | razão |
|---|---|---|---|
| tempo (mediana) | 2,72 s | **1,07 s** | **2,5× mais rápido** |
| tempo (mínimo–máximo) | 2,62–2,85 s | 0,95–1,25 s | |
| pico de RSS (mediana) | 318,8 MB | **11,6 MB** | **27,5× menos memória** |
| pico de RSS (mínimo–máximo) | 313,5–319,1 MB | 11,5–11,6 MB | |
| saída (`127.0.0.1_<ts>.csv`) | 6 118 231 bytes | 6 118 231 bytes | **byte a byte igual** |
| linhas exportadas | 100 000 | 100 000 | |

## Por que a diferença de memória

Não é ajuste fino: é a diferença entre materializar e transmitir. O legado lê as 100k linhas para
dentro de um `List<Dictionary<string,string>>` (uma `Dictionary` por linha, cada valor uma string
nova) antes de escrever qualquer coisa, então o pico acompanha o resultado — e cresce com ele. O
Mustel transmite: o resultado do driver chega como fluxo (`QueryStream` sobre `query_raw`) e as
linhas vão para o arquivo em blocos de 64 (T34), então o pico não depende do tamanho do resultado.

O tempo segue a mesma causa (o legado aloca ~100k dicionários e depois escreve; o Mustel escreve
enquanto lê) mais a partida do runtime.

## Paridade do fixture

O CSV consolidado e o por-servidor dos dois lados foram comparados com `cmp`: **idênticos**. O
fixture exercita o que o export tem de sensível — texto com prefixo, `int4`, `int4` com módulo e
`numeric` com escala (`0.01000000000000000000`, 20 casas preservadas).

**Ressalvas**: (1) o pico de RSS do oráculo inclui o runtime .NET, mas isso não explica 318 MB —
100k dicionários custam isso; (2) o pico é **amostrado** a cada 10 ms, então um pico mais curto que
isso poderia passar batido (no Mustel a memória é estável, e no legado ela fica alta por segundos);
(3) a máquina é a mesma dos dois lados, e a medição foi feita com o Docker parado.

O comparativo final (.NET × Rust) é o T46, com todas as métricas reais.
