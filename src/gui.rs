use crate::{
    run_manager::RunManager,
    worker_thread::{TestReport, TestingData},
};
use anyhow::Result;
use eframe::{
    egui::{self, Color32},
    App,
};
use file_select::UiFileSelect;
use rule_panel::UiRulePanel;
use std::sync::atomic::Ordering;

mod file_select;
mod rule_data;
mod rule_panel;

pub use rule_data::{ContentType, RuleData, RuleType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppState {
    Idle,
    Working,
    Finished,
}

#[derive(Debug)]
pub struct AppGui {
    run_manager: RunManager,
    successes_required: u32,
    state: AppState,

    ui_file_select: UiFileSelect,
    ui_rule_panel: UiRulePanel,
}

impl AppGui {
    pub fn new(cc: &eframe::CreationContext) -> Result<Self> {
        // default is too small (esp on linux)
        cc.egui_ctx.set_zoom_factor(1.75);

        Ok(Self {
            run_manager: RunManager::create_and_start_thread(),
            successes_required: 1,
            state: AppState::Idle,

            ui_file_select: Default::default(),
            ui_rule_panel: Default::default(),
        })
    }

    #[inline]
    fn collect_testing_data(&self) -> TestingData {
        TestingData {
            program_path: self.ui_file_select.program_file.as_ref().unwrap().clone(),
            rules: self.ui_rule_panel.rules().clone(),
            successes_required: self.successes_required,
        }
    }

    fn ui_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.ui_file_select.display(ctx, ui);

        if self.ui_file_select.is_file_selected() {
            ui.separator();

            self.ui_rule_panel.display(ui);

            ui.separator();

            let slider = egui::Slider::new(&mut self.successes_required, 1..=10_000_000)
                .text("Требуемое количество успешных тестов")
                .logarithmic(true)
                .integer();

            ui.add(slider);

            match self.state {
                AppState::Idle if !self.ui_rule_panel.rules().is_empty() => {
                    self.ui_start_button(ui);
                }
                AppState::Idle => {}
                AppState::Working => {
                    self.ui_footer_working(ui);
                }
                AppState::Finished => {
                    self.ui_footer_finished(ui);
                }
            }
        }
    }

    #[inline]
    fn ui_start_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Начать тестирование").clicked() {
            let testing_data = self.collect_testing_data();

            if !self.run_manager.send_testing_data(testing_data) {
                return;
            }

            self.state = AppState::Working;
        }
    }

    fn ui_footer_working(&mut self, ui: &mut egui::Ui) {
        if ui.button("Остановить").clicked() {
            self.run_manager.force_stop_thread();

            self.state = AppState::Finished;
        }

        let tests_solved = self
            .run_manager
            .work_state
            .solved_tests
            .load(Ordering::Acquire);
        let tests_required = self
            .run_manager
            .work_state
            .required_tests
            .load(Ordering::Acquire);

        let progress = (tests_solved as f32) / (tests_required as f32);

        let progress_bar = egui::ProgressBar::new(progress)
            .show_percentage()
            .text(format!("Прогресс: {}/{}", tests_solved, tests_required));

        ui.add(progress_bar);

        match self.run_manager.try_receive_result() {
            Some(true) => {
                self.state = AppState::Finished;
            }
            Some(false) => {}
            None => {
                self.state = AppState::Idle;
            }
        }
    }

    fn ui_footer_finished(&mut self, ui: &mut egui::Ui) {
        ui.separator();

        match self.run_manager.last_report.as_mut() {
            Some(TestReport::Success) => {
                ui.colored_label(Color32::GREEN, "Все тесты прошли успешно");
            }
            Some(TestReport::Failure {
                history,
                error_message,
            }) => {
                ui.colored_label(Color32::DARK_RED, "Обнаружены ошибки:");

                ui.label("История ввода/вывода: ");
                ui.label(format!("{}", history));

                ui.label(error_message.as_str());
            }
            Some(TestReport::Error(error)) => {
                ui.colored_label(Color32::DARK_RED, "Возникла ошибка выполнения: ");
                ui.label(format!("{error}"));
            }
            None => {
                self.state = AppState::Idle;
            }
        }

        self.ui_start_button(ui);
    }
}

impl App for AppGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.ui_main(ctx, ui);
            });
        });
    }
}
