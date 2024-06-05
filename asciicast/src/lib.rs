//! Minimal library to generate `asciicast(v2)`.

use std::io::{self, Write};

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
}

pub struct Header {
    pub version: u8,
    pub width: u16,
    pub height: u16,
}

impl Header {
    pub fn write_to(&self, mut w: impl Write) -> io::Result<()> {
        write!(
            w,
            r#"{{"version":{}, "width":{}, "height":{}}}"#,
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
            EventKind::Output(output) => {
                let s = serde_json::to_string(output)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
                ("o", s)
            }
        };

        let time = format!("{}.{:0>6}", self.time_us / 1_000_000, self.time_us % 1_000_000);
        write!(w, "[{time}, {code}, {data}]")
    }
}
