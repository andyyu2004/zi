use std::fmt;
use std::str::FromStr;

use chumsky::Parser;

pub enum CommandRange {}

impl fmt::Debug for CommandRange {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

pub struct Command {
    range: Option<CommandRange>,
    kind: CommandKind,
}

impl Command {
    pub fn range(&self) -> Option<&CommandRange> {
        self.range.as_ref()
    }

    pub fn kind(&self) -> &CommandKind {
        &self.kind
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(range) = &self.range {
            write!(f, "{:?} ", range)?;
        }

        write!(f, "{:?}", self.kind)?;
        Ok(())
    }
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        command().parse(s).map_err(|errs| {
            use std::fmt::Write;
            let mut msg = String::new();
            for err in errs {
                write!(msg, "{err}").unwrap();
            }
            msg
        })
    }
}

fn command() -> impl Parser<char, Command, Error = chumsky::error::Simple<char>> {
    command_kind().map(|kind| Command { range: None, kind })
}

fn command_kind() -> impl Parser<char, CommandKind, Error = chumsky::error::Simple<char>> {
    use chumsky::prelude::*;

    // A generic command is just a bunch of whitespace separated words.
    // The first word is the command, the rest are string arguments.

    // Something like the following hangs.
    // whitespace()
    //     .not()
    //     .repeated()
    //     .separated_by(whitespace())
    //     .at_least(1)
    //     .allow_leading()
    //     .allow_trailing()
    //     .map(|text: Vec<Vec<char>>| {
    //         let mut words =
    //             text.into_iter().map(|word| Word::from(word.into_iter().collect::<String>()));
    //         let cmd = words.next().expect("expect at least 1");
    //         let args = words.collect::<Box<_>>();
    //         CommandKind::Generic(cmd, args)
    //     })

    any().repeated().at_least(1).map(|text: Vec<char>| {
        let s = text.into_iter().collect::<String>();
        let mut words = s.split_whitespace().map(Word::from);
        let cmd = words.next().expect("expect at least 1");
        let args = words.collect::<Box<_>>();
        CommandKind::Generic(cmd, args)
    })
}

/// A single word in a command, without whitespace.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Word(String);

impl Word {
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl From<String> for Word {
    fn from(s: String) -> Self {
        assert!(s.chars().all(|c| !c.is_whitespace()));
        Word(s)
    }
}

impl From<&str> for Word {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

pub enum CommandKind {
    Generic(Word, Box<[Word]>),
}

impl fmt::Debug for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandKind::Generic(cmd, args) => {
                write!(f, "{cmd}")?;
                for arg in args.iter() {
                    write!(f, " {arg}")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
