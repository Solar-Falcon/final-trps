use anyhow::Result;
use eframe::{egui, App};
use std::{
    mem::swap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub args: Vec<Argument>,
    pub use_prev_errors: bool,
    pub successes_required: u32,
    pub work_state: Arc<SharedWorkState>,
}

#[derive(Debug, Default)]
pub struct SharedWorkState {
    solved_tests: AtomicU32,
    required_tests: AtomicU32,
    stop_requested: AtomicBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ArgType {
    #[default]
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ContentType {
    #[default]
    Empty,
    Plain,
    Regex,
}

#[derive(Clone, Debug, Default)]
pub struct Argument {
    pub name: String,
    pub arg_type: ArgType,
    pub content_type: ContentType,
    pub text: String,
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

pub struct AppGui {
    work_state: Arc<SharedWorkState>,
    work_sender: SyncSender<TestingData>,
    state: AppState,
    program: Option<PathBuf>,

    ui: UiData,
}

impl AppGui {
    pub fn new(cc: &eframe::CreationContext) -> Result<Self> {
        let work_state = Arc::new(SharedWorkState::default());
        let (work_sender, work_receiver) = mpsc::sync_channel::<TestingData>(0);

        thread::spawn(move || {
            while let Ok(work) = work_receiver.recv() {
                println!("'send' work to runner mod\nwork: {:?}", &work);

                println!(
                    "program output: {:?}",
                    std::process::Command::new(work.program_path)
                        .arg("Hello world")
                        .output()
                        .map(|bytes| String::from_utf8_lossy(&bytes.stdout).into_owned())
                );

                eprintln!("TODO!!!");
            }

            // if recv() returns Err(), the channel is disconnected => main thread has finished
        });

        // default is too small on linux
        cc.egui_ctx.set_zoom_factor(1.75);

        Ok(Self {
            work_state,
            work_sender,
            program: None,
            state: AppState::Idle,

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
            work_state: self.work_state.clone(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
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
                .logarithmic(false)
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

        if ui.button("Добавить аргумент в конец").clicked() {
            self.ui.args.push(Default::default());
            self.ui.arg_cursor = self.ui.args.len() - 1; // cursor on the new argument
        }

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

        let args_len = self.ui.args.len();
        if let Some(arg) = self.ui.args.get_mut(self.ui.arg_cursor) {
            if ui.button("Вверх").clicked() {
                self.ui.arg_cursor = self.ui.arg_cursor.saturating_sub(1);
            }
            if ui.button("Вниз").clicked() {
                self.ui.arg_cursor = self.ui.arg_cursor.saturating_add(1).min(args_len.saturating_sub(1));
            }

            ui.label("Название аргумента:");
            ui.text_edit_singleline(&mut arg.name);

            ui.label("Тип аргумента:");
            ui.radio_value(&mut arg.arg_type, ArgType::Input, "Входной");
            ui.radio_value(&mut arg.arg_type, ArgType::Output, "Выходной");

            ui.label("Тип содержания:");
            ui.radio_value(&mut arg.content_type, ContentType::Empty, "Пустой");
            ui.radio_value(&mut arg.content_type, ContentType::Plain, "Текст");
            ui.radio_value(
                &mut arg.content_type,
                ContentType::Regex,
                "Регулярное выражение",
            );

            match arg.content_type {
                ContentType::Empty => {}
                ContentType::Plain | ContentType::Regex => {
                    ui.text_edit_multiline(&mut arg.text);
                }
            }

            // add after/before
            // shift up/down
        }
    }

    #[inline]
    fn ui_start_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Начать тестирование").clicked() {
            let testing_data = self.collect_testing_data();
            self.work_sender
                .send(testing_data)
                .expect("TODO: fatal err handling (worker thread died) -- add a new AppState");

            thread::sleep(Duration::from_secs_f32(0.3));

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
    }

    fn ui_footer_finished(&mut self, ui: &mut egui::Ui) {
        self.ui_start_button(ui);
    }
}

impl App for AppGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // TODO: side panel with help?

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.ui(ui);
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
