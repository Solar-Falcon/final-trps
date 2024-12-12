use crate::{
    communicator::{Communicator, History},
    generator::Rules,
    gui::SharedWorkState,
    parser::parse_args,
    validator::Validation,
    DATE_FORMAT,
};
use anyhow::Result;
use std::{
    fmt::Display,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::Ordering,
        mpsc::{Receiver, SyncSender},
        Arc,
    },
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
            Self::Input => write!(f, "входной"),
            Self::Output => write!(f, "выходной"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ContentType {
    #[default]
    Empty,
    Plain,
    Regex,
    Int,
}

#[derive(Clone, Debug)]
pub struct Argument {
    pub name: String,
    pub arg_type: ArgType,
    pub content_type: ContentType,
    pub text: String,
    pub min: i64,
    pub max: i64,
}

impl Default for Argument {
    #[inline]
    fn default() -> Self {
        Self {
            name: String::default(),
            arg_type: ArgType::default(),
            content_type: ContentType::default(),
            text: String::default(),
            min: i64::MIN,
            max: i64::MAX,
        }
    }
}

#[derive(Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub args: Vec<Argument>,
    pub successes_required: u32,
}

#[inline]
pub fn working_thread(
    work_state: Arc<SharedWorkState>,
    work_receiver: Receiver<TestingData>,
    result_sender: SyncSender<RunResult>,
) -> impl FnOnce() + Send + 'static {
    move || {
        while let Ok(testing_data) = work_receiver.recv() {
            let result = run(testing_data, &work_state);

            if result_sender.send(result.into()).is_err() {
                // result channel disconnected => main thread died
                break;
            }

            work_state.reset();
        }

        // work channel disconnected => main thread died
    }
}

fn run(testing_data: TestingData, work_state: &Arc<SharedWorkState>) -> Result<RunResult> {
    let mut command = Command::new(testing_data.program_path);
    command.stdin(Stdio::piped()).stdout(Stdio::piped());

    let ops = parse_args(&testing_data.args)?;

    work_state
        .required_tests
        .store(testing_data.successes_required, Ordering::Release);

    while !work_state.stop_requested.load(Ordering::Acquire)
        && (work_state.solved_tests.fetch_add(1, Ordering::AcqRel)
            < work_state.required_tests.load(Ordering::Acquire))
    {
        let result = run_single(&mut command, &ops)?;

        if !matches!(result, RunResult::Success) {
            return Ok(result);
        }
    }

    Ok(RunResult::Success)
}

fn run_single(command: &mut Command, operations: &[Operation]) -> Result<RunResult> {
    let mut comm = Communicator::new(command)?;

    for op in operations.iter() {
        if !op.exec(&mut comm)? {
            let failed_valid = op.to_validation();

            save_history(&comm.history, &failed_valid);

            return Ok(RunResult::Failure {
                history: comm.history,
                failed_valid,
            });
        }
    }

    Ok(RunResult::Success)
}

fn save_history(history: &History, failed_valid: &Validation) {
    let date = time::OffsetDateTime::now_utc();

    let file_name = format!("{}.txt", date.format(&DATE_FORMAT).unwrap());
    let file_content = format!("{}\nОжидаемый вывод: {}", history, failed_valid);

    if fs::write(file_name, file_content).is_err() {
        eprintln!("Не удалось сохранить данные в файл!");
    }
}

#[derive(Clone, Debug)]
pub enum Operation {
    Output { validation: Validation },
    Input { rules: Rules },
}

impl Operation {
    fn to_validation(&self) -> Validation {
        match self {
            Self::Output { validation } => validation.clone(),
            _ => unreachable!(),
        }
    }

    fn exec(&self, comm: &mut Communicator) -> Result<bool> {
        match self {
            Self::Input { rules } => {
                let arg = rules.generate();

                comm.write_line(&arg)?;

                Ok(true)
            }
            Self::Output { validation } => {
                let text = comm.read_line()?;

                Ok(validation.validate(&text))
            }
        }
    }
}

#[derive(Debug)]
pub enum RunResult {
    Success,
    Failure {
        history: History,
        failed_valid: Validation,
    },
    Error(anyhow::Error),
}

impl From<Result<Self>> for RunResult {
    #[inline]
    fn from(value: Result<Self>) -> Self {
        match value {
            Ok(this) => this,
            Err(error) => Self::Error(error),
        }
    }
}
