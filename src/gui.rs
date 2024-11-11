use anyhow::Result;
use eframe::{egui, App};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

enum TESTING_DATA_STUB {}

#[derive(Debug, Default)]
struct SharedState {
    progress: AtomicU32,
    stop_requested: AtomicBool,
}

#[derive(Debug)]
enum State {
    Idle,
    Working,
    Finished,
}

#[derive(Debug, Default)]
struct UiData {
    use_prev_errors: bool,
}

pub struct AppGui {
    work_state: Arc<SharedState>,
    work_sender: SyncSender<TESTING_DATA_STUB>,
    state: State,
    program: Option<PathBuf>,

    ui: UiData,
}

impl AppGui {
    pub fn new(_cc: &eframe::CreationContext) -> Result<Self> {
        let work_state = Arc::new(SharedState::default());
        let (work_sender, work_receiver) = mpsc::sync_channel(0);

        thread::spawn(move || loop {
            match work_receiver.recv() {
                Ok(work) => {
                    todo!("do stuff")
                }
                Err(_) => {
                    // main thread finished
                    break;
                }
            }
        });

        Ok(Self {
            work_state,
            work_sender,
            program: None,
            state: State::Idle,

            ui: UiData::default(),
        })
    }

    fn start_testing_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Начать тестирование").clicked() {
            todo!("send");

            thread::sleep(Duration::from_secs_f32(0.3));
            self.state = State::Working;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::warn_if_debug_build(ui);

        ui.label("\nПеренесите исполняемый файл программы сюда!\n")
            .highlight();

        if let Some(prog_path) = self.program.as_ref() {
            ui.label(format!("Выбран файл: {}", prog_path.display()));

            ui.separator();

            ui.separator();

            ui.checkbox(
                &mut self.ui.use_prev_errors,
                "Использовать предыдущие ошибки",
            );

            match self.state {
                State::Idle => {
                    self.start_testing_button(ui);
                }
                State::Working => {
                    if ui.button("Остановить").clicked() {
                        self.work_state.stop_requested.store(true, Ordering::SeqCst);

                        thread::sleep(Duration::from_secs_f32(0.5));
                        self.state = State::Finished;
                    }

                    let prog_bar = egui::ProgressBar::new(0.0)
                        .show_percentage()
                        .text("Прогресс")
                        .desired_width(20.0);
                    ui.add(prog_bar);
                }
                State::Finished => {
                    // TODO: display results

                    self.start_testing_button(ui);
                }
            }
        }
    }
}

impl App for AppGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // TODO: side panel with help?

        let _ = egui::CentralPanel::default().show(ctx, |ui| {
            self.ui(ui);
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
