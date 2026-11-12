//! Janela do Mustel **sem console** — o binário que se abre com duplo clique no Windows.
//!
//! O binário principal (`mustel`) é um programa de console: o Windows cria uma janela de terminal
//! antes do `main`, e ela fica atrás da janela do egui. Este binário é compilado para o subsistema
//! gráfico do Windows (`windows_subsystem = "windows"`), então nenhum console é criado.
//!
//! Ele **não** chama o `mustel`: os dois linkam a mesma biblioteca `mustel-gui` e são
//! independentes. `mustel` continua sendo o CLI (e abre a janela quando roda sem subcomando num
//! terminal); `mustel-gui` serve para quem quer só a janela, sem terminal nenhum.
//!
//! Em plataformas que não são Windows o atributo é inerte e o binário é um lançador comum — ele não
//! vai no pacote do Linux, onde o `.desktop` chama `mustel gui`.

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    // Modo oculto do updater, antes de a janela existir: o helper é uma **cópia** deste exe e
    // precisa conseguir trocar os arquivos sem abrir nada.
    if let Some(code) = mustel_core::update::apply_from_args(std::env::args_os().skip(1)) {
        std::process::exit(code);
    }

    if let Err(error) = mustel_gui::run() {
        // Sem console no Windows, esta linha não aparece quando o binário é aberto pelo Explorer.
        // A falha típica é não conseguir criar a janela (sem OpenGL, por exemplo); o exit code 1
        // continua sendo a informação que um atalho ou script consegue observar.
        eprintln!("mustel-gui: {error}");
        std::process::exit(1);
    }
}
