# How-to: atualizar o Mustel

> Você está usando o Mustel e quer a versão mais nova, sem procurar a release à mão.

## Pelo aviso da janela

1. Abra o Mustel. Se houver uma versão nova, aparece no rodapé o aviso
   `Mustel X is available.` — ele não expira sozinho.
2. Clique em **Update**. O aviso vira `Downloading Mustel X...` e a barra mostra o progresso.
3. Quando o download termina, o Mustel fecha e reabre sozinho. Na abertura ele mostra
   `Updated to X (from Y).`

Se mudar de ideia durante o download, clique no **X**: o download é cancelado e os arquivos
parciais são apagados. O Mustel continua na versão atual.

## Pelo About

Se o aviso já foi fechado (ele volta a aparecer depois de 7 dias), ou se você quer conferir agora:

1. Abra **Settings › About**.
2. Clique em **Check for updates**.
3. Se houver versão nova, o aviso sticky aparece no rodapé; siga a partir do passo 2 acima.
4. Se não houver, a janela responde `You are on the latest version (X).`

O link **Release notes** abre a página da release no navegador, com as notas completas.

## Quando a pasta do programa é somente leitura

Se o Mustel está instalado numa pasta do sistema (por exemplo, `/usr/bin`), ele não consegue trocar
o próprio executável. Nesse caso:

1. O download é feito e o Mustel avisa
   `The app folder is read-only. The update was saved to <caminho>; install it manually.`
2. Copie o executável de `<caminho>` sobre o `mustel` instalado (com `sudo`, se for o caso).
3. Abra o Mustel de novo.

## Quando a atualização falha

O aviso vira `Update failed: …` com o botão **Retry** e o motivo. O app continua aberto e
funcionando na versão atual. Tente o **Retry**; se continuar falhando, baixe o executável na página
da release (o link está no About) e substitua o arquivo à mão.

No Windows, feche **as duas** janelas do Mustel (`mustel.exe` e `mustel-gui.exe`) antes de substituir
arquivos à mão — um executável em execução fica travado.

## Conferir a versão

`Mustel <versão>` aparece no topo de **Settings › About**. Pela linha de comando:

```bash
mustel --version
```

## Se você instalou pelo Cargo

O updater não é o canal para binários instalados por `cargo install`; atualize pelo próprio Cargo:

```bash
cargo install --locked mustel --force
```
