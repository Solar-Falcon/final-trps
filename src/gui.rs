use crate::runner::{ArgType, Argument, ContentType, Runner, TestReport, TestingData};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppState {
    Idle,
    Working,
    Finished,
}

#[derive(Debug, Default)]
struct UiFileSelect {
    program_file: Option<PathBuf>,
    file_dialog: FileDialog,
}

impl UiFileSelect {
    fn display(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if ui.button("Выбрать тестируемый исполняемый файл").clicked()
        {
            self.file_dialog.select_file();
        }

        self.file_dialog.update(ctx);

        if let Some(path) = self.file_dialog.take_selected() {
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
struct UiArgumentPanel {
    cursor: usize,
    args: Vec<Argument>,
}

impl UiArgumentPanel {
    #[inline]
    fn shift_cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    #[inline]
    fn shift_cursor_down(&mut self) {
        self.cursor = self
            .cursor
            .saturating_add(1)
            .min(self.args.len().saturating_sub(1));
    }

    fn display(&mut self, ui: &mut egui::Ui) {
        ui.label("Входные/выходные параметры программы");

        self.display_arg_creation(ui);

        ui.separator();

        ui.horizontal(|ui| {
            self.display_main_panel(ui);

            ui.separator();

            self.display_argument_list(ui);
        });
    }

    fn display_arg_creation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Добавить в конец списка").clicked() {
                self.args.push(Default::default());
                self.cursor = self.args.len() - 1; // cursor on the new argument
            }

            if !self.args.is_empty() {
                if ui.button("Добавить после текущего").clicked() {
                    self.cursor += 1;
                    self.args.insert(self.cursor, Default::default());
                }

                if ui.button("Добавить перед текущим").clicked() {
                    self.args.insert(self.cursor, Default::default());
                }
            }
        });

        if ui.button("Удалить текущий").clicked() {
            self.args.remove(self.cursor);
            self.shift_cursor_up(); // when we remove the last arg, cursor points to nothing
        }
    }

    fn display_main_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::ComboBox::from_label("Выбор параметра").show_index(
                ui,
                &mut self.cursor,
                self.args.len(),
                |i| {
                    self.args
                        .get(i)
                        .map(|arg| arg.name.as_str())
                        .unwrap_or("Ничего нет!")
                },
            );

            if self.cursor < self.args.len() {
                let arg = &mut self.args[self.cursor];

                ui.horizontal(|ui| {
                    ui.label("Название: ");
                    ui.text_edit_singleline(&mut arg.name);
                });

                ui.horizontal(|ui| {
                    ui.label("Тип параметра: ");
                    ui.radio_value(&mut arg.arg_type, ArgType::Input, "Входной");
                    ui.radio_value(&mut arg.arg_type, ArgType::Output, "Выходной");
                });

                ui.horizontal(|ui| {
                    ui.label("Тип содержимого: ");
                    ui.radio_value(&mut arg.content_type, ContentType::PlainText, "Текст");
                    ui.radio_value(
                        &mut arg.content_type,
                        ContentType::Regex,
                        "Регулярное выражение",
                    );
                    ui.radio_value(&mut arg.content_type, ContentType::IntRanges, "Целые числа");
                });

                let text_edit = egui::TextEdit::multiline(&mut arg.text).code_editor();

                ui.add(text_edit);
            }
        });
    }

    fn display_argument_list(&mut self, ui: &mut egui::Ui) {
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
                        self.args.swap(current, self.cursor);
                    }

                    if ui.button("Сдвинуть вниз").clicked() {
                        self.shift_cursor_down();
                        self.args.swap(current, self.cursor);
                    }
                });
            });

            ui.separator();

            for (i, arg) in self.args.iter().enumerate() {
                if i == self.cursor {
                    ui.label(format!("> {} ({})", &arg.name, &arg.arg_type));
                } else {
                    ui.label(format!("- {} ({})", &arg.name, &arg.arg_type));
                }
            }
        });
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

    fn try_receive_result(&mut self) -> bool {
        match self.result_receiver.try_recv() {
            Ok(result) => {
                self.last_report = Some(result);
                true
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // worker thread died (panic happened)
                eprintln!("worker thread died -- restarting");
                self.restart_thread();

                false
            }
            Err(mpsc::TryRecvError::Empty) => false,
        }
    }
}

#[derive(Debug)]
pub struct AppGui {
    run_manager: RunManager,
    successes_required: u32,
    state: AppState,

    ui_file_select: UiFileSelect,
    ui_arg_panel: UiArgumentPanel,
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
            ui_arg_panel: Default::default(),
        })
    }

    #[inline]
    fn collect_testing_data(&self) -> TestingData {
        TestingData {
            program_path: self.ui_file_select.program_file.as_ref().unwrap().clone(),
            args: self.ui_arg_panel.args.clone(),
            successes_required: self.successes_required,
        }
    }

    fn ui_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.ui_file_select.display(ctx, ui);

        if self.ui_file_select.is_file_selected() {
            ui.separator();

            self.ui_arg_panel.display(ui);

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

        if self.run_manager.try_receive_result() {
            self.state = AppState::Finished;
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
