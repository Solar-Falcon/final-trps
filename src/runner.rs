use crate::communicator::Communicator;
use anyhow::Result;
use bstr::BString;
use regex::bytes::Regex;
use regex_syntax::hir::Hir;
use std::{process::Command, rc::Rc};

#[derive(Debug)]
pub struct Runner {
    pub successes_required: u32,
    pub operations: Vec<Operation>,
    pub command: Command,
}

impl Runner {
    pub fn run_all(&mut self) {
        todo!()
    }
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
                return Ok(RunResult::Failure { ops: self.operations.clone() });
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

                Ok(validation.validate(text))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Rules {
    Plain(Rc<String>),
    Regex(Rc<Hir>),
}

impl Rules {
    #[inline]
    pub fn generate(&self) -> BString {
        match self {
            Self::Plain(text) => BString::from(text.as_bytes()),
            Self::Regex(hir) => {
                todo!("call to generator")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Validation {
    Plain(Rc<String>),
    Regex(Rc<Regex>),
}

impl Validation {
    #[inline]
    fn validate(&self, text: BString) -> bool {
        match self {
            Self::Plain(correct) => text == correct.as_bytes(),
            Self::Regex(regex) => regex.is_match(text.as_slice()),
        }
    }
}

#[derive(Debug)]
pub enum RunResult {
    Success,
    Failure {
        ops: Vec<Operation>,
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