use zi::Point;

use crate::wit::exports::zi::api::command;
use crate::wit::zi::api;

impl From<zi::Mode> for api::editor::Mode {
    fn from(mode: zi::Mode) -> Self {
        match mode {
            zi::Mode::Normal => api::editor::Mode::Normal,
            zi::Mode::Insert => api::editor::Mode::Insert,
            zi::Mode::Command => api::editor::Mode::Command,
            zi::Mode::Visual => api::editor::Mode::Visual,
            zi::Mode::VisualLine => api::editor::Mode::VisualLine,
            zi::Mode::VisualBlock => api::editor::Mode::VisualBlock,
            zi::Mode::OperatorPending(op) => api::editor::Mode::OperatorPending(op.into()),
            zi::Mode::ReplacePending => api::editor::Mode::ReplacePending,
        }
    }
}

impl From<api::editor::Mode> for zi::Mode {
    fn from(mode: api::editor::Mode) -> Self {
        match mode {
            api::editor::Mode::Normal => zi::Mode::Normal,
            api::editor::Mode::Insert => zi::Mode::Insert,
            api::editor::Mode::Command => zi::Mode::Command,
            api::editor::Mode::Visual => zi::Mode::Visual,
            api::editor::Mode::VisualLine => zi::Mode::VisualLine,
            api::editor::Mode::VisualBlock => zi::Mode::VisualBlock,
            api::editor::Mode::OperatorPending(op) => zi::Mode::OperatorPending(op.into()),
            api::editor::Mode::ReplacePending => zi::Mode::ReplacePending,
        }
    }
}

impl From<zi::Operator> for api::editor::Operator {
    fn from(op: zi::Operator) -> Self {
        match op {
            zi::Operator::Change => api::editor::Operator::Change,
            zi::Operator::Delete => api::editor::Operator::Delete,
            zi::Operator::Yank => api::editor::Operator::Yank,
        }
    }
}

impl From<api::editor::Operator> for zi::Operator {
    fn from(op: api::editor::Operator) -> Self {
        match op {
            api::editor::Operator::Change => zi::Operator::Change,
            api::editor::Operator::Delete => zi::Operator::Delete,
            api::editor::Operator::Yank => zi::Operator::Yank,
        }
    }
}

impl From<zi::EditError> for api::editor::EditError {
    fn from(err: zi::EditError) -> Self {
        match err {
            zi::EditError::Readonly => api::editor::EditError::Readonly,
        }
    }
}

impl From<Point> for api::editor::Point {
    fn from(value: Point) -> Self {
        Self { line: value.line() as u32, byte_col: value.byte_col() as u32 }
    }
}

impl From<api::editor::Point> for Point {
    fn from(value: api::editor::Point) -> Self {
        Self::from((value.line as usize, value.byte_col as usize))
    }
}

impl From<api::editor::Direction> for zi::Direction {
    fn from(dir: api::editor::Direction) -> Self {
        match dir {
            api::editor::Direction::Left => zi::Direction::Left,
            api::editor::Direction::Right => zi::Direction::Right,
            api::editor::Direction::Up => zi::Direction::Up,
            api::editor::Direction::Down => zi::Direction::Down,
        }
    }
}

impl From<api::editor::VerticalAlignment> for zi::VerticalAlignment {
    fn from(align: api::editor::VerticalAlignment) -> Self {
        match align {
            api::editor::VerticalAlignment::Top => zi::VerticalAlignment::Top,
            api::editor::VerticalAlignment::Center => zi::VerticalAlignment::Center,
            api::editor::VerticalAlignment::Bottom => zi::VerticalAlignment::Bottom,
        }
    }
}

impl From<command::Arity> for zi::command::Arity {
    fn from(arity: command::Arity) -> Self {
        zi::command::Arity { min: arity.min, max: arity.max }
    }
}

impl From<command::CommandFlags> for zi::command::CommandFlags {
    fn from(flags: command::CommandFlags) -> Self {
        // Ideally don't need transmute, but don't see a better non-manual way to do this conversion currently.
        zi::command::CommandFlags::from_bits(unsafe {
            std::mem::transmute::<crate::wit::exports::zi::api::command::CommandFlags, u8>(flags)
        })
        .expect("invalid command flags")
    }
}

impl From<zi::LanguageServiceStatus> for api::editor::LanguageServiceStatus {
    fn from(status: zi::LanguageServiceStatus) -> Self {
        let zi::LanguageServiceStatus { id, state, command, filetypes } = status;
        Self {
            id: id.to_string(),
            state: state.into(),
            command: command.map(|zi::ServiceCommand { program, args }| {
                api::editor::LanguageServer {
                    command: program.to_string_lossy().into_owned(),
                    args: args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect(),
                }
            }),
            filetypes: filetypes.iter().map(ToString::to_string).collect(),
        }
    }
}

impl From<zi::LanguageServiceState> for api::editor::LanguageServiceState {
    fn from(state: zi::LanguageServiceState) -> Self {
        match state {
            zi::LanguageServiceState::Running(details) => Self::Running(details.into()),
            zi::LanguageServiceState::Exited(details) => Self::Exited(details.into()),
            zi::LanguageServiceState::Starting => Self::Starting,
            zi::LanguageServiceState::Failed(err) => Self::Failed(err),
            zi::LanguageServiceState::NotStarted => Self::NotStarted,
            zi::LanguageServiceState::Undefined => Self::Undefined,
        }
    }
}

impl From<zi::ServiceDetails> for api::editor::LanguageServiceDetails {
    fn from(details: zi::ServiceDetails) -> Self {
        let zi::ServiceDetails { server_info, process_id, root, position_encoding, capabilities } =
            details;
        Self {
            server_info: server_info.map(|zi::lstypes::ServerInfo { name, version }| {
                api::editor::ServerInfo { name, version }
            }),
            pid: process_id,
            root: root.to_string_lossy().into_owned(),
            position_encoding: position_encoding.map(|encoding| match encoding {
                zi::lstypes::PositionEncoding::Utf8 => api::editor::PositionEncoding::Utf8,
                zi::lstypes::PositionEncoding::Utf16 => api::editor::PositionEncoding::Utf16,
            }),
            capabilities: capabilities.into(),
        }
    }
}

impl From<zi::ServiceCapabilities> for api::editor::LanguageServiceCapabilities {
    fn from(caps: zi::ServiceCapabilities) -> Self {
        use api::editor::LanguageServiceCapabilities as Wit;
        use zi::ServiceCapabilities as Zi;

        let mut out = Wit::empty();
        for (zi, wit) in [
            (Zi::DEFINITION, Wit::DEFINITION),
            (Zi::DECLARATION, Wit::DECLARATION),
            (Zi::IMPLEMENTATION, Wit::IMPLEMENTATION),
            (Zi::TYPE_DEFINITION, Wit::TYPE_DEFINITION),
            (Zi::REFERENCES, Wit::REFERENCES),
            (Zi::COMPLETION, Wit::COMPLETION),
            (Zi::DIAGNOSTICS, Wit::DIAGNOSTICS),
            (Zi::SEMANTIC_TOKENS, Wit::SEMANTIC_TOKENS),
            (Zi::FORMATTING, Wit::FORMATTING),
        ] {
            if caps.contains(zi) {
                out |= wit;
            }
        }
        out
    }
}
