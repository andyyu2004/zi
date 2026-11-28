//! `:lsp` shows the configured language services, whether they are running, and what they support.

use std::fmt::{self, Write as _};

use api::command::{self, Arity, Command, CommandError, CommandFlags};
use api::dependency;
use api::lifecycle::{self, InitializeResult};
use bindings::zi::api::editor::{
    self, Direction, LanguageServiceCapabilities, LanguageServiceDetails, LanguageServiceState,
    LanguageServiceStatus, PositionEncoding, ServerInfo,
};

use self::bindings::exports::zi::api;

struct Component;

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../zi-wasm/wit/zi.wit",
        ownership: Borrowing { duplicate_if_necessary: true },
    });
}

impl command::Guest for Component {
    type Handler = CommandHandler;
}

struct CommandHandler;

impl command::GuestHandler for CommandHandler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, _args: Vec<String>, _force: bool) -> Result<(), CommandError> {
        match cmd.as_str() {
            "lsp" => {
                let report = report().map_err(|err| CommandError::Message(err.to_string()))?;
                let buf = editor::create_scratch_buffer("[lsp]", &report);
                let view = editor::get_active_view().split(Direction::Up);
                view.set_buffer(&buf);
                Ok(())
            }
            _ => Err(CommandError::Message(format!("unexpected command: `{cmd}`"))),
        }
    }
}

fn report() -> Result<String, fmt::Error> {
    let services = editor::get_language_services();
    let buffers = editor::get_buffers()
        .into_iter()
        .filter_map(|buf| Some((buf.get_filetype(), buf.get_file_path()?)))
        .collect::<Vec<_>>();

    let mut out = String::new();
    if services.is_empty() {
        writeln!(out, "No language services are configured.")?;
    }

    for service in &services {
        write_service(&mut out, service, &buffers)?;
        writeln!(out)?;
    }

    writeln!(
        out,
        "Language server stderr is logged at debug level under the `zi_lsp::stderr` target."
    )?;
    writeln!(out, "Enable it with ZI_LOG=zi_lsp=debug.")?;
    Ok(out)
}

fn write_service(
    out: &mut String,
    service: &LanguageServiceStatus,
    buffers: &[(String, String)],
) -> fmt::Result {
    let LanguageServiceStatus { id, state, command, filetypes } = service;

    let (state_name, details) = match state {
        LanguageServiceState::Running(details) => ("running", Some(details)),
        LanguageServiceState::Exited(details) => ("exited", Some(details)),
        LanguageServiceState::Starting => ("starting", None),
        LanguageServiceState::Failed(_) => ("failed to start", None),
        LanguageServiceState::NotStarted => ("not started", None),
        LanguageServiceState::Undefined => ("not defined", None),
    };
    writeln!(out, "{id}: {state_name}")?;

    if let Some(command) = command {
        write!(out, "  command: {}", command.command)?;
        for arg in &command.args {
            write!(out, " {arg}")?;
        }
        writeln!(out)?;
    }

    if let LanguageServiceState::Failed(err) = state {
        writeln!(out, "  error: {err}")?;
    }

    writeln!(out, "  filetypes: {}", filetypes.join(" "))?;

    let Some(details) = details else { return Ok(()) };
    let LanguageServiceDetails { server_info, pid, root, position_encoding, capabilities } =
        details;

    if let Some(ServerInfo { name, version }) = server_info {
        match version {
            Some(version) => writeln!(out, "  server: {name} {version}")?,
            None => writeln!(out, "  server: {name}")?,
        }
    }

    if let Some(pid) = pid {
        writeln!(out, "  pid: {pid}")?;
    }

    writeln!(out, "  root: {root}")?;

    if let Some(encoding) = position_encoding {
        let encoding = match encoding {
            PositionEncoding::Utf8 => "utf-8",
            PositionEncoding::Utf16 => "utf-16",
        };
        writeln!(out, "  position encoding: {encoding}")?;
    }

    let capabilities = [
        (LanguageServiceCapabilities::DEFINITION, "definition"),
        (LanguageServiceCapabilities::DECLARATION, "declaration"),
        (LanguageServiceCapabilities::IMPLEMENTATION, "implementation"),
        (LanguageServiceCapabilities::TYPE_DEFINITION, "type definition"),
        (LanguageServiceCapabilities::REFERENCES, "references"),
        (LanguageServiceCapabilities::COMPLETION, "completion"),
        (LanguageServiceCapabilities::DIAGNOSTICS, "diagnostics"),
        (LanguageServiceCapabilities::SEMANTIC_TOKENS, "semantic tokens"),
        (LanguageServiceCapabilities::FORMATTING, "formatting"),
    ]
    .into_iter()
    .filter(|&(flag, _)| capabilities.contains(flag))
    .map(|(_, name)| name)
    .collect::<Vec<_>>();
    writeln!(out, "  capabilities: {}", capabilities.join(", "))?;

    writeln!(out, "  buffers:")?;
    for (_, path) in buffers.iter().filter(|(ft, _)| filetypes.contains(ft)) {
        writeln!(out, "    {path}")?;
    }

    Ok(())
}

impl lifecycle::Guest for Component {
    fn initialize() -> InitializeResult {
        InitializeResult {
            commands: vec![Command {
                name: "lsp".into(),
                arity: Arity { min: 0, max: 0 },
                opts: CommandFlags::empty(),
            }],
        }
    }

    fn shutdown() {}
}

impl dependency::Guest for Component {
    fn get_name() -> String {
        "lspinfo".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
