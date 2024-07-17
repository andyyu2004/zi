use async_lsp::lsp_types;
use zi::{lstypes, Delta, Deltas, Diagnostic, Point, PointRange, PositionEncoding, Severity, Text};

pub fn goto_definition(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    res: Option<lsp_types::GotoDefinitionResponse>,
) -> Option<lstypes::GotoDefinitionResponse> {
    let res = match res {
        None => lstypes::GotoDefinitionResponse::Array(vec![]),
        Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
            lstypes::GotoDefinitionResponse::Array(vec![location(encoding, text, loc)?])
        }
        Some(lsp_types::GotoDefinitionResponse::Array(locs)) => {
            lstypes::GotoDefinitionResponse::Array(
                locs.into_iter().filter_map(|loc| location(encoding, text, loc)).collect(),
            )
        }
        Some(lsp_types::GotoDefinitionResponse::Link(links)) => {
            lstypes::GotoDefinitionResponse::Array(
                links
                    .into_iter()
                    .filter_map(|link| {
                        Some(lstypes::Location {
                            url: link.target_uri,
                            range: range(encoding, text, link.target_selection_range)?,
                        })
                    })
                    .collect(),
            )
        }
    };
    Some(res)
}

pub fn location(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    loc: lsp_types::Location,
) -> Option<lstypes::Location> {
    Some(lstypes::Location { url: loc.uri, range: range(encoding, text, loc.range)? })
}

pub fn deltas(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    edits: impl IntoIterator<Item = lsp_types::TextEdit, IntoIter: ExactSizeIterator>,
) -> Option<Deltas<'static>> {
    let edits = edits.into_iter();
    let n = edits.len();
    let deltas = Deltas::new(edits.filter_map(|edit| {
        let range = text.point_range_to_byte_range(range(encoding, text, edit.range)?);
        Some(Delta::new(range, edit.new_text))
    }));

    // If any of the edits were invalid, return None.
    if deltas.len() < n { None } else { Some(deltas) }
}

pub fn range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: lsp_types::Range,
) -> Option<PointRange> {
    Some(PointRange::new(point(encoding, text, range.start)?, point(encoding, text, range.end)?))
}

pub fn point(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    point: lsp_types::Position,
) -> Option<Point> {
    if point.line as usize > text.len_lines() {
        return None;
    }

    match encoding {
        PositionEncoding::Utf8 => Some(Point::new(point.line as usize, point.character as usize)),
        PositionEncoding::Utf16 => {
            let line_start_byte = text.line_to_byte(point.line as usize);
            let line_start_cu = text.byte_to_utf16_cu(line_start_byte);
            if line_start_cu + point.character as usize > text.len_utf16_cu() {
                return None;
            }

            let byte = text.utf16_cu_to_byte(line_start_cu + point.character as usize);
            Some(text.byte_to_point(byte))
        }
    }
}

pub fn diagnostics(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diags: impl IntoIterator<Item = lsp_types::Diagnostic>,
) -> Vec<Diagnostic> {
    diags.into_iter().filter_map(|diag| diagnostic(encoding, text, diag)).collect()
}

pub fn diagnostic(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diag: lsp_types::Diagnostic,
) -> Option<Diagnostic> {
    Some(Diagnostic {
        range: range(encoding, &text, diag.range)?,
        severity: match diag.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => Severity::Error,
            Some(lsp_types::DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => Severity::Info,
            Some(lsp_types::DiagnosticSeverity::HINT) => Severity::Hint,
            // Assume error if unspecified
            _ => Severity::Error,
        },
        message: diag.message,
    })
}

pub fn completion_response(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    res: lsp_types::CompletionResponse,
) -> lstypes::CompletionResponse {
    let items = match res {
        lsp_types::CompletionResponse::Array(items) => items,
        lsp_types::CompletionResponse::List(list) => list.items,
    };

    lstypes::CompletionResponse {
        items: items.into_iter().filter_map(|item| completion_item(encoding, text, item)).collect(),
    }
}

pub fn completion_item(
    _encoding: PositionEncoding,
    _text: &(impl Text + ?Sized),
    item: lsp_types::CompletionItem,
) -> Option<lstypes::CompletionItem> {
    Some(lstypes::CompletionItem {
        label: item.label,
        insert_text: item.insert_text,
        filter_text: item.filter_text,
    })
}

pub fn semantic_tokens(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    legend: &lsp_types::SemanticTokensLegend,
    theme: &zi::Theme,
    tokens: lsp_types::SemanticTokens,
) -> Vec<zi::MarkBuilder> {
    let mut line = 0;
    let mut char = 0;
    tokens
        .data
        .into_iter()
        .filter_map(|token| {
            if token.delta_line > 0 {
                char = 0;
            }

            line += token.delta_line;
            char += token.delta_start;

            let hl = semantic_tt_to_highlight(&legend.token_types[token.token_type as usize])
                .map(|name| theme.highlight_id_by_name(name))?;

            let point = point(encoding, text, lsp_types::Position::new(line, char))?;
            let start = text.point_to_byte(point);
            // TODO need to convert this length to the right encoding too...
            Some(zi::Mark::builder(start).width(token.length as usize).hl(hl))
        })
        .collect::<Vec<_>>()
}

// Naive mapping from semantic token types to highlight names for now
use zi::HighlightName;
fn semantic_tt_to_highlight(tt: &lsp_types::SemanticTokenType) -> Option<HighlightName> {
    use lsp_types::SemanticTokenType as Stt;
    Some(match tt {
        t if t == &Stt::NAMESPACE => HighlightName::NAMESPACE,
        t if t == &Stt::TYPE => HighlightName::TYPE,
        t if t == &Stt::STRUCT => HighlightName::TYPE,
        t if t == &Stt::CLASS => HighlightName::TYPE,
        t if t == &Stt::INTERFACE => HighlightName::TYPE,
        t if t == &Stt::ENUM => HighlightName::TYPE,
        t if t == &Stt::TYPE_PARAMETER => HighlightName::TYPE,
        t if t == &Stt::PARAMETER => HighlightName::PARAMETER,
        t if t == &Stt::VARIABLE => HighlightName::VARIABLE,
        t if t == &Stt::PROPERTY => HighlightName::PROPERTY,
        // t if t == &Stt::ENUM_MEMBER => HighlightName::ENUM_MEMBER,
        // t if t == &Stt::EVENT => HighlightName::EVENT,
        t if t == &Stt::FUNCTION => HighlightName::FUNCTION,
        t if t == &Stt::METHOD => HighlightName::FUNCTION,
        t if t == &Stt::MACRO => HighlightName::MACRO,
        t if t == &Stt::KEYWORD => HighlightName::KEYWORD,
        // t if t == &Stt::MODIFIER => HighlightName::MODIFIER,
        t if t == &Stt::COMMENT => HighlightName::COMMENT,
        t if t == &Stt::STRING => HighlightName::STRING,
        t if t == &Stt::NUMBER => HighlightName::NUMBER,
        t if t == &Stt::REGEXP => HighlightName::STRING,
        // t if t == &Stt::OPERATOR => HighlightName::OPERATOR,
        // t if t == &Stt::DECORATOR => HighlightName::DECORATOR,
        _ => return None,
    })
}
