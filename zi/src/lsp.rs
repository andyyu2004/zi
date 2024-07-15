#[doc(hidden)]
pub mod from_proto;
#[doc(hidden)]
pub mod to_proto;

use std::fmt;

use futures_core::future::BoxFuture;
use zi_language_service::lsp_types::request::Request;
use zi_language_service::lsp_types::{self, ClientCapabilities};

use crate::{Client, LanguageServiceId};

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

impl zi_language_service::LanguageClient for LanguageClient {}

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
