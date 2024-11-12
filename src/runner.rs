use crate::{
    communicator::Communicator,
    gui::{SharedWorkState, TestingData},
};
use anyhow::Result;
use bstr::BString;
use regex::bytes::Regex;
use regex_syntax::hir::Hir;
use std::{
    process::Command,
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc,
    },
};

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

            if result_sender.send(result).is_err() {
                // result channel disconnected => main thread died
                break;
            }
        }

        // work channel disconnected => main thread died
    }
}

fn run(testing_data: TestingData, work_state: &Arc<SharedWorkState>) -> RunResult {
    todo!()
}

#[derive(Debug)]
struct SingleRunData<'a> {
    pub command: &'a mut Command,
    pub operations: &'a Vec<Operation>,
}

impl<'a> SingleRunData<'a> {
    fn run(self) -> Result<RunResult> {
        let mut comm = Communicator::new(self.command)?;

        for op in self.operations.iter() {
            if !op.exec(&mut comm)? {
                // TODO: we failed and now we reduce and do again right now
                return Ok(RunResult::Failure {
                    history: comm.history,
                });
            }
        }

        Ok(RunResult::Success)
    }
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

#[derive(Clone, Debug)]
pub enum Rules {
    Empty,
    Plain(Arc<String>),
    Regex(Arc<Hir>),
}

impl Rules {
    #[inline]
    pub fn generate(&self) -> Arc<BString> {
        match self {
            Self::Empty => panic!("maybe later"),
            Self::Plain(text) => Arc::new(BString::from(text.as_bytes())),
            Self::Regex(hir) => {
                todo!("call to generator")
            }
        }
    }
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
            Self::Empty => panic!("maybe later"),
            Self::Plain(correct) => text == correct.as_bytes(),
            Self::Regex(regex) => regex.is_match(text.as_slice()),
        }
    }
}

#[derive(Debug)]
pub enum RunResult {
    Success,
    Failure { history: Vec<Arc<BString>> }, // TODO: add communicator's history
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
