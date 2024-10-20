use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::ops::{Bound, Deref, RangeBounds, RangeInclusive};
use std::pin::Pin;
use std::str::FromStr;

use anyhow::bail;
use chumsky::primitive::end;
use chumsky::text::{digits, ident, newline, whitespace};
use chumsky::Parser;
use smol_str::SmolStr;

use crate::editor::SaveFlags;
// use crate::plugin::PluginId;
// use crate::wit::exports::zi::api::command::{Arity, CommandFlags};
use crate::{Active, BufferFlags, Client, Editor, Error, OpenFlags};

pub struct Commands(Box<[Command]>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arity {
    pub min: u8,
    pub max: u8,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CommandFlags: u8 {
        const RANGE = 0b0001;
    }
}

impl fmt::Debug for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for cmd in self.iter() {
            write!(f, "{cmd:?};")?;
        }
        Ok(())
    }
}

impl Commands {
    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.0.iter()
    }
}

impl IntoIterator for Commands {
    type Item = Command;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_vec().into_iter()
    }
}

impl FromStr for Commands {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        commands().then_ignore(end()).parse(s).map_err(|errs| {
            use std::fmt::Write;
            let mut msg = String::new();
            for err in errs {
                write!(msg, "{err}").unwrap();
            }
            anyhow::anyhow!("{msg}")
        })
    }
}

#[derive(Clone)]
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
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        command().then_ignore(whitespace()).then_ignore(end()).parse(s).map_err(|errs| {
            use std::fmt::Write;
            let mut msg = String::new();
            for err in errs {
                write!(msg, "{err}").unwrap();
            }
            anyhow::anyhow!("{msg}")
        })
    }
}

impl TryFrom<&str> for Command {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

fn commands() -> impl Parser<char, Commands, Error = chumsky::error::Simple<char>> {
    use chumsky::prelude::*;

    command()
        .separated_by(just(';').ignored().or(newline()))
        .allow_trailing()
        .map(|commands| Commands(commands.into_boxed_slice()))
}

fn command() -> impl Parser<char, Command, Error = chumsky::error::Simple<char>> {
    command_kind().map(|kind| Command { range: None, kind })
}

fn command_kind() -> impl Parser<char, CommandKind, Error = chumsky::error::Simple<char>> {
    use chumsky::prelude::*;

    // A generic command is just a bunch of whitespace separated words.
    // The first word is the command, the rest are string arguments.

    ident()
        .or(digits(10))
        .separated_by(filter(|&c: &char| c.is_whitespace() && c != '\n').ignored().repeated())
        .at_least(1)
        .allow_leading()
        .allow_trailing()
        .map(|words| {
            let mut words = words.into_iter().map(|s| Word::try_from(s).unwrap());
            let cmd = words.next().expect("expect at least 1 word");
            let args = words.collect::<Box<_>>();
            CommandKind::Generic(cmd, args)
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

pub struct Handler {
    name: Word,
    arity: Arity,
    opts: CommandFlags,
    f: HandlerFn,
}

pub type HandlerFn = Box<
    dyn Fn(
            Client,
            Option<CommandRange>,
            Box<[Word]>,
        ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>>
        + Send,
>;

pub fn handler<Fut>(
    f: impl Fn(Client, Option<CommandRange>, Box<[Word]>) -> Fut + Send + 'static,
) -> HandlerFn
where
    Fut: Future<Output = crate::Result<()>> + Send + 'static,
{
    Box::new(move |client, range, args| Box::pin(f(client, range, args)))
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
    pub fn new(name: impl Into<Word>, arity: Arity, opts: CommandFlags, f: HandlerFn) -> Self {
        Self { name: name.into(), arity, opts, f }
    }

    pub fn execute(
        &self,
        editor: &Editor,
        range: Option<CommandRange>,
        args: Box<[Word]>,
    ) -> Result<(), Error> {
        self.check(range.as_ref(), &args)?;
        let fut = (self.f)(editor.client(), range, args);
        editor.spawn("command handler", fut);
        Ok(())
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
    const ZERO: Self = Self::exact(0);

    const fn exact(n: u8) -> Self {
        Self { min: n, max: n }
    }
}

pub(crate) fn builtin_handlers() -> HashMap<Word, Handler> {
    [
        Handler {
            name: "q".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());
                client.with(|editor| editor.close_view(Active)).await;
                Ok(())
            })
            .into(),
        },
        Handler {
            name: "w".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());
                let () =
                    client.with(|editor| editor.save(Active, SaveFlags::empty())).await.await?;
                Ok(())
            }),
        },
        Handler {
            name: "e".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());

                client
                    .with(|editor| async move {
                        let buf = editor.buffer(Active);
                        let Some(path) = buf.file_path() else { return Ok(()) };
                        if buf.flags().contains(BufferFlags::DIRTY) {
                            bail!("buffer is dirty")
                        }

                        let mut open_flags = OpenFlags::FORCE;
                        if buf.flags().contains(BufferFlags::READONLY) {
                            open_flags |= OpenFlags::READONLY;
                        }

                        editor.open(path, open_flags)?.await?;
                        Ok(())
                    })
                    .await
                    .await?;
                Ok(())
            }),
        },
        Handler {
            name: "jumps".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());

                client.with(|editor| editor.open_jump_list(Active));
                Ok(())
            })
            .into(),
        },
        Handler {
            name: "inspect".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());
                client.with(|editor| editor.inspect(Active)).await;
                Ok(())
            })
            .into(),
        },
        Handler {
            name: "explore".try_into().unwrap(),
            arity: Arity::ZERO,
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.is_empty());
                client.with(|editor| editor.open_file_explorer(".")).await;
                Ok(())
            })
            .into(),
        },
        // `set x y` to set parameter `x` to value `y`
        Handler {
            name: "set".try_into().unwrap(),
            arity: Arity::exact(2),
            opts: CommandFlags::empty(),
            f: handler(|client, range, args| async move {
                assert!(range.is_none());
                assert!(args.len() == 2);

                client.with(move |editor| set(editor, &args[0], &args[1])).await
            })
            .into(),
        },
    ]
    .into_iter()
    .map(|handler| (handler.name.clone(), handler))
    .collect()
}

fn set(editor: &Editor, key: &Word, value: &Word) -> crate::Result<()> {
    let buf = editor.buffer(Active).settings();
    let view = editor.view(Active).settings();

    match key.as_str() {
        "tabstop" | "ts" | "tabwidth" => buf.tab_width.write(value.parse()?),
        "numberwidth" | "nuw" => view.line_number_width.write(value.parse()?),
        "numberstyle" | "nus" => view.line_number_style.write(value.parse()?),
        _ => anyhow::bail!("unknown parameter: `{key}`"),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
