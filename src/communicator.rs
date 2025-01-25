use anyhow::{Error, Result};
use bstr::{BString, ByteSlice};
use std::{
    fmt::Display,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command},
};

#[derive(Clone, Debug)]
enum Item {
    Stdin(BString),
    Stdout(BString),
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
    items: Vec<Item>,
}

impl Display for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.items.iter() {
            writeln!(f, "{}", item)?;
        }

        Ok(())
    }
}

pub struct Communicator {
    process: Child,
    reader: BufReader<ChildStdout>,
    writer: ChildStdin,
    pub history: History,
}

impl Communicator {
    #[inline]
    pub fn new(command: &mut Command) -> Result<Self> {
        let mut process = command.spawn()?;

        Ok(Self {
            reader: BufReader::new(
                process
                    .stdout
                    .take()
                    .ok_or(Error::msg("program stdout unavailable"))?,
            ),
            writer: process
                .stdin
                .take()
                .ok_or(Error::msg("program stdin unavailable"))?,
            process,
            history: History { items: Vec::new() },
        })
    }

    pub fn read_line(&mut self) -> Result<BString> {
        let mut buffer = Vec::new();
        self.reader.read_until(b'\n', &mut buffer)?;

        let string = BString::from(buffer.as_bstr().trim_end());
        self.history.items.push(Item::Stdout(string.clone()));

        Ok(string)
    }

    pub fn write_line(&mut self, mut line: BString) -> Result<()> {
        line.push(b'\n');
        self.writer.write_all(&line)?;

        self.history.items.push(Item::Stdin(line));

        Ok(())
    }

    pub fn finish(mut self) -> Result<CommReport> {
        let output = self.process.wait_with_output()?;

        let stdout_empty;
        if !output.stdout.is_empty() {
            stdout_empty = false;
            self.history
                .items
                .push(Item::Stdout(BString::new(output.stdout)));
        } else {
            stdout_empty = true;
        }

        if output.status.success() {
            if stdout_empty {
                Ok(CommReport::Success(self.history))
            } else {
                Ok(CommReport::NonEmptyStdout(self.history))
            }
        } else {
            let stderr = BString::new(output.stderr);
            Ok(CommReport::ProgramError(self.history, stderr))
        }
    }
}

#[derive(Debug)]
pub enum CommReport {
    Success(History),
    NonEmptyStdout(History),
    ProgramError(History, BString),
}
