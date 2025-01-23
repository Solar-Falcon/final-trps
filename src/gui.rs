use crate::runner::{RuleType, RuleData, ContentType, Runner, TestReport, TestingData};
use anyhow::Result;
use eframe::{
    egui::{self, Color32},
    App,
};
use egui_file_dialog::FileDialog;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
};

#[derive(Debug, Default)]
struct UiFileSelect {
    program_file: Option<PathBuf>,
    file_dialog: FileDialog,
}

impl UiFileSelect {
    fn display(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
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
    fn is_file_selected(&self) -> bool {
        self.program_file.is_some()
    }
}

#[derive(Debug, Default)]
struct UiRulePanel {
    cursor: usize,
    rules: Vec<RuleData>,
}

impl UiRulePanel {
    #[inline]
    fn shift_cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    #[inline]
    fn shift_cursor_down(&mut self) {
        self.cursor = self
            .cursor
            .saturating_add(1)
            .min(self.rules.len().saturating_sub(1));
    }

    fn display(&mut self, ui: &mut egui::Ui) {
        self.display_rule_creation(ui);

        ui.separator();

        ui.horizontal(|ui| {
            self.display_main_panel(ui);

            ui.separator();

            self.display_rule_list(ui);
        });
    }

    fn display_rule_creation(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Список правил").show_index(
            ui,
            &mut self.cursor,
            self.rules.len(),
            |i| {
                self.rules
                    .get(i)
                    .map(|rule| rule.name.as_str())
                    .unwrap_or("Ничего нет!")
            },
        );

        ui.horizontal(|ui| {
            if ui.button("Добавить в конец списка").clicked() {
                self.rules.push(Default::default());
                self.cursor = self.rules.len() - 1; // cursor on the new rule
            }

            if !self.rules.is_empty() {
                if ui.button("Добавить после выбранного").clicked() {
                    self.cursor += 1;
                    self.rules.insert(self.cursor, Default::default());
                }

                if ui.button("Добавить перед выбранным").clicked() {
                    self.rules.insert(self.cursor, Default::default());
                }
            }
        });

        if ui.button("Удалить выбранное правило").clicked() {
            self.rules.remove(self.cursor);
            self.shift_cursor_up(); // when we remove the last rule, cursor points to nothing
        }
    }

    fn display_main_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            if self.cursor < self.rules.len() {
                let rule = &mut self.rules[self.cursor];

                ui.horizontal(|ui| {
                    ui.label("Название: ");
                    ui.text_edit_singleline(&mut rule.name);
                });

                ui.horizontal(|ui| {
                    ui.label("Тип параметра: ");
                    ui.radio_value(&mut rule.rule_type, RuleType::Input, "Входной");
                    ui.radio_value(&mut rule.rule_type, RuleType::Output, "Выходной");
                });

                ui.horizontal(|ui| {
                    ui.label("Тип данных: ");
                    ui.radio_value(&mut rule.content_type, ContentType::PlainText, "Текст");
                    ui.radio_value(
                        &mut rule.content_type,
                        ContentType::Regex,
                        "Регулярное выражение",
                    );
                    ui.radio_value(&mut rule.content_type, ContentType::IntRanges, "Целые числа");
                });

                let text_edit = egui::TextEdit::singleline(&mut rule.text).code_editor();

                ui.add(text_edit);
            }
        });
    }

    fn display_rule_list(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("Навигация");

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if ui.button("Вверх").clicked() {
                        self.shift_cursor_up();
                    }
                    if ui.button("Вниз").clicked() {
                        self.shift_cursor_down();
                    }
                });

                ui.vertical(|ui| {
                    let current = self.cursor;

                    if ui.button("Сдвинуть вверх").clicked() {
                        self.shift_cursor_up();
                        self.rules.swap(current, self.cursor);
                    }

                    if ui.button("Сдвинуть вниз").clicked() {
                        self.shift_cursor_down();
                        self.rules.swap(current, self.cursor);
                    }
                });
            });

            ui.separator();

            for (i, rule) in self.rules.iter().enumerate() {
                if i == self.cursor {
                    ui.label(format!("> {} ({})", &rule.name, &rule.rule_type));
                } else {
                    ui.label(format!("- {} ({})", &rule.name, &rule.rule_type));
                }
            }
        });
    }
}

#[derive(Debug, Default)]
pub struct SharedRunnerState {
    pub solved_tests: AtomicU32,
    pub required_tests: AtomicU32,
}

impl SharedRunnerState {
    #[inline]
    pub fn reset(&self) {
        self.solved_tests.store(0, Ordering::Release);
        self.required_tests.store(0, Ordering::Release);
    }
}

#[derive(Debug)]
struct RunManager {
    work_state: Arc<SharedRunnerState>,
    work_sender: SyncSender<TestingData>,
    result_receiver: Receiver<TestReport>,
    last_report: Option<TestReport>,
}

impl RunManager {
    fn create_and_start_thread() -> Self {
        let work_state = Arc::new(SharedRunnerState::default());
        let (work_sender, work_receiver) = mpsc::sync_channel::<TestingData>(0);
        let (result_sender, result_receiver) = mpsc::sync_channel::<TestReport>(0);

        Runner::new(work_state.clone(), work_receiver, result_sender).start();

        Self {
            work_state,
            work_sender,
            result_receiver,
            last_report: None,
        }
    }

    fn restart_thread(&mut self) {
        self.work_state.reset();
        self.last_report = None;

        let (work_sender, work_receiver) = mpsc::sync_channel::<TestingData>(0);
        let (result_sender, result_receiver) = mpsc::sync_channel::<TestReport>(0);

        Runner::new(self.work_state.clone(), work_receiver, result_sender).start();

        self.work_sender = work_sender;
        self.result_receiver = result_receiver;
    }

    #[inline]
    fn force_stop_thread(&mut self) {
        self.restart_thread();
        self.last_report = None;
    }

    #[inline]
    fn send_testing_data(&mut self, data: TestingData) -> bool {
        if self.work_sender.send(data).is_err() {
            self.restart_thread();
            false
        } else {
            true
        }
    }

    fn try_receive_result(&mut self) -> Option<bool> {
        match self.result_receiver.try_recv() {
            Ok(result) => {
                self.last_report = Some(result);
                Some(true)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // worker thread died (panic happened)
                eprintln!("worker thread died -- restarting");
                self.restart_thread();

                None
            }
            Err(mpsc::TryRecvError::Empty) => Some(false),
        }
    }
}

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
            rules: self.ui_rule_panel.rules.clone(),
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
                AppState::Idle => {
                    self.ui_start_button(ui);
                }
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
