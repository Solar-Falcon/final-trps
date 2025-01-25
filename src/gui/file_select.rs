use eframe::egui::{self, Color32};
use egui_file_dialog::FileDialog;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct UiFileSelect {
    pub program_file: Option<PathBuf>,
    file_dialog: FileDialog,
}

impl UiFileSelect {
    pub fn display(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if ui.button("Выбрать тестируемый исполняемый файл").clicked()
        {
            self.file_dialog.pick_file();
        }

        self.file_dialog.update(ctx);

        if let Some(path) = self.file_dialog.take_picked() {
            self.program_file = Some(path);
        }

        if let Some(prog_path) = self.program_file.as_ref() {
            ui.separator();
            ui.label(format!("Выбран файл: {}", prog_path.display()));

            if !prog_path.is_file() {
                ui.colored_label(
                    Color32::ORANGE,
                    "Внимание! Выбранный файл не существует или недоступен.",
                );
            }
        }
    }

    #[inline]
    pub fn is_file_selected(&self) -> bool {
        self.program_file.is_some()
    }
}
