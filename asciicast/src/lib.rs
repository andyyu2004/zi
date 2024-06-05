//! Minimal library to generate `asciicast(v2)`.

use std::io::{self, Write};

#[derive(Debug, PartialEq, Eq)]
pub struct Asciicast {
    header: Header,
    events: Vec<Event>,
}

impl Asciicast {
    const VERSION: u8 = 2;

    pub fn new(width: u16, height: u16, events: impl IntoIterator<Item = Event>) -> Self {
        Asciicast {
            header: Header { version: Self::VERSION, width, height },
            events: events.into_iter().collect(),
        }
    }

    pub fn write_to(&self, mut w: impl Write) -> io::Result<()> {
        self.header.write_to(&mut w)?;
        for event in &self.events {
            event.write_to(&mut w)?;
        }
        Ok(())
    }

    pub fn read_from(mut r: impl io::BufRead) -> io::Result<Self> {
        let mut header = String::new();
        r.read_line(&mut header)?;
        let header: Header = serde_json::from_str(&header)?;

        let mut events = vec![];
        let mut line = String::new();
        loop {
            line.clear();
            if r.read_line(&mut line)? == 0 {
                break;
            };

            let (time_us, code, data): (f64, &str, String) = serde_json::from_str(&line)?;
            let time_us = (time_us * 1_000_000.0) as u64;
            let kind = match code {
                "o" => EventKind::Output(data),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown event code")),
            };
            events.push(Event { time_us, kind });
        }

        Ok(Asciicast { header, events })
    }
}

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Header {
    pub version: u8,
    pub width: u16,
    pub height: u16,
}

impl Header {
    pub fn write_to(&self, mut w: impl Write) -> io::Result<()> {
        writeln!(
            w,
            r#"{{"version": {}, "width": {}, "height": {}}}"#,
            self.version, self.width, self.height
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    pub time_us: u64,
    pub kind: EventKind,
}

#[derive(PartialEq, Eq, Debug)]
pub enum EventKind {
    Output(String),
}

impl Event {
    fn write_to(&self, mut w: impl Write) -> io::Result<()> {
        let (code, data) = match &self.kind {
            EventKind::Output(output) => ("o", output),
        };

        serde_json::to_writer(&mut w, &(self.time_us as f64 / 1_000_000.0, code, data))?;
        writeln!(w)
    }
}
