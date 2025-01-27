use crate::{
    communicator::{CommReport, Communicator, History},
    gui::{ContentType, RuleData, RuleType},
    rules::{IntRanges, PlainText, RegExpr, Rule},
    run_manager::SharedRunnerState,
    DATE_FORMAT,
};
use std::{
    fmt::Debug,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::Ordering,
        mpsc::{Receiver, SyncSender},
        Arc,
    },
    thread,
};

#[derive(Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub rules: Vec<RuleData>,
    pub successes_required: u32,
}

#[derive(Debug)]
pub struct Runner {
    work_state: Arc<SharedRunnerState>,
    work_receiver: Receiver<TestingData>,
    result_sender: SyncSender<TestReport>,
}

impl Runner {
    #[inline]
    pub fn new(
        work_state: Arc<SharedRunnerState>,
        work_receiver: Receiver<TestingData>,
        result_sender: SyncSender<TestReport>,
    ) -> Self {
        Self {
            work_state,
            work_receiver,
            result_sender,
        }
    }

    pub fn start(mut self) {
        thread::spawn(move || {
            while let Ok(testing_data) = self.work_receiver.recv() {
                let result = self.run_tests(testing_data);

                if self.result_sender.send(result.into()).is_err() {
                    // result channel disconnected => main thread died
                    break;
                }

                self.work_state.reset();
            }

            // work channel disconnected => main thread died
        });
    }

    fn run_tests(&mut self, testing_data: TestingData) -> anyhow::Result<TestReport> {
        let mut command = Command::new(testing_data.program_path);
        command.stdin(Stdio::piped()).stdout(Stdio::piped());

        let ops = Operation::process(&testing_data.rules)?;

        self.work_state
            .required_tests
            .store(testing_data.successes_required, Ordering::Release);

        let mut success_histories = Vec::new();

        while self.work_state.solved_tests.fetch_add(1, Ordering::AcqRel)
            < self.work_state.required_tests.load(Ordering::Acquire)
        {
            let result = self.run_single(&mut command, &ops, &mut success_histories)?;

            if !matches!(result, TestReport::Success) {
                return Ok(result);
            }
        }

        save_to_file(
            "Успехи",
            &success_histories.join("\n#====================#\n"),
        );

        Ok(TestReport::Success)
    }

    fn run_single(
        &mut self,
        command: &mut Command,
        operations: &[Operation],
        success_histories: &mut Vec<String>,
    ) -> anyhow::Result<TestReport> {
        let mut comm = Communicator::new(command)?;

        for op in operations.iter() {
            match op.exec(&mut comm)? {
                OpReport::Success => {}
                OpReport::Failure { error_message } => {
                    save_to_file("Ошибки", &format!("{}\n{}", &comm.history, &error_message));

                    return Ok(TestReport::Failure {
                        history: comm.history,
                        error_message,
                    });
                }
            }
        }

        let report = comm.finish()?;

        match report {
            CommReport::Success(history) => {
                success_histories.push(history.to_string());
                Ok(TestReport::Success)
            }
            CommReport::NonEmptyStdout(history) => {
                let error_message = "Программа вывела лишние данные";

                save_to_file("Ошибки", &format!("{}\n{}", &history, &error_message));
                Ok(TestReport::Failure {
                    history,
                    error_message: error_message.to_string(),
                })
            }
            CommReport::ProgramError(history, stderr) => {
                let error_message = format!("Программа не была успешно завершена:\n{}", stderr);

                Ok(TestReport::Failure {
                    history,
                    error_message,
                })
            }
        }
    }
}

fn save_to_file(file_prefix: &str, contents: &str) {
    let date = time::OffsetDateTime::now_utc();

    let file_name = format!("{} {}.txt", file_prefix, date.format(&DATE_FORMAT).unwrap());

    if fs::write(file_name, contents).is_err() {
        eprintln!("Не удалось сохранить данные в файл!");
    }
}

impl RuleData {
    fn to_rule(&self) -> anyhow::Result<Box<dyn Rule>> {
        match self.content_type {
            ContentType::PlainText => PlainText::parse(&self.text).map(|rule| {
                let boxed: Box<dyn Rule> = Box::new(rule);

                boxed
            }),
            ContentType::Regex => RegExpr::parse(&self.text).map(|rule| {
                let boxed: Box<dyn Rule> = Box::new(rule);

                boxed
            }),
            ContentType::IntRanges => IntRanges::parse(&self.text).map(|rule| {
                let boxed: Box<dyn Rule> = Box::new(rule);

                boxed
            }),
        }
    }
}

#[derive(Debug)]
pub enum Operation {
    Output(Box<dyn Rule>),
    Input(Box<dyn Rule>),
}

impl Operation {
    #[inline]
    fn process(rules: &[RuleData]) -> anyhow::Result<Vec<Self>> {
        rules
            .iter()
            .map(|rule| {
                Ok(match rule.rule_type {
                    RuleType::Input => Self::Input(rule.to_rule()?),
                    RuleType::Output => Self::Output(rule.to_rule()?),
                })
            })
            .collect()
    }

    fn exec(&self, comm: &mut Communicator) -> anyhow::Result<OpReport> {
        match self {
            Self::Input(rule) => {
                let string = rule.generate();

                comm.write_line(string)?;

                Ok(OpReport::Success)
            }
            Self::Output(rule) => {
                let text = comm.read_line()?;

                Ok(rule.validate(&text))
            }
        }
    }
}

#[derive(Debug)]
pub enum OpReport {
    Success,
    Failure { error_message: String },
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
