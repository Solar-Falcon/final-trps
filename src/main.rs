use eframe::{egui::ViewportBuilder, NativeOptions};
use gui::AppGui;

pub mod communicator;
pub mod generator;
pub mod gui;
pub mod parser;
pub mod runner;

pub const PERSISTANCE_FILE_NAME: &str = "errors.txt";

fn main() {
    let native_options = NativeOptions {
        viewport: ViewportBuilder {
            title: Some("Программа автоматизации тестирования ПО".to_owned()),
            drag_and_drop: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };

    match eframe::run_native(
        "patpo",
        native_options,
        Box::new(|cc| match AppGui::new(cc) {
            Ok(app) => Ok(Box::new(app)),
            Err(error) => Err(format!("{error}").into()),
        }),
    ) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error}");
        }
    }
}
