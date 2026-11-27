use std::collections::BTreeSet;

use super::*;
use crate::{LanguageServiceState, LanguageServiceStatus, ServiceCapabilities, ServiceDetails};

impl Editor {
    /// The state of every language service that is defined or that a language uses, by id.
    pub fn language_services(&self) -> Vec<LanguageServiceStatus> {
        self.language_config
            .language_services
            .keys()
            .copied()
            .chain(
                self.language_config
                    .languages
                    .values()
                    .flat_map(|config| config.language_services.iter().copied()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| self.language_service_status(id))
            .collect()
    }

    fn language_service_status(&self, id: LanguageServiceId) -> LanguageServiceStatus {
        let state = if let Some(instance) = self.active_language_services.get(&id) {
            let details = self.service_details(id, instance);
            if instance.is_running() {
                LanguageServiceState::Running(details)
            } else {
                LanguageServiceState::Exited(details)
            }
        } else if self
            .starting_language_services
            .get(&id)
            .is_some_and(|starting| starting.strong_count() > 0)
        {
            LanguageServiceState::Starting
        } else if let Some(err) = self.failed_language_services.get(&id) {
            LanguageServiceState::Failed(err.clone())
        } else if self.language_config.has_language_service(id) {
            LanguageServiceState::NotStarted
        } else {
            LanguageServiceState::Undefined
        };

        LanguageServiceStatus {
            id,
            state,
            command: self
                .language_config
                .language_services
                .get(&id)
                .and_then(|config| config.command()),
            filetypes: self
                .language_config
                .languages
                .iter()
                .filter(|(_, config)| config.language_services.contains(&id))
                .map(|(&ft, _)| ft)
                .collect(),
        }
    }

    fn service_details(
        &self,
        id: LanguageServiceId,
        service: &LanguageServiceInstance,
    ) -> ServiceDetails {
        let mut capabilities = ServiceCapabilities::empty();
        for (flag, cap) in [
            (ServiceCapabilities::DEFINITION, service.definition_capabilities()),
            (ServiceCapabilities::DECLARATION, service.declaration_capabilities()),
            (ServiceCapabilities::IMPLEMENTATION, service.implementation_capabilities()),
            (ServiceCapabilities::TYPE_DEFINITION, service.type_definition_capabilities()),
            (ServiceCapabilities::REFERENCES, service.reference_capabilities()),
            (ServiceCapabilities::COMPLETION, service.completion_capabilities()),
            (ServiceCapabilities::DIAGNOSTICS, service.diagnostic_capabilities()),
            (ServiceCapabilities::SEMANTIC_TOKENS, service.semantic_tokens_capabilities()),
            (ServiceCapabilities::FORMATTING, service.formatting_capabilities()),
        ] {
            capabilities.set(flag, cap.is_some());
        }

        ServiceDetails {
            server_info: service.server_info().cloned(),
            process_id: service.process_id(),
            root: self.lsp_root_path(id),
            position_encoding: service.position_encoding(),
            capabilities,
        }
    }
}
