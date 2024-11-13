use anyhow::{Error, Result};
use bstr::{BString, ByteSlice};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command},
    sync::Arc,
};

// TODO: dedicated history type (w/ input/output)

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
            .ok_or(Error::msg("program stdout unavailable"))?;

        let mut buffer = Vec::new();
        BufReader::new(stdout).read_until(b'\n', &mut buffer)?;

        let string = Arc::new(BString::from(buffer.as_bstr().trim_end()));
        self.history.push(string.clone());

        Ok(string)
    }

    pub fn write_line(&mut self, line: &Arc<BString>) -> Result<()> {
        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or(Error::msg("program stdin unavailable"))?;

        stdin.write_all(line)?;
        stdin.write_all(b"\n")?;

        self.history.push(line.clone());

        Ok(())
    }
}
