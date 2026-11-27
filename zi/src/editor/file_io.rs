use std::io;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use zi_text::{AnyText, ReadonlyText, Rope, RopeBuilder, Text};

use super::{Client, OpenFlags};
use crate::buffer::{self, Buffer, TextBuffer};
use crate::syntax::Syntax;
use crate::{BufferId, FileType, Setting, Theme};

pub(super) enum OpenPlan {
    Reuse(BufferId),
    Load { replace: Option<BufferId> },
}

pub(super) struct OpenFile {
    pub(super) path: PathBuf,
    pub(super) ft: FileType,
    pub(super) syntax: Option<Box<dyn Syntax>>,
    pub(super) theme: Setting<Theme>,
}

impl OpenFile {
    pub(super) async fn load(
        self,
        client: &Client,
        replace: Option<BufferId>,
        flags: OpenFlags,
    ) -> io::Result<BufferId> {
        let id = if flags.contains(OpenFlags::READONLY) {
            debug_assert!(self.path.exists() && self.path.is_file());
            // TODO: consider taking an advisory lock while the file is mapped.
            let text = unsafe { ReadonlyText::open(&self.path) }?;
            let settings = buffer::Settings { readonly: Setting::new(true), ..Default::default() };
            self.install(client, replace, text, settings).await
        } else {
            let text = if self.path.exists() {
                rope_from_reader(tokio::fs::File::open(&self.path).await?).await?
            } else {
                Rope::new()
            };
            self.install(client, replace, text, Default::default()).await
        };
        Ok(id)
    }

    async fn install<T: Text + Clone + 'static>(
        self,
        client: &Client,
        replace: Option<BufferId>,
        text: T,
        settings: buffer::Settings,
    ) -> BufferId {
        let Self { path, ft, syntax, theme } = self;
        client
            .with(move |editor| {
                let make_buffer = |id| {
                    Buffer::new(TextBuffer::new(
                        id,
                        settings,
                        ft,
                        &path,
                        text,
                        &theme.read(),
                        syntax,
                    ))
                };
                // The previous buffer can disappear while file I/O is in flight.
                if let Some(id) = replace.filter(|&id| editor.has_buffer(id)) {
                    *editor.db.buffer_mut(id) = make_buffer(id);
                    id
                } else {
                    editor.db.insert_buffer(make_buffer)
                }
            })
            .await
    }
}

pub(super) async fn write_buffer(path: &Path, text: &dyn AnyText) -> io::Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    let mut reader = futures_util::io::AllowStdIo::new(text.reader()).compat();
    let mut writer = tokio::io::BufWriter::new(&mut file);
    tokio::io::copy(&mut reader, &mut writer).await?;
    writer.flush().await?;
    file.flush().await
}

async fn rope_from_reader(reader: impl tokio::io::AsyncRead + Unpin) -> io::Result<Rope> {
    let mut reader = tokio::io::BufReader::new(reader);
    let mut builder = RopeBuilder::new();

    // Handle utf-8 byte order mark.
    // Not supporting other encodings for now.
    let buf = reader.fill_buf().await?;
    if let [0xEF, 0xBB, 0xBF, ..] = buf {
        // Skip the BOM before decoding. This means we won't preserve it on save but we don't care.
        reader.consume(3);
    }

    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            break;
        }

        let s = match std::str::from_utf8(buf) {
            Ok(s) => s,
            Err(err) => {
                let n = err.valid_up_to();
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, err));
                }
                unsafe { std::str::from_utf8_unchecked(&buf[..n]) }
            }
        };

        builder.append(s);
        let n = s.len();
        reader.consume(n);
    }

    Ok(builder.build())
}
