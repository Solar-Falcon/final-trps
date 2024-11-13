use crate::runner::{self, ArgType, Argument, ContentType, RunResult, TestingData};
use anyhow::Result;
use eframe::{
    egui::{self, Color32},
    App,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

#[derive(Debug, Default)]
pub struct SharedWorkState {
    pub solved_tests: AtomicU32,
    pub required_tests: AtomicU32,
    pub stop_requested: AtomicBool,
}

impl SharedWorkState {
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
    program_path_input: String,
    use_prev_errors: bool,
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
    program: Option<PathBuf>,
    last_result: Option<RunResult>,

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

        // default is too small on linux
        cc.egui_ctx.set_zoom_factor(2.0);

        Ok(Self {
            work_state,
            work_sender,
            result_receiver,
            program: None,
            state: AppState::Idle,
            last_result: None,

            ui: UiData {
                program_path_input: String::new(),
                use_prev_errors: false,
                successes_required: 1,
                arg_cursor: 0,
                args: vec![],
            },
        })
    }

    #[inline]
    fn collect_testing_data(&self) -> TestingData {
        TestingData {
            program_path: self.program.as_ref().unwrap().clone(),
            args: self.ui.args.clone(),
            use_prev_errors: self.ui.use_prev_errors,
            successes_required: self.ui.successes_required,
        }
    }

    fn ui_main(&mut self, ui: &mut egui::Ui) {
        egui::warn_if_debug_build(ui);

        ui.label(
            "Перенесите исполняемый файл программы сюда!\nИли введите путь к нему в поле ниже",
        )
        .highlight();

        if ui
            .text_edit_singleline(&mut self.ui.program_path_input)
            .changed()
        {
            self.program = Some(PathBuf::from(&self.ui.program_path_input));
        }

        if let Some(prog_path) = self.program.as_ref() {
            ui.separator();
            ui.label(format!("Выбран файл: {}", prog_path.display()));

            ui.separator();
            self.ui_argument_list(ui);

            ui.separator();
            ui.checkbox(
                &mut self.ui.use_prev_errors,
                "Использовать предыдущие ошибки",
            );

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
        ui.label("Аргументы программы");

        ui.horizontal(|ui| {
            if ui.button("Добавить в конец").clicked() {
                self.ui.args.push(Default::default());
                self.ui.arg_cursor = self.ui.args.len() - 1; // cursor on the new argument
            }

            if ui.button("Добавить после текущего").clicked() {
                self.ui.arg_cursor += 1;
                self.ui.args.insert(self.ui.arg_cursor, Default::default());
            }

            if ui.button("Добавить перед текущим").clicked() {
                self.ui.args.insert(self.ui.arg_cursor, Default::default());
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Выбрать аргумент программы").show_index(
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

            if ui.button("Вверх").clicked() {
                self.ui.shift_cursor_up();
            }
            if ui.button("Вниз").clicked() {
                self.ui.shift_cursor_down();
            }
        });

        if self.ui.arg_cursor < self.ui.args.len() {
            let arg = &mut self.ui.args[self.ui.arg_cursor];

            ui.horizontal(|ui| {
                ui.label("Название аргумента: ");
                ui.text_edit_singleline(&mut arg.name);
            });

            ui.horizontal(|ui| {
                ui.label("Тип аргумента: ");
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
            });

            match arg.content_type {
                ContentType::Empty => {}
                ContentType::Plain | ContentType::Regex => {
                    ui.text_edit_multiline(&mut arg.text);
                }
            }

            ui.horizontal(|ui| {
                let current = self.ui.arg_cursor;

                if ui.button("Сдвинуть вниз").clicked() {
                    self.ui.shift_cursor_down();
                    self.ui.args.swap(current, self.ui.arg_cursor);
                }

                if ui.button("Сдвинуть вверх").clicked() {
                    self.ui.shift_cursor_up();
                    self.ui.args.swap(current, self.ui.arg_cursor);
                }

                if ui.button("Удалить").clicked() {
                    self.ui.args.remove(current);
                    self.ui.shift_cursor_up(); // when we remove the last arg, cursor points to nothing
                }
            });
        }
    }

    #[inline]
    fn ui_start_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Начать тестирование").clicked() {
            let testing_data = self.collect_testing_data();
            self.work_sender
                .send(testing_data)
                .expect("fatal error (worker thread died) -- TODO: restart thread");

            thread::sleep(Duration::from_secs_f32(0.35));

            self.state = AppState::Working;
        }
    }

    fn ui_footer_working(&mut self, ui: &mut egui::Ui) {
        if ui.button("Остановить").clicked() {
            self.work_state
                .stop_requested
                .store(true, Ordering::Release);

            thread::sleep(Duration::from_secs_f32(0.5));
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
                self.state = AppState::Idle; // < include in restart()
                todo!("restart worker thread");
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
            Some(RunResult::Failure { history }) => {
                ui.colored_label(Color32::DARK_RED, "Обнаружены ошибки:");

                ui.label("История ввода/вывода: ");

                println!("{:?}", history);
                for s in history.iter() {
                    ui.label(format!("{s}"));
                }
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
        // TODO: side panel with help

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                self.ui_main(ui);
            });
        });
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(path) = raw_input
            .dropped_files
            .first_mut()
            .and_then(|dropped_file| dropped_file.path.take())
        {
            self.program = Some(path);
        }
    }
}
