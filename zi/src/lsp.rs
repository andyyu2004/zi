#[doc(hidden)]
pub mod from_proto;
#[doc(hidden)]
pub mod to_proto;

use std::fmt;

use zi_language_service::lsp_types::{self, ClientCapabilities};

use crate::{lsp, Client, LanguageServiceId};

pub struct LanguageClient {
    for_server: LanguageServiceId,
    client: Client,
}

impl LanguageClient {
    pub fn new(for_server: LanguageServiceId, client: Client) -> Self {
        Self { client, for_server }
    }
}

impl fmt::Debug for LanguageClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageClient").field("server", &self.for_server).finish()
    }
}

impl zi_language_service::LanguageClient for LanguageClient {
    fn log_message(&mut self, params: lsp_types::LogMessageParams) {
        self.client.send(move |editor| {
            tracing::info!("received log message");
            // TODO there are multiple levels of log messages
            editor.set_error(params.message);
            Ok(())
        });
    }

    fn publish_diagnostics(&mut self, params: lsp_types::PublishDiagnosticsParams) {
        let server = self.for_server;
        self.client.send(move |editor| {
            let Ok(path) = params.uri.to_file_path() else {
                tracing::warn!("received diagnostics for non-file URI: {}", params.uri);
                return Ok(());
            };

            tracing::info!(
                %server,
                ?path,
                version = params.version,
                n = params.diagnostics.len(),
                "received push diagnostics"
            );

            if let Some(server) = editor.active_language_services.get(&server) {
                let encoding = server.position_encoding();

                editor.update_diagnostics(
                    path,
                    params.version.map(|i| i as u32),
                    params
                        .diagnostics
                        .into_iter()
                        .map(|diag| lsp::from_proto::diagnostic(encoding, diag))
                        .collect::<Box<_>>(),
                );
            }

            Ok(())
        })
    }
}

pub fn client_capabilities() -> ClientCapabilities {
    const GOTO_CAPABILITY: Option<lsp_types::GotoCapability> = Some(lsp_types::GotoCapability {
        dynamic_registration: Some(false),
        link_support: Some(true),
    });

    lsp_types::ClientCapabilities {
        workspace: Some(lsp_types::WorkspaceClientCapabilities {
            semantic_tokens: Some(lsp_types::SemanticTokensWorkspaceClientCapabilities {
                refresh_support: None,
            }),
            ..Default::default()
        }),
        text_document: Some(lsp_types::TextDocumentClientCapabilities {
            declaration: GOTO_CAPABILITY,
            definition: GOTO_CAPABILITY,
            type_definition: GOTO_CAPABILITY,
            implementation: GOTO_CAPABILITY,
            diagnostic: Some(lsp_types::DiagnosticClientCapabilities {
                related_document_support: Some(true),
                ..Default::default()
            }),
            publish_diagnostics: Some(lsp_types::PublishDiagnosticsClientCapabilities {
                version_support: Some(true),
                ..Default::default()
            }),
            semantic_tokens: Some(lsp_types::SemanticTokensClientCapabilities {
                requests: lsp_types::SemanticTokensClientCapabilitiesRequests {
                    range: None,
                    full: Some(lsp_types::SemanticTokensFullOptions::Delta { delta: Some(true) }),
                },
                token_modifiers: vec![],
                token_types: vec![
                    lsp_types::SemanticTokenType::NAMESPACE,
                    lsp_types::SemanticTokenType::TYPE,
                    lsp_types::SemanticTokenType::CLASS,
                    lsp_types::SemanticTokenType::ENUM,
                    lsp_types::SemanticTokenType::INTERFACE,
                    lsp_types::SemanticTokenType::STRUCT,
                    lsp_types::SemanticTokenType::TYPE_PARAMETER,
                    lsp_types::SemanticTokenType::PARAMETER,
                    lsp_types::SemanticTokenType::VARIABLE,
                    lsp_types::SemanticTokenType::PROPERTY,
                    lsp_types::SemanticTokenType::ENUM_MEMBER,
                    lsp_types::SemanticTokenType::EVENT,
                    lsp_types::SemanticTokenType::FUNCTION,
                    lsp_types::SemanticTokenType::METHOD,
                    lsp_types::SemanticTokenType::MACRO,
                    lsp_types::SemanticTokenType::KEYWORD,
                    lsp_types::SemanticTokenType::MODIFIER,
                    lsp_types::SemanticTokenType::COMMENT,
                    lsp_types::SemanticTokenType::STRING,
                    lsp_types::SemanticTokenType::NUMBER,
                    lsp_types::SemanticTokenType::REGEXP,
                    lsp_types::SemanticTokenType::OPERATOR,
                ],
                formats: vec![lsp_types::TokenFormat::RELATIVE],
                multiline_token_support: None,
                augments_syntax_tokens: None,
                dynamic_registration: None,
                overlapping_token_support: None,
                server_cancel_support: None,
            }),
            ..Default::default()
        }),
        window: None,
        general: Some(lsp_types::GeneralClientCapabilities {
            position_encodings: Some(vec![
                // There are in order of preference
                lsp_types::PositionEncodingKind::UTF8,
                lsp_types::PositionEncodingKind::UTF16,
                // No support for UTF32 yet
            ]),
            ..Default::default()
        }),
        experimental: None,
    }
}
