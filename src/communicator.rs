use anyhow::{Error, Result};
use bstr::{BString, ByteSlice};
use std::{
    fmt::Display,
    io::{BufRead, BufReader, Write},
    process::{Child, Command},
    sync::Arc,
};

#[derive(Clone, Debug)]
enum Item {
    Stdin(Arc<BString>),
    Stdout(Arc<BString>),
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdin(inp) => write!(f, "> {}", inp),
            Self::Stdout(out) => write!(f, "< {}", out),
        }
    }
}

#[derive(Clone, Debug)]
pub struct History {
    vec: Vec<Item>,
}

impl Display for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.vec.iter() {
            writeln!(f, "{}", item)?;
        }

        Ok(())
    }
}

pub struct Communicator {
    process: Child,
    pub history: History,
}

impl Communicator {
    #[inline]
    pub fn new(command: &mut Command) -> Result<Self> {
        Ok(Self {
            process: command.spawn()?,
            history: History { vec: Vec::new() },
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
        self.history.vec.push(Item::Stdout(string.clone()));

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

        self.history.vec.push(Item::Stdin(line.clone()));

        Ok(())
    }
}
