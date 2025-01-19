use crate::{
    communicator::{Communicator, History},
    gui::SharedRunnerState,
    rules::{IntRanges, PlainText, RegExpr},
    DATE_FORMAT,
};
use bstr::BString;
use std::{
    fmt::{Debug, Display},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ArgType {
    #[default]
    Input,
    Output,
}

impl Display for ArgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "входное"),
            Self::Output => write!(f, "выходное"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ContentType {
    #[default]
    PlainText,
    Regex,
    IntRanges,
}

#[derive(Clone, Debug, Default)]
pub struct Argument {
    pub name: String,
    pub arg_type: ArgType,
    pub content_type: ContentType,
    pub text: String,
}

impl Argument {
    fn to_rule(&self) -> anyhow::Result<Box<dyn Rule>> {
        match self.content_type {
            ContentType::PlainText => PlainText::parse(&self.text).map(|arg| {
                let boxed: Box<dyn Rule> = Box::new(arg);

                boxed
            }),
            ContentType::Regex => RegExpr::parse(&self.text).map(|arg| {
                let boxed: Box<dyn Rule> = Box::new(arg);

                boxed
            }),
            ContentType::IntRanges => IntRanges::parse(&self.text).map(|arg| {
                let boxed: Box<dyn Rule> = Box::new(arg);

                boxed
            }),
        }
    }
}

#[derive(Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub args: Vec<Argument>,
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

        let ops = Operation::process(&testing_data.args)?;

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

        success_histories.push(comm.history.to_string());

        Ok(TestReport::Success)
    }
}

fn save_to_file(file_prefix: &str, contents: &str) {
    let date = time::OffsetDateTime::now_utc();

    let file_name = format!("{} {}.txt", file_prefix, date.format(&DATE_FORMAT).unwrap());

    if fs::write(file_name, contents).is_err() {
        eprintln!("Не удалось сохранить данные в файл!");
    }
}

pub trait Rule: Debug {
    fn parse(text: &str) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn validate(&self, text: &BString) -> OpReport;
    fn generate(&self) -> BString;
}

#[derive(Debug)]
pub enum Operation {
    Output(Box<dyn Rule>),
    Input(Box<dyn Rule>),
}

impl Operation {
    #[inline]
    fn process(args: &[Argument]) -> anyhow::Result<Vec<Self>> {
        args.iter()
            .map(|arg| {
                Ok(match arg.arg_type {
                    ArgType::Input => Self::Input(arg.to_rule()?),
                    ArgType::Output => Self::Output(arg.to_rule()?),
                })
            })
            .collect()
    }

    fn exec(&self, comm: &mut Communicator) -> anyhow::Result<OpReport> {
        match self {
            Self::Input(arg) => {
                let string = arg.generate();

                comm.write_line(string)?;

                Ok(OpReport::Success)
            }
            Self::Output(arg) => {
                let text = comm.read_line()?;

                Ok(arg.validate(&text))
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
