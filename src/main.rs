use eframe::{
    egui::{Vec2, ViewportBuilder},
    NativeOptions,
};
use gui::AppGui;
use std::sync::LazyLock;
use time::format_description::OwnedFormatItem;

pub mod strategies;
pub mod communicator;
pub mod gui;
pub mod runner;

static DATE_FORMAT: LazyLock<OwnedFormatItem> = LazyLock::new(|| {
    time::format_description::parse_owned::<2>("[year]-[month]-[day] [hour]-[minute]-[second]")
        .unwrap()
});

fn main() {
    let native_options = NativeOptions {
        viewport: ViewportBuilder {
            title: Some("Программа автоматизации тестирования ПО".to_owned()),
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
