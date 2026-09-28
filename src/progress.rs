//! Explicitly flushed progress output for streaming commands.

use std::io::{self, Write};

pub(crate) struct Reporter<'a, W: Write> {
    out: &'a mut W,
}

impl<'a, W: Write> Reporter<'a, W> {
    pub(crate) fn new(out: &'a mut W) -> Self {
        Self { out }
    }

    pub(crate) fn line(&mut self, message: impl std::fmt::Display) -> io::Result<()> {
        writeln!(self.out, "{message}")?;
        self.out.flush()
    }

    pub(crate) fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
