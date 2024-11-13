use crate::{
    communicator::Communicator, converter::parse_args, generator::generate, gui::SharedWorkState,
};
use anyhow::Result;
use bstr::BString;
use regex::bytes::Regex;
use regex_syntax::hir::Hir;
use std::{
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

#[derive(Debug)]
pub struct TestingData {
    pub program_path: PathBuf,
    pub args: Vec<Argument>,
    pub use_prev_errors: bool,
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
            println!("work: {:?}", &testing_data);

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

    assert!(
        !testing_data.use_prev_errors,
        "prev errors are not yet supported"
    );
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
            // TODO: we failed and now we reduce and do again right now
            return Ok(RunResult::Failure {
                history: comm.history,
            });
        }
    }

    Ok(RunResult::Success)
}

#[derive(Clone, Debug)]
pub enum Operation {
    Output { validation: Validation },
    Input { rules: Rules },
}

impl Operation {
    fn exec(&self, comm: &mut Communicator) -> Result<bool> {
        match self {
            Self::Input { rules } => {
                let arg = generate(rules);

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

#[derive(Clone, Debug)]
pub enum Rules {
    Empty,
    Plain(Arc<String>),
    Regex(Arc<Hir>),
}

#[derive(Clone, Debug)]
pub enum Validation {
    Empty,
    Plain(Arc<String>),
    Regex(Arc<Regex>),
}

impl Validation {
    #[inline]
    fn validate(&self, text: &BString) -> bool {
        match self {
            Self::Empty => text.is_empty(),
            Self::Plain(correct) => text == correct.as_bytes(),
            Self::Regex(regex) => regex.is_match(text.as_slice()),
        }
    }
}

#[derive(Debug)]
pub enum RunResult {
    Success,
    Failure { history: Vec<Arc<BString>> },
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
