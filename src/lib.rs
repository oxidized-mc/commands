//! Brigadier-compatible command framework — CommandDispatcher, parsing engine, tab completion.
//!
//! Provides a generic command graph (`CommandDispatcher<S>`), argument types,
//! parsers, tab-completion, and serialization to the `ClientboundCommandsPacket`
//! wire format. The framework is fully generic over the command source type `S`.

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod argument_access;
pub mod argument_parser;
pub mod arguments;
pub mod context;
pub mod coordinates;
pub mod dispatcher;
pub mod nodes;
pub mod serializer;
pub mod string_reader;

pub use argument_access::{
    get_bool, get_color_str, get_double, get_float, get_gamemode_str, get_integer, get_long,
    get_string, get_time, get_uuid,
};
pub use argument_parser::{parse_argument, parse_range};
pub use arguments::{ArgumentType, StringKind};
pub use context::{
    ArgumentResult, CommandContext, ParseResults, ParsedArgument, StringRange, Suggestion,
};
pub use coordinates::{CoordinateKind, Coordinates, EntityAnchorKind, WorldCoordinate};
pub use dispatcher::CommandDispatcher;
pub use nodes::{
    ArgumentBuilder, ArgumentCommandNode, CommandFn, CommandNode, LiteralBuilder,
    LiteralCommandNode, RequirementFn, RootCommandNode,
};
pub use serializer::{CommandNodeData, CommandTreeData};
pub use string_reader::StringReader;

/// Errors from command parsing or execution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CommandError {
    /// The command could not be parsed (unknown command, bad syntax).
    #[error("{0}")]
    Parse(String),
    /// The command was parsed but execution failed.
    #[error("{0}")]
    Execution(String),
    /// The command exists in the tree but its logic is not yet implemented.
    #[error("command not yet implemented: {0}")]
    NotImplemented(String),
}
