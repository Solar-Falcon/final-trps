use anyhow::{Error, Result};
use bstr::BString;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command},
    sync::Arc,
};

pub struct Communicator {
    process: Child,
    pub history: Vec<Arc<BString>>,
}

impl Communicator {
    #[inline]
    pub fn new(command: &mut Command) -> Result<Self> {
        Ok(Self {
            process: command.spawn()?,
            history: Vec::new(),
        })
    }

    pub fn read_line(&mut self) -> Result<Arc<BString>> {
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or(Error::msg("program stdout unavailable (TODO: better msg)"))?;

        let mut buffer = Vec::new();
        BufReader::new(stdout).read_until(b'\n', &mut buffer)?;

        let string = Arc::new(BString::new(buffer));
        self.history.push(string.clone());

        Ok(string)
    }

    pub fn write_line(&mut self, line: &Arc<BString>) -> Result<()> {
        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or(Error::msg("program stdin unavailable (TODO: better msg)"))?;

        stdin.write_all(line)?;

        self.history.push(line.clone());

        Ok(())
    }
}
