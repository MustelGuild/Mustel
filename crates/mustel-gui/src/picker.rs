//! Seletor de pastas do Mustel — uma janela do próprio app, não o diálogo do SO.
//!
//! Por que próprio: o `egui-file-dialog` (a opção natural) exige egui `^0.36` e o projeto está em
//! `0.37`; adotá-lo exigiria voltar o framework uma versão. Um seletor mínimo entrega a mesma UX
//! (janela interna, sem diálogo nativo) sem dependência nova.
//!
//! A navegação é **função pura sobre `std::fs`** ([`subdirectories`], [`parent_of`], [`roots`]), para
//! ser testável sem abrir janela. O desenho ([`Picker::show`]) é a camada fina em cima disso.

use std::path::{Path, PathBuf};

/// Quantos níveis de caminho o cabeçalho mostra antes de cortar.
const PATH_TAIL_SEGMENTS: usize = 3;

/// Estado do seletor.
#[derive(Debug, Default)]
pub struct Picker {
    /// `Some` quando a janela está aberta.
    current: Option<PathBuf>,
    /// Mensagem de erro da última leitura de diretório.
    error: Option<String>,
}

impl Picker {
    /// Abre o seletor já posicionado em `start` (ou no primeiro ancestral que existir).
    pub fn open(&mut self, start: &Path) {
        let start = existing_ancestor(start);
        self.error = None;
        self.current = Some(start);
    }

    /// Fecha sem escolher.
    pub fn close(&mut self) {
        self.current = None;
        self.error = None;
    }

    /// Diretório em exibição, se aberto.
    pub fn current(&self) -> Option<&Path> {
        self.current.as_deref()
    }

    /// Vai para `directory` (usado pelos testes e pelos atalhos de raiz).
    pub fn navigate_to(&mut self, directory: PathBuf) {
        self.current = Some(directory);
        self.error = None;
    }

    /// Sobe um nível, quando há para onde subir.
    pub fn go_up(&mut self) {
        if let Some(current) = &self.current
            && let Some(parent) = parent_of(current)
        {
            self.navigate_to(parent);
        }
    }

    /// Desenha a janela e devolve a pasta escolhida, se o usuário confirmar.
    ///
    /// Devolver `Some` já fecha o seletor: quem chama só precisa gravar o caminho na caixa.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        let current = self.current.clone()?;
        let mut chosen = None;
        let mut stay_open = true;

        let modal = egui::Modal::new(egui::Id::unique("folder_picker")).show(ctx, |ui| {
            ui.set_max_width(560.0);
            ui.set_min_width(520.0);
            ui.heading("Escolher pasta");

            ui.horizontal(|ui| {
                if crate::icons::button_enabled(
                    ui,
                    crate::icons::Icon::ArrowUp,
                    parent_of(&current).is_some(),
                )
                .on_hover_text("Subir um nível")
                .clicked()
                {
                    self.go_up();
                }
                ui.label(short_path(&current));
            });

            ui.separator();

            // Raízes (drives no Windows): só quando não há pai, para não poluir a lista.
            if parent_of(&current).is_none() {
                ui.horizontal_wrapped(|ui| {
                    for root in roots() {
                        if ui.button(root.display().to_string()).clicked() {
                            self.navigate_to(root);
                        }
                    }
                });
                ui.separator();
            }

            let entries = match subdirectories(&current) {
                Ok(entries) => {
                    self.error = None;
                    entries
                }
                Err(error) => {
                    self.error = Some(format!("Não consegui ler a pasta: {error}"));
                    Vec::new()
                }
            };

            egui::ScrollArea::vertical()
                .max_height(280.0)
                .id_salt("folder_picker_entries")
                .show(ui, |ui| {
                    if entries.is_empty() && self.error.is_none() {
                        ui.weak("Nenhuma subpasta aqui.");
                    }
                    for entry in &entries {
                        let name = entry
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| entry.display().to_string());
                        if crate::icons::selectable(ui, crate::icons::Icon::Folder, &name).clicked()
                        {
                            self.navigate_to(entry.clone());
                        }
                    }
                });

            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Selecionar esta pasta").clicked() {
                    chosen = Some(current.clone());
                    stay_open = false;
                }
                if ui.button("Cancelar").clicked() {
                    stay_open = false;
                }
            });
        });

        // Esc fecha, como qualquer diálogo.
        if modal.should_close() {
            stay_open = false;
        }
        if !stay_open {
            self.close();
        }

        chosen
    }
}

/// Subpastas de `directory`, ordenadas por nome sem diferenciar maiúsculas.
///
/// Só diretórios: escolher pasta não deve oferecer arquivo. Entradas ilegíveis são ignoradas em vez de
/// derrubar a lista inteira — uma pasta protegida no meio não pode esconder as outras.
pub fn subdirectories(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(directory)? {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            entries.push(path);
        }
    }

    entries.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    Ok(entries)
}

/// Pai de `directory`, se houver.
///
/// `parent()` devolve `None` na raiz (`C:\` ou `/`), que é exatamente quando o botão de subir precisa
/// ficar desabilitado — não há caso especial a inventar.
pub fn parent_of(directory: &Path) -> Option<PathBuf> {
    directory.parent().map(Path::to_path_buf)
}

