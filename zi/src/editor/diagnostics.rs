use std::collections::HashMap;
use std::path::PathBuf;

use zi_core::Severity;
use zi_lsp::lsp_types;
use zi_text::PointRangeExt;

use super::request_redraw;
use crate::lsp::from_proto;
use crate::syntax::HighlightName;
use crate::{BufferId, Editor, LanguageServerId, Mark, Setting};

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
        let ns = self.create_namespace(format!("lsp-diagnostics-{server}-{buf:?}"));

        let Some(diagnostics) = self
            .buffer(buf)
            .path()
            .and_then(|path| self.lsp_diagnostics.get(&path))
            .and_then(|diagnostics| diagnostics.get(&server))
        else {
            return;
        };

        let buf_version = self[buf].version();
        let guard = diagnostics.read();
        let (version, diags) = &*guard;
        tracing::debug!(version, buf_version, "diagnostics");

        // If the diagnostics are from a different version of the text, we clear them.
        if *version != buf_version {
            tracing::debug!(
                "clearing diagnostics for `{}` because the version is outdated: {version} > {}",
                self[buf].path().expect("if we're here, the buffer has a path").display(),
                buf_version,
            );

            drop(guard);
            diagnostics.write((buf_version, vec![].into_boxed_slice()));
            self[buf].clear_marks(ns, ..);
            return;
        }

        let text = self[buf].text();
        let encoding = self.active_language_servers[&server].position_encoding();
        let marks = diags
            .iter()
            .cloned()
            .map(|diag| from_proto::diagnostic(encoding, text, diag))
            .filter_map(|diag| {
                let diag = diag?;
                let hl_name = match diag.severity {
                    Severity::Error => HighlightName::ERROR,
                    Severity::Warning => HighlightName::WARNING,
                    Severity::Info => HighlightName::INFO,
                    Severity::Hint => HighlightName::HINT,
                };
                let hl = self.highlight_id_by_name(hl_name);
                // Need to explode out multi-line ranges into multiple single-line ranges.
                Some(diag.range.explode(text).map(move |range| (range, hl)))
            })
            .flatten()
            .map(|(point_range, style)| {
                let byte_range = text.point_range_to_byte_range(point_range);
                let width = byte_range.end - byte_range.start;
                Mark::builder(byte_range.start).width(width).hl(style)
            })
            .collect::<Vec<_>>();

        drop(guard);

        self[buf].replace_marks(ns, marks);
    }
}
