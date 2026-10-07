use std::io::{self, Write};

use calc_i18n::Localized;

pub struct Output<'a> {
    writer: &'a mut dyn Write,
}

impl<'a> Output<'a> {
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self { writer }
    }

    pub fn line(&mut self, text: &Localized) -> io::Result<()> {
        writeln!(self.writer, "{text}")
    }

    pub fn json(&mut self, canonical_json: &[u8]) -> io::Result<()> {
        self.writer.write_all(canonical_json)
    }

    pub fn json_record(&mut self, one_line_json: &[u8]) -> io::Result<()> {
        self.writer.write_all(one_line_json)?;
        self.writer.write_all(b"\n")
    }
}
