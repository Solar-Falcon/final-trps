use crate::{communicator::History, gui::RuleData, worker_thread::Runner};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
};

#[derive(Debug)]
pub struct RunManager {
    pub work_state: Arc<SharedRunnerState>,
    work_sender: SyncSender<TestingData>,
    result_receiver: Receiver<TestReport>,
    pub last_report: Option<TestReport>,
}

impl RunManager {
    pub fn create_and_start_thread() -> Self {
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
    pub fn force_stop_thread(&mut self) {
        self.restart_thread();
        self.last_report = None;
    }

    #[inline]
    pub fn send_testing_data(&mut self, data: TestingData) -> bool {
        if self.work_sender.send(data).is_err() {
            self.restart_thread();
            false
        } else {
            true
        }
    }

    pub fn try_receive_result(&mut self) -> Option<bool> {
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
pub struct TestingData {
    pub program_path: PathBuf,
    pub rules: Vec<RuleData>,
    pub successes_required: u32,
}

#[derive(Debug)]
pub enum TestReport {
    Success,
    Failure {
        history: History,
        error_message: String,
    },
    Error(anyhow::Error),
}

impl From<anyhow::Result<Self>> for TestReport {
    #[inline]
    fn from(value: anyhow::Result<Self>) -> Self {
        match value {
            Ok(this) => this,
            Err(error) => Self::Error(error),
        }
    }
}
