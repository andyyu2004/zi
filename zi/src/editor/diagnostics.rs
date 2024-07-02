use std::collections::HashMap;
use std::path::PathBuf;

use zi_lsp::lsp_types;

use super::request_redraw;
use crate::{BufferId, Editor, LanguageServerId, Setting};

pub(super) type LspDiagnostics = Setting<(u32, Box<[lsp_types::Diagnostic]>)>;

impl Editor {
    /// Return the current state of the raw diagnostics returned by the language servers.
    pub fn lsp_diagnostics(&self) -> &HashMap<PathBuf, HashMap<LanguageServerId, LspDiagnostics>> {
        &self.lsp_diagnostics
    }

    pub(crate) fn update_diagnostics(
        &mut self,
        server: LanguageServerId,
        path: PathBuf,
        version: Option<u32>,
        diagnostics: impl Into<Box<[lsp_types::Diagnostic]>>,
    ) {
        let buf = self.buffers.values().find(|b| {
            b.file_url().and_then(|url| url.to_file_path().ok()).as_deref() == Some(&path)
        });
        let version = version.unwrap_or_else(|| {
            // If there's a buffer with the same path, use its version.
            if let Some(buf) = buf { buf.version() } else { 0 }
        });

        let mut diagnostics: Box<[_]> = diagnostics.into();
        diagnostics.sort_unstable_by_key(|d| d.range.start);
        self.lsp_diagnostics
            .entry(path)
            .or_default()
            .entry(server)
            .or_default()
            .write((version, diagnostics));

        if let Some(buf) = buf {
            self.refresh_diagnostic_marks(server, buf.id());
            request_redraw();
        }
    }

    fn refresh_diagnostic_marks(&mut self, server: LanguageServerId, buf: BufferId) {
        let Some(diagnostics) =
            self.buffer(buf).path().and_then(|path| self.lsp_diagnostics.get(&path))
        else {
            return;
        };

        let ns = self.create_namespace(format!("lsp-diagnostics-{server}-{buf:?}"));
        // let ns = self[buf].clear_marks();
    }
}
