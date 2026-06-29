use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandEffect, ReplCommandId, ReplCommandStatus,
};
use serde_json::Value;

/// Byte span for the command name including the leading `:`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CliReplCommandSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

/// CLI-owned typed command after the shared REPL parser declined ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::agent::native) struct CliReplCommand {
    pub(super) anchor: CliReplCommandSpan,
    pub(super) kind: CliReplCommandKind,
}

/// CLI-owned inspection/debug command families.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CliReplCommandKind {
    Trace,
    Actions,
    Inspection(CliInspectionCommand),
    Capture(CliCaptureCommand),
    Query(CliQueryCommand),
    Drop(CliDropCommand),
    Save(CliSaveCommand),
    Connect(CliConnectCommand),
    Parse(CliParseCommand),
    Complete(CliCompleteCommand),
    Highlight(CliHighlightCommand),
    History,
    Bindings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliInspectionCommand {
    pub(super) kind: CliInspectionKind,
    pub(super) source: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CliInspectionKind {
    Type,
    Ast,
    Hir,
    Bytecode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliCaptureCommand {
    pub(super) target: CliCaptureTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CliCaptureTarget {
    Viewport,
    Layer { id: String },
    Object { id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliQueryCommand {
    pub(super) text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliDropCommand {
    pub(super) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliSaveCommand {
    pub(super) path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliConnectCommand {
    pub(super) target: CliConnectTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CliConnectTarget {
    Current,
    Source {
        path: String,
    },
    Profile {
        id: String,
        manifest: Option<String>,
    },
    StdioMcp {
        program: String,
        args: Vec<String>,
    },
    Inferred {
        target: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliParseCommand {
    pub(super) kind: CliParseKind,
    pub(super) source: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CliParseKind {
    Parse,
    Classify,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliCompleteCommand {
    pub(super) source_before_cursor: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliHighlightCommand {
    pub(super) source: String,
}

/// Typed CLI-local command result before human or JSON formatting.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app::agent::native) struct CliReplCommandResult {
    pub(super) command_id: ReplCommandId,
    pub(super) command_name: &'static str,
    pub(in crate::app::agent::native) status: ReplCommandStatus,
    pub(super) evidence: CliReplCommandEvidence,
    pub(super) diagnostics: Vec<ReplCommandDiagnostic>,
}

/// CLI-local evidence. Complex compiler/tooling surfaces keep their existing
/// deterministic JSON payloads, but the command family, source, target, and
/// status are typed and never recovered by parsing terminal text.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CliReplCommandEvidence {
    Empty,
    Trace {
        value: Value,
    },
    Actions {
        value: Value,
    },
    Inspection {
        kind: CliInspectionKind,
        source: String,
        value: Value,
    },
    Capture {
        target: CliCaptureTarget,
        value: Value,
    },
    Query {
        text: String,
        value: Value,
    },
    Drop {
        name: String,
        value: Value,
    },
    Save {
        path: String,
        value: Value,
    },
    Connect {
        target: CliConnectTarget,
        value: Value,
    },
    Parse {
        kind: CliParseKind,
        source: String,
        value: Value,
    },
    Complete {
        source_before_cursor: String,
        value: Value,
    },
    Highlight {
        source: String,
        value: Value,
    },
    History {
        value: Value,
    },
    Bindings {
        value: Value,
    },
}

impl CliReplCommandSpan {
    #[must_use]
    pub(super) const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl CliReplCommand {
    #[must_use]
    pub(super) const fn new(anchor: CliReplCommandSpan, kind: CliReplCommandKind) -> Self {
        Self { anchor, kind }
    }

    #[must_use]
    pub(super) fn name(&self) -> &'static str {
        self.kind.name()
    }

    #[must_use]
    pub(super) fn effect(&self) -> ReplCommandEffect {
        self.kind.effect()
    }
}

impl CliReplCommandKind {
    #[must_use]
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Trace => ":trace",
            Self::Actions => ":actions",
            Self::Inspection(command) => command.kind.name(),
            Self::Capture(_) => ":capture",
            Self::Query(_) => ":query",
            Self::Drop(_) => ":drop",
            Self::Save(_) => ":save",
            Self::Connect(_) => ":connect",
            Self::Parse(command) => command.kind.name(),
            Self::Complete(_) => ":complete",
            Self::Highlight(_) => ":highlight",
            Self::History => ":history",
            Self::Bindings => ":bindings",
        }
    }

    #[must_use]
    pub(super) fn effect(&self) -> ReplCommandEffect {
        match self {
            Self::Trace
            | Self::Actions
            | Self::Inspection(_)
            | Self::Query(_)
            | Self::Parse(_)
            | Self::Complete(_)
            | Self::Highlight(_)
            | Self::History
            | Self::Bindings => ReplCommandEffect::ReadOnly,
            Self::Capture(_) | Self::Save(_) | Self::Connect(_) => ReplCommandEffect::HostMutation,
            Self::Drop(_) => ReplCommandEffect::SessionMutation,
        }
    }
}

impl CliInspectionKind {
    #[must_use]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Type => ":type",
            Self::Ast => ":ast",
            Self::Hir => ":hir",
            Self::Bytecode => ":bytecode",
        }
    }

    #[must_use]
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Ast => "ast",
            Self::Hir => "hir",
            Self::Bytecode => "bytecode",
        }
    }
}

impl CliCaptureTarget {
    #[must_use]
    pub(super) fn to_repl_arg(&self) -> String {
        match self {
            Self::Viewport => String::new(),
            Self::Layer { id } => format!("layer {id}"),
            Self::Object { id } => format!("object {id}"),
        }
    }

    #[must_use]
    pub(super) fn label(&self) -> String {
        match self {
            Self::Viewport => "viewport".to_owned(),
            Self::Layer { id } => format!("layer:{id}"),
            Self::Object { id } => format!("object:{id}"),
        }
    }
}

impl CliConnectTarget {
    #[must_use]
    pub(super) fn label(&self) -> String {
        match self {
            Self::Current => "current".to_owned(),
            Self::Source { path } => format!("source:{path}"),
            Self::Profile { id, manifest } => format!(
                "profile:{id}:{}",
                manifest.as_deref().unwrap_or("<default-manifest>")
            ),
            Self::StdioMcp { program, args } => {
                if args.is_empty() {
                    format!("stdio:{program}")
                } else {
                    format!("stdio:{program} {}", args.join(" "))
                }
            }
            Self::Inferred { target } => format!("inferred:{target}"),
        }
    }
}

impl CliParseKind {
    #[must_use]
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Parse => ":parse",
            Self::Classify => ":classify",
        }
    }

    #[must_use]
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Classify => "classify",
        }
    }
}

impl CliReplCommandResult {
    #[must_use]
    pub(super) fn ok(
        command_id: ReplCommandId,
        command_name: &'static str,
        evidence: CliReplCommandEvidence,
    ) -> Self {
        Self {
            command_id,
            command_name,
            status: ReplCommandStatus::Ok,
            evidence,
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub(super) fn rejected(
        command_id: ReplCommandId,
        command_name: &'static str,
        evidence: CliReplCommandEvidence,
        diagnostic: ReplCommandDiagnostic,
    ) -> Self {
        Self {
            command_id,
            command_name,
            status: ReplCommandStatus::Rejected,
            evidence,
            diagnostics: vec![diagnostic],
        }
    }

    #[must_use]
    pub(super) fn error(
        command_id: ReplCommandId,
        command_name: &'static str,
        evidence: CliReplCommandEvidence,
        diagnostic: ReplCommandDiagnostic,
    ) -> Self {
        Self {
            command_id,
            command_name,
            status: ReplCommandStatus::Error,
            evidence,
            diagnostics: vec![diagnostic],
        }
    }
}
