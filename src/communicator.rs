use anyhow::{Error, Result};
use bstr::BString;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command},
};

pub struct Communicator {
    process: Child,
}

impl Communicator {
    #[inline]
    pub fn new(command: &mut Command) -> Result<Self> {
        Ok(Self {
            process: command.spawn()?,
        })
    }

    #[inline]
    pub fn read_line(&mut self) -> Result<BString> {
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or(Error::msg("program stdout unavailable"))?;

        let mut buffer = Vec::new();
        BufReader::new(stdout).read_until(b'\n', &mut buffer)?;

        Ok(BString::new(buffer))
    }

    #[inline]
    pub fn write_line(&mut self, line: &[u8]) -> Result<()> {
        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or(Error::msg("program stdin unavailable"))?;

        stdin.write_all(line)?;

        Ok(())
    }
}
