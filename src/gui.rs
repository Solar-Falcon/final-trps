use crate::runner::{self, ArgType, Argument, ContentType, RunResult, TestingData};
use anyhow::Result;
use eframe::{
    egui::{self, Color32},
    App,
};
use egui_file_dialog::FileDialog;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
};

#[derive(Debug, Default)]
pub struct SharedWorkState {
    pub solved_tests: AtomicU32,
    pub required_tests: AtomicU32,
    pub stop_requested: AtomicBool,
}

impl SharedWorkState {
    #[inline]
    pub fn reset(&self) {
        self.solved_tests.store(0, Ordering::Release);
        self.required_tests.store(0, Ordering::Release);
        self.stop_requested.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppState {
    Idle,
    Working,
    Finished,
}

#[derive(Debug)]
struct UiData {
    successes_required: u32,
    arg_cursor: usize,
    args: Vec<Argument>,
}

impl UiData {
    #[inline]
    fn shift_cursor_up(&mut self) {
        self.arg_cursor = self.arg_cursor.saturating_sub(1);
    }

    #[inline]
    fn shift_cursor_down(&mut self) {
        self.arg_cursor = self
            .arg_cursor
            .saturating_add(1)
            .min(self.args.len().saturating_sub(1));
    }
}

pub struct AppGui {
    work_state: Arc<SharedWorkState>,
    work_sender: SyncSender<TestingData>,
    result_receiver: Receiver<RunResult>,
    state: AppState,
    last_result: Option<RunResult>,

    program_file: Option<PathBuf>,
    file_dialog: FileDialog,

    ui: UiData,
}

impl AppGui {
    pub fn new(cc: &eframe::CreationContext) -> Result<Self> {
        let work_state = Arc::new(SharedWorkState::default());
        let (work_sender, work_receiver) = mpsc::sync_channel::<TestingData>(0);
        let (result_sender, result_receiver) = mpsc::sync_channel::<RunResult>(0);

        thread::spawn(runner::working_thread(
            work_state.clone(),
            work_receiver,
            result_sender,
        ));

        // default is too small (especially on linux)
        cc.egui_ctx.set_zoom_factor(1.75);

        Ok(Self {
            work_state,
            work_sender,
            result_receiver,
            state: AppState::Idle,
            last_result: None,

            program_file: None,
            file_dialog: FileDialog::new(),

            ui: UiData {
                successes_required: 1,
                arg_cursor: 0,
                args: vec![],
            },
        })
    }

    #[inline]
    fn collect_testing_data(&self) -> TestingData {
        TestingData {
            program_path: self.program_file.as_ref().unwrap().clone(),
            args: self.ui.args.clone(),
            successes_required: self.ui.successes_required,
        }
    }

    fn restart_worker_thread(&mut self) {
        self.state = AppState::Idle;
        self.work_state.reset();
        self.last_result = None;

        let (work_sender, work_receiver) = mpsc::sync_channel::<TestingData>(0);
        let (result_sender, result_receiver) = mpsc::sync_channel::<RunResult>(0);

        thread::spawn(runner::working_thread(
            self.work_state.clone(),
            work_receiver,
            result_sender,
        ));

        self.work_sender = work_sender;
        self.result_receiver = result_receiver;
    }

    fn ui_main(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::warn_if_debug_build(ui);

        if ui.button("Выбрать тестируемый исполняемый файл").clicked() {
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

            ui.separator();
            self.ui_argument_display(ui);

            ui.separator();

            let slider = egui::Slider::new(&mut self.ui.successes_required, 1..=u32::MAX)
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

    fn ui_argument_list(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("Навигация");

            if ui.button("Вверх").clicked() {
                self.ui.shift_cursor_up();
            }
            if ui.button("Вниз").clicked() {
                self.ui.shift_cursor_down();
            }

            ui.separator();

            for (i, arg) in self.ui.args.iter().enumerate() {
                if i == self.ui.arg_cursor {
                    ui.label(format!("> {} ({})", &arg.name, &arg.arg_type));
                } else {
                    ui.label(format!("- {} ({})", &arg.name, &arg.arg_type));
                }
            }
        });
    }

    fn ui_argument_display(&mut self, ui: &mut egui::Ui) {
        ui.label("Входные/выходные параметры программы");

        ui.horizontal(|ui| {
            if ui.button("Добавить в конец списка").clicked() {
                self.ui.args.push(Default::default());
                self.ui.arg_cursor = self.ui.args.len() - 1; // cursor on the new argument
            }

            if !self.ui.args.is_empty() {
                if ui.button("Добавить после текущего").clicked() {
                    self.ui.arg_cursor += 1;
                    self.ui.args.insert(self.ui.arg_cursor, Default::default());
                }

                if ui.button("Добавить перед текущим").clicked() {
                    self.ui.args.insert(self.ui.arg_cursor, Default::default());
                }
            }
        });

        let current = self.ui.arg_cursor;

        ui.horizontal(|ui| {
            if ui.button("Сдвинуть вниз").clicked() {
                self.ui.shift_cursor_down();
                self.ui.args.swap(current, self.ui.arg_cursor);
            }

            if ui.button("Сдвинуть вверх").clicked() {
                self.ui.shift_cursor_up();
                self.ui.args.swap(current, self.ui.arg_cursor);
            }
        });

        if ui.button("Удалить").clicked() {
            self.ui.args.remove(current);
            self.ui.shift_cursor_up(); // when we remove the last arg, cursor points to nothing
        }

        ui.separator();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                egui::ComboBox::from_label("Выбор").show_index(
                    ui,
                    &mut self.ui.arg_cursor,
                    self.ui.args.len(),
                    |i| {
                        self.ui
                            .args
                            .get(i)
                            .map(|arg| arg.name.as_str())
                            .unwrap_or("Ничего нет!")
                    },
                );

                if self.ui.arg_cursor < self.ui.args.len() {
                    let arg = &mut self.ui.args[self.ui.arg_cursor];

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
                        ui.label("Тип содержания: ");
                        ui.radio_value(&mut arg.content_type, ContentType::Empty, "Пустой");
                        ui.radio_value(&mut arg.content_type, ContentType::Plain, "Текст");
                        ui.radio_value(
                            &mut arg.content_type,
                            ContentType::Regex,
                            "Регулярное выражение",
                        );
                        ui.radio_value(&mut arg.content_type, ContentType::Int, "Целое число");
                    });

                    match arg.content_type {
                        ContentType::Empty => {}
                        ContentType::Plain | ContentType::Regex => {
                            ui.text_edit_multiline(&mut arg.text);
                        }
                        ContentType::Int => {
                            let min_slider = egui::Slider::new(&mut arg.min, i64::MIN..=i64::MAX)
                                .text("Минимальное значение")
                                .logarithmic(true)
                                .integer();

                            ui.add(min_slider);

                            let max_slider = egui::Slider::new(&mut arg.max, arg.min..=i64::MAX)
                                .text("Максимальное значение")
                                .logarithmic(true)
                                .integer();

                            ui.add(max_slider);
                        }
                    }
                }
            });

            ui.separator();

            self.ui_argument_list(ui);
        });
    }

    #[inline]
    fn ui_start_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Начать тестирование").clicked() {
            let testing_data = self.collect_testing_data();

            if self.work_sender.send(testing_data).is_err() {
                self.restart_worker_thread();
                return;
            }

            self.state = AppState::Working;
        }
    }

    fn ui_footer_working(&mut self, ui: &mut egui::Ui) {
        if ui.button("Остановить").clicked() {
            self.work_state
                .stop_requested
                .store(true, Ordering::Release);

            self.restart_worker_thread(); // in case it stopped responding

            self.last_result = None;

            self.state = AppState::Finished;
        }

        let tests_solved = self.work_state.solved_tests.load(Ordering::Acquire);
        let tests_required = self.work_state.required_tests.load(Ordering::Acquire);
        let progress = (tests_solved as f32) / (tests_required as f32);

        let progress_bar = egui::ProgressBar::new(progress)
            .show_percentage()
            .text(format!("Прогресс: {}/{}", tests_solved, tests_required));

        ui.add(progress_bar);

        match self.result_receiver.try_recv() {
            Ok(result) => {
                self.last_result = Some(result);
                self.state = AppState::Finished;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // worker thread died (panic happened)
                println!("worker thread died -- restarting");
                self.restart_worker_thread();
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn ui_footer_finished(&mut self, ui: &mut egui::Ui) {
        ui.separator();

        match self.last_result.as_mut() {
            Some(RunResult::Success) => {
                ui.colored_label(Color32::GREEN, "Все тесты прошли успешно");
            }
            Some(RunResult::Failure {
                history,
                failed_valid,
            }) => {
                ui.colored_label(Color32::DARK_RED, "Обнаружены ошибки:");

                ui.label("История ввода/вывода: ");
                ui.label(format!("{}", history));

                ui.label(format!("Ожидаемый вывод: {}", failed_valid));
            }
            Some(RunResult::Error(error)) => {
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

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(path) = raw_input
            .dropped_files
            .first_mut()
            .and_then(|dropped_file| dropped_file.path.take())
        {
            self.program_file = Some(path);
        }
    }
}
