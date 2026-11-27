use std::collections::BTreeSet;

use wasmtime::component::Resource;
use zi::command;
use zi::{Active, Editor, FileType, LanguageConfig, LanguageServiceId, Point, SaveFlags};
use zi_lsp::LanguageServerConfig;

use super::state::HostState;
use crate::wit::zi::api;

impl api::editor::HostView for HostState {
    async fn exists(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<bool> {
        let view = self.view_id(&view)?;
        Ok(self.client().with(move |editor| editor.has_view(view)).await)
    }

    async fn get_buffer(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Resource<api::editor::Buffer>> {
        let buf_id = self.with_view(&view, |editor, view| editor.view(view).buffer()).await?;
        Ok(self.push_buffer(buf_id))
    }

    async fn get_cursor(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<api::editor::Point> {
        self.with_view(&view, |editor, view| editor.view(view).cursor().into()).await
    }

    async fn set_cursor(
        &mut self,
        view: Resource<api::editor::View>,
        pos: api::editor::Point,
    ) -> wasmtime::Result<()> {
        self.with_view(&view, move |editor, view| {
            editor.set_cursor(view, Point::from(pos));
        })
        .await
    }

    async fn close(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        let view_id = self.view_id(&view)?;
        command::close_view(self.client(), view_id).await;
        Ok(())
    }

    async fn save(
        &mut self,
        view: Resource<api::editor::View>,
        force: bool,
    ) -> wasmtime::Result<Result<(), String>> {
        let flags = if force { SaveFlags::FORCE } else { SaveFlags::empty() };
        let fut = self.with_view(&view, move |editor, view| editor.save(view, flags)).await?;
        Ok(fut.await.map_err(|e| e.to_string()))
    }

    async fn inspect(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.with_view(&view, Editor::inspect).await
    }

    async fn insert(
        &mut self,
        view: Resource<api::editor::View>,
        text: String,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, move |editor, view| editor.insert(view, &text)).await?;
        Ok(res.map_err(Into::into))
    }

    async fn delete_char(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, Editor::delete_char).await?;
        Ok(res.map_err(Into::into))
    }

    async fn paste_after(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, Editor::paste_after).await?;
        Ok(res.map_err(Into::into))
    }

    async fn undo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<bool, api::editor::EditError>> {
        let res = self.with_view(&view, Editor::undo).await?;
        Ok(res.map_err(Into::into))
    }

    async fn redo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<bool, api::editor::EditError>> {
        let res = self.with_view(&view, Editor::redo).await?;
        Ok(res.map_err(Into::into))
    }

    async fn scroll_by(
        &mut self,
        view: Resource<api::editor::View>,
        direction: api::editor::Direction,
        amount: u32,
    ) -> wasmtime::Result<()> {
        let dir = zi::Direction::from(direction);
        self.with_view(&view, move |editor, view| editor.scroll_by(view, dir, amount as usize))
            .await
    }

    async fn align(
        &mut self,
        view: Resource<api::editor::View>,
        alignment: api::editor::VerticalAlignment,
    ) -> wasmtime::Result<()> {
        let align = zi::VerticalAlignment::from(alignment);
        self.with_view(&view, move |editor, view| editor.align_view(view, align)).await
    }

    async fn focus(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.with_view(&view, Editor::focus).await
    }

    async fn split(
        &mut self,
        view: Resource<api::editor::View>,
        direction: api::editor::Direction,
    ) -> wasmtime::Result<Resource<api::editor::View>> {
        let direction = zi::Direction::from(direction);
        let new = self
            .with_view(&view, move |editor, view| {
                editor.split(view, direction, zi::Constraint::Fill(1))
            })
            .await?;
        Ok(self.push_view(new))
    }

