# Ícone da janela no Wayland

Você abre `mustel gui` no Linux e a barra de título mostra o ícone genérico do Wayland no lugar do
Mustel.

## Por que acontece

No Wayland **não existe ícone por janela**: o protocolo `xdg-shell` não tem esse pedido. A barra de
título é desenhada pelo compositor, e ele descobre a arte procurando um arquivo `.desktop` cujo nome
(sem o `.desktop`) seja igual ao `app_id` que a janela declarou. O `Icon=` desse arquivo aponta para
o nome do ícone instalado no tema.

O `with_icon` do `eframe`/`winit` — que resolve no X11 e no Windows — é **ignorado** nesse caminho.
Sem um `.desktop` casando com o `app_id`, o compositor não tem como saber qual arte usar e desenha o
ícone genérico.

O Mustel declara `app_id` = `mustel`; logo, o compositor procura `mustel.desktop` e um ícone
chamado `mustel`. Essa é a decisão `A7` em [Decisões de engenharia](../explanation/decisoes.md); a
linha de contrato é `IF-257`.

## O que instalar

Os dois arquivos vêm no pacote de release (e também em `crates/mustel-gui/assets/`):

```bash
install -Dm644 mustel.desktop ~/.local/share/applications/mustel.desktop
install -Dm644 mustel.png ~/.local/share/icons/hicolor/256x256/apps/mustel.png
update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

Feche e abra `mustel gui` de novo — o `app_id` é declarado na criação da janela, então não basta
trocar de workspace.

Quem usa X11 ou Windows não precisa de nada disso: nesses ambientes o ícone vai embutido no binário.

## Rodando dentro de um container (podman)

O compositor roda **no host**. O `.desktop` e o ícone têm de estar no `$HOME` de quem roda o
compositor — não no `$HOME` interno do container, a menos que ele seja exatamente o mesmo diretório
montado do host.

Se o container tem home próprio, instale no host, com os arquivos copiados para fora:

```bash
# no host
install -Dm644 mustel.desktop ~/.local/share/applications/mustel.desktop
install -Dm644 mustel.png ~/.local/share/icons/hicolor/256x256/apps/mustel.png
```

Se o binário roda em um container **sem** esse `$HOME` compartilhado, a janela continua abrindo (o
socket Wayland é o que importa para isso), mas o ícone não tem como ser encontrado pelo compositor.
