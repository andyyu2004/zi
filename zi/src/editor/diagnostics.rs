use std::collections::HashMap;
use std::path::PathBuf;

use zi_text::PointRangeExt;

use super::request_redraw;
use crate::lstypes::{self, Diagnostic, Severity};
use crate::syntax::HighlightName;
use crate::{BufferId, Editor, Mark, Setting};

pub(super) type BufferDiagnostics = Setting<(u32, Box<[Diagnostic]>)>;

impl Editor {
    /// Return the current state of the raw diagnostics returned by the language servers.
    pub fn diagnostics(&self) -> &HashMap<PathBuf, BufferDiagnostics> {
        &self.diagnostics
    }

    pub fn replace_diagnostics(
        &mut self,
        path: PathBuf,
        version: Option<u32>,
        diagnostics: lstypes::Diagnostics,
    ) {
        let lstypes::Diagnostics::Full(diagnostics) = diagnostics else { return };
        let buf = self.buffer_at_path(&path);
        let version = version.unwrap_or_else(|| {
            // If there's a buffer with the same path, use its version.
            if let Some(buf) = buf { self[buf].version() } else { 0 }
        });

        let mut diagnostics: Box<[_]> = diagnostics.into();
        diagnostics.sort_unstable_by_key(|d| d.range.start());
        self.diagnostics.entry(path).or_default().write((version, diagnostics));

        if let Some(buf) = buf {
            self.refresh_diagnostic_marks(self[buf].id());
            request_redraw();
        }
    }

    fn refresh_diagnostic_marks(&mut self, buf: BufferId) {
        let ns = self.create_namespace(format!("lsp-diagnostics"));

        let Some(diagnostics) =
            self.buffer(buf).path().and_then(|path| self.diagnostics.get(&path))
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
        let marks = diags
            .iter()
            .cloned()
            .filter_map(|diag| {
                let hl_name = match diag.severity {
                    Severity::Error => HighlightName::ERROR,
                    Severity::Warning => HighlightName::WARNING,
                    Severity::Info => HighlightName::INFO,
                    Severity::Hint => HighlightName::HINT,
                };
                let hl = self.highlight_id_by_name(hl_name);
                // Need to explode out multi-line ranges into multiple single-line ranges.
                Some(diag.range.decode(text)?.explode(text).map(move |range| (range, hl)))
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