    async fn set_buffer(
        &mut self,
        view: Resource<api::editor::View>,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<()> {
        let buf = self.buffer_id(&buf)?;
        self.with_view(&view, move |editor, view| {
            if editor.has_buffer(buf) {
                editor.set_buffer(view, buf);
                Ok(())
            } else {
                Err(wasmtime::Error::msg(format!("buffer {buf:?} no longer exists")))
            }
        })
        .await?
    }

    #[expect(clippy::unused_async_trait_impl, reason = "keep resource removal deferred until poll")]
    async fn drop(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.forget_view(&view);
        Ok(())
    }
}

impl api::editor::HostBuffer for HostState {
    async fn exists(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<bool> {
        let buf = self.buffer_id(&buf)?;
        Ok(self.client().with(move |editor| editor.has_buffer(buf)).await)
    }

    async fn get_file_path(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<Option<String>> {
        self.with_buffer(&buf, |editor, buf| {
            editor.buffer(buf).file_path().map(|p| p.to_string_lossy().into_owned())
        })
        .await
    }

    async fn get_flags(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<api::editor::BufferFlags> {
        self.with_buffer(&buf, |editor, buf| {
            let buf = editor.buffer(buf);
            let mut flags = api::editor::BufferFlags::empty();
            if buf.flags().contains(zi::BufferFlags::DIRTY) {
                flags |= api::editor::BufferFlags::DIRTY;
            }
            flags
        })
        .await
    }

    async fn len_lines(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<u32> {
        self.with_buffer(&buf, |editor, buf| editor.buffer(buf).text().len_lines() as u32).await
    }

    async fn get_filetype(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<String> {
        self.with_buffer(&buf, |editor, buf| editor.buffer(buf).file_type().to_string()).await
    }

    #[expect(clippy::unused_async_trait_impl, reason = "keep resource removal deferred until poll")]
    async fn drop(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<()> {
        self.forget_buffer(&buf);
        Ok(())
    }
}

impl api::editor::Host for HostState {
    async fn insert(
        &mut self,
        text: String,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        Ok(self.client().with(move |editor| editor.insert(Active, &text)).await.map_err(Into::into))
    }

    async fn get_mode(&mut self) -> wasmtime::Result<api::editor::Mode> {
        Ok(self.client().with(|editor| editor.mode()).await.into())
    }

    async fn set_mode(&mut self, mode: api::editor::Mode) -> wasmtime::Result<()> {
        self.client().with(move |editor| editor.set_mode(mode.into())).await;
        Ok(())
    }

    async fn get_active_view(&mut self) -> wasmtime::Result<Resource<api::editor::View>> {
        let view_id = self.client().with(|editor| editor.view(Active).id()).await;
        Ok(self.push_view(view_id))
    }

    async fn save_all(&mut self, force: bool) -> wasmtime::Result<Result<(), String>> {
        Ok(command::save_all(self.client(), force).await.map_err(|e| e.to_string()))
    }

    async fn reload(&mut self) -> wasmtime::Result<Result<(), String>> {
        Ok(command::reload(self.client()).await.map_err(|e| e.to_string()))
    }

    async fn open_jump_list(&mut self) -> wasmtime::Result<()> {
        self.client()
            .with(|editor| {
                editor.open_jump_list(Active);
            })
            .await;
        Ok(())
    }

    async fn open_file_explorer(&mut self, path: String) -> wasmtime::Result<()> {
        self.client().with(move |editor| editor.open_file_explorer(path)).await;
        Ok(())
    }

    async fn set_option(
        &mut self,
        key: String,
        value: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .client()
            .with(move |editor| command::set_option(editor, &key, &value))
            .await
            .map_err(|e| e.to_string()))
    }

    async fn add_language_server(
        &mut self,
        id: String,
        server: api::editor::LanguageServer,
    ) -> wasmtime::Result<()> {
        let api::editor::LanguageServer { command, args } = server;
        let server = LanguageServerConfig::new(command, args.into_iter().map(Into::into));
        self.client()
            .with(move |editor| {
                editor.language_config_mut().add_language_service(id.as_str(), server);
            })
            .await;
        Ok(())
    }

    async fn get_language_services(
        &mut self,
    ) -> wasmtime::Result<Vec<api::editor::LanguageServiceStatus>> {
        let services = self.client().with(|editor| editor.language_services()).await;
        Ok(services.into_iter().map(Into::into).collect())
    }

    async fn get_buffers(&mut self) -> wasmtime::Result<Vec<Resource<api::editor::Buffer>>> {
        let bufs = self
            .client()
            .with(|editor| editor.buffers().map(zi::buffer::Buffer::id).collect::<Vec<_>>())
            .await;
        Ok(bufs.into_iter().map(|buf| self.push_buffer(buf)).collect())
    }

    async fn create_scratch_buffer(
        &mut self,
        name: String,
        text: String,
    ) -> wasmtime::Result<Resource<api::editor::Buffer>> {
        let buf = self
            .client()
            .with(move |editor| {
                let buf = editor.create_readonly_buffer(name, text.into_bytes());
                editor[buf].settings().buf_hidden.write(zi::buffer::BufHidden::Wipe);
                buf
            })
            .await;
        Ok(self.push_buffer(buf))
    }

    async fn add_language(
        &mut self,
        filetype: String,
        language_servers: Vec<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .client()
            .with(move |editor| {
                let config = editor.language_config_mut();
                let ids = language_servers.iter().map(|id| LanguageServiceId::new(id.as_str()));
                if let Some(id) = ids.clone().find(|&id| !config.has_language_service(id)) {
                    return Err(format!(
                        "language server `{id}` for language `{filetype}` is not defined"
                    ));
                }
                let mut seen = BTreeSet::new();
                if let Some(id) = ids.clone().find(|&id| !seen.insert(id)) {
                    return Err(format!(
                        "language server `{id}` is listed more than once for language `{filetype}`"
                    ));
                }
                config.add_language(FileType::from_name(&filetype), LanguageConfig::new(ids));
                Ok(())
            })
            .await)
    }
}