/// Raízes navegáveis.
///
/// No Windows não há API em `std` para listar drives; sondar as letras que existem é barato (26
/// `is_dir`) e não traz dependência. No Unix a raiz é uma só.
pub fn roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut drives = Vec::new();
        for letter in b'A'..=b'Z' {
            let candidate = PathBuf::from(format!("{}:\\", letter as char));
            if candidate.is_dir() {
                drives.push(candidate);
            }
        }
        drives
    } else {
        vec![PathBuf::from("/")]
    }
}

/// Primeiro ancestral de `path` que existe, incluindo o próprio.
///
/// A caixa pode apontar para uma pasta que ainda não foi criada (a consulta não rodou). Abrir o
/// seletor no ancestral mais próximo é mais útil que abrir numa pasta inexistente e mostrar erro.
fn existing_ancestor(path: &Path) -> PathBuf {
    let mut candidate = path.to_path_buf();
    loop {
        if candidate.is_dir() {
            return candidate;
        }
        match candidate.parent() {
            Some(parent) => candidate = parent.to_path_buf(),
            None => return std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

/// Caminho encurtado para o cabeçalho: as últimas `PATH_TAIL_SEGMENTS` pastas, com `…` na frente.
fn short_path(path: &Path) -> String {
    let segments: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();

    if segments.len() <= PATH_TAIL_SEGMENTS {
        return path.display().to_string();
    }

    format!(
        "…{}",
        segments[segments.len() - PATH_TAIL_SEGMENTS..].join("/")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lista só subpastas, em ordem alfabética sem diferenciar maiúsculas.
    #[test]
    fn subdirectories_lists_only_directories_sorted() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("zebra")).expect("cria");
        std::fs::create_dir(dir.path().join("Alfa")).expect("cria");
        std::fs::create_dir(dir.path().join("beta")).expect("cria");
        std::fs::write(dir.path().join("arquivo.txt"), "x").expect("escreve");

        let names: Vec<String> = subdirectories(dir.path())
            .expect("lê")
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            names,
            vec!["Alfa", "beta", "zebra"],
            "sem arquivo, em ordem"
        );
    }

    /// Pasta ilegível vira erro, não pânico.
    #[test]
    fn subdirectories_reports_a_missing_directory() {
        let missing = std::path::Path::new("/nao/existe/mesmo");
        assert!(subdirectories(missing).is_err());
    }

    /// A raiz não tem pai; um nível abaixo tem.
    #[test]
    fn parent_stops_at_the_root() {
        let root = if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };
        assert_eq!(parent_of(&root), None, "raiz não tem pai");
        assert_eq!(parent_of(&root.join("a")), Some(root));
    }

    /// Sempre há pelo menos uma raiz navegável.
    #[test]
    fn roots_is_never_empty() {
        assert!(!roots().is_empty());
    }

    /// Abrir numa pasta que não existe cai no ancestral existente.
    #[test]
    fn opening_a_missing_directory_falls_back_to_the_existing_ancestor() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("ainda-nao-existe").join("nem-esta");

        let mut picker = Picker::default();
        picker.open(&missing);

        assert_eq!(
            picker.current(),
            Some(dir.path()),
            "cai no ancestral existente"
        );
    }

    /// Subir para no topo em vez de sair da raiz.
    #[test]
    fn go_up_stops_at_the_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut picker = Picker::default();
        picker.open(dir.path());
        let start = picker.current().map(Path::to_path_buf).expect("aberto");

        picker.go_up();
        assert_eq!(picker.current(), parent_of(&start).as_deref());
    }

    /// Caminho longo é encurtado no cabeçalho, sem perder o fim.
    #[test]
    fn a_long_path_is_shortened_in_the_header() {
        let path = if cfg!(windows) {
            PathBuf::from("C:\\a\\b\\c\\d\\e")
        } else {
            PathBuf::from("/a/b/c/d/e")
        };
        let short = short_path(&path);
        assert!(short.starts_with('…'), "encurtou: {short}");
        assert!(
            short.ends_with("d/e") || short.ends_with("d\\e"),
            "termina no fim: {short}"
        );
    }

    /// O seletor desenha num contexto do `egui` sem janela.
    ///
    /// É o teste que pega erro de layout/borrow antes de alguém abrir a GUI — mesmo padrão do modal
    /// de confirmação da tela de consulta.
    #[test]
    fn the_picker_renders_without_a_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("subpasta")).expect("cria");

        let ctx = egui::Context::default();
        let mut picker = Picker::default();
        picker.open(dir.path());

        let output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            let inner = ctx.clone();
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = picker.show(&inner);
                let _ = ui;
            });
        });

        assert!(!output.shapes.is_empty(), "o seletor desenhou alguma coisa");
        let mut textures = output.textures_delta;
        textures.clear();
        assert!(
            picker.current().is_some(),
            "continua aberto enquanto não escolher"
        );
    }
}
