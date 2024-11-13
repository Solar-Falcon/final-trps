use eframe::{
    egui::{Vec2, ViewportBuilder},
    NativeOptions,
};
use gui::AppGui;

pub mod communicator;
pub mod converter;
pub mod generator;
pub mod gui;
pub mod runner;

pub const PERSISTANCE_FILE_NAME: &str = "errors.txt";

fn main() {
    let native_options = NativeOptions {
        viewport: ViewportBuilder {
            title: Some("Программа автоматизации тестирования ПО".to_owned()),
            drag_and_drop: Some(true),
            min_inner_size: Some(Vec2::new(1200.0, 600.0)),
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
