use std::fmt;
use std::ops::{Bound, Deref, RangeBounds, RangeInclusive};
use std::str::FromStr;

use chumsky::Parser;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use crate::plugin::PluginId;
use crate::wit::exports::zi::api::command::{Arity, CommandFlags};
use crate::{Active, Editor, Error};

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
    //         let cmd = words.next().expect("expect at least 1 word");
    //         let args = words.collect::<Box<_>>();
    //         CommandKind::Generic(cmd, args)
    //     })

    any().repeated().at_least(1).try_map(|text, span| {
        let s = text.into_iter().collect::<String>();
        let mut words = s.split_whitespace().map(Word::try_from).map(Result::unwrap);
        let cmd = words
            .next()
            .ok_or_else(|| chumsky::error::Simple::custom(span, "expected at least 1 word"))?;
        let args = words.collect::<Box<_>>();
        Ok(CommandKind::Generic(cmd, args))
    })
}

/// A single word in a command, without whitespace.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Word(SmolStr);

impl Word {
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for Word {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&Word> for String {
    fn from(value: &Word) -> Self {
        value.0.clone().into()
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

impl TryFrom<String> for Word {
    type Error = &'static str;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.as_str().try_into()
    }
}

impl TryFrom<&str> for Word {
    type Error = &'static str;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.chars().all(|c| !c.is_whitespace()) {
            Ok(Word(s.into()))
        } else {
            Err("word contains whitespace")
        }
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

#[derive(Clone)]
pub struct Handler {
    name: Word,
    arity: Arity,
    opts: CommandFlags,
    handler: CommandHandler,
}

#[derive(Clone, Copy)]
pub enum CommandHandler {
    Local(LocalHandler),
    Remote(PluginId),
}

#[derive(Clone, Copy)]
pub struct LocalHandler(fn(&mut Editor, Option<&CommandRange>, &[Word]) -> crate::Result<()>);

impl From<LocalHandler> for CommandHandler {
    fn from(v: LocalHandler) -> Self {
        Self::Local(v)
    }
}

impl From<RangeInclusive<u8>> for Arity {
    fn from(range: RangeInclusive<u8>) -> Self {
        let (min, max) = range.into_inner();
        Self { min, max }
    }
}

impl RangeBounds<u8> for Arity {
    fn start_bound(&self) -> Bound<&u8> {
        Bound::Included(&self.min)
    }

    fn end_bound(&self) -> Bound<&u8> {
        Bound::Included(&self.max)
    }
}

impl Handler {
    pub fn new(
        name: impl Into<Word>,
        arity: Arity,
        opts: CommandFlags,
        handler: impl Into<CommandHandler>,
    ) -> Self {
        Self { name: name.into(), arity, opts, handler: handler.into() }
    }

    pub fn execute(
        &self,
        editor: &mut Editor,
        range: Option<&CommandRange>,
        args: &[Word],
    ) -> Result<(), Error> {
        self.check(range, args)?;
        match self.handler {
            CommandHandler::Local(f) => (f.0)(editor, range, args),
            CommandHandler::Remote(id) => {
                let plugins = editor.plugins();
                let name = self.name.clone();
                let args = args.iter().map(Into::into).collect::<Box<_>>();
                editor.schedule(format!("plugin command {name}"), async move {
                    plugins.execute(id, name, None, args).await
                });
                Ok(())
            }
        }
    }

    fn check(&self, range: Option<&CommandRange>, args: &[Word]) -> Result<(), Error> {
        if !self.arity.contains(&(args.len() as u8)) {
            if self.arity.min == self.arity.max {
                anyhow::bail!("expected {} arguments, got {}", self.arity.min, args.len())
            }

            anyhow::bail!(
                "expected {} to {} arguments, got {}",
                self.arity.min,
                self.arity.max,
                args.len()
            )
        }

        if range.is_some() && !self.opts.contains(CommandFlags::RANGE) {
            anyhow::bail!("range not allowed")
        }

        Ok(())
    }

    pub fn name(&self) -> Word {
        Word::clone(&self.name)
    }
}

impl Arity {
    const ZERO: Self = Self { min: 0, max: 0 };
}

pub(crate) fn builtin_handlers() -> FxHashMap<Word, Handler> {
    [
        Handler {
            name: "q".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            handler: LocalHandler(|editor, range, args| {
                assert!(range.is_none());
                assert!(args.is_empty());
                editor.close_view(Active);
                Ok(())
            })
            .into(),
        },
        Handler {
            name: "jumps".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            handler: LocalHandler(|editor, range, args| {
                assert!(range.is_none());
                assert!(args.is_empty());

                editor.open_jump_list();

                Ok(())
            })
            .into(),
        },
        Handler {
            name: "inspect".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            handler: LocalHandler(|editor, range, args| {
                assert!(range.is_none());
                assert!(args.is_empty());
                editor.inspect();
                Ok(())
            })
            .into(),
        },
        Handler {
            name: "explore".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            handler: LocalHandler(|editor, range, args| {
                assert!(range.is_none());
                assert!(args.is_empty());
                editor.open_file_explorer(".");
                Ok(())
            })
            .into(),
        },
    ]
    .into_iter()
    .map(|handler| (handler.name.clone(), handler))
    .collect()
}

#[cfg(test)]
mod tests;
