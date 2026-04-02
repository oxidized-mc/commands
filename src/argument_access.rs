//! Typed argument getters for extracting parsed values from a command context.

use crate::CommandError;
use crate::context::{ArgumentResult, CommandContext};

/// Looks up a parsed argument by name.
fn get_arg_result<'a, S>(
    ctx: &'a CommandContext<S>,
    name: &str,
) -> Result<&'a ArgumentResult, CommandError> {
    ctx.arguments
        .get(name)
        .map(|a| &a.result)
        .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))
}

/// Extracts a typed value from a named argument using an extractor closure.
fn get_typed<S, T>(
    ctx: &CommandContext<S>,
    name: &str,
    type_name: &str,
    extract: impl FnOnce(&ArgumentResult) -> Option<T>,
) -> Result<T, CommandError> {
    extract(get_arg_result(ctx, name)?)
        .ok_or_else(|| CommandError::Parse(format!("Argument '{name}' is not {type_name}")))
}

/// Gets an integer argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not an integer.
pub fn get_integer<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    get_typed(ctx, name, "an integer", |r| match r {
        ArgumentResult::Integer(v) => Some(*v),
        _ => None,
    })
}

/// Gets a long argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a long.
pub fn get_long<S>(ctx: &CommandContext<S>, name: &str) -> Result<i64, CommandError> {
    get_typed(ctx, name, "a long", |r| match r {
        ArgumentResult::Long(v) => Some(*v),
        _ => None,
    })
}

/// Gets a float argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a float.
pub fn get_float<S>(ctx: &CommandContext<S>, name: &str) -> Result<f32, CommandError> {
    get_typed(ctx, name, "a float", |r| match r {
        ArgumentResult::Float(v) => Some(*v),
        _ => None,
    })
}

/// Gets a double argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a double.
pub fn get_double<S>(ctx: &CommandContext<S>, name: &str) -> Result<f64, CommandError> {
    get_typed(ctx, name, "a double", |r| match r {
        ArgumentResult::Double(v) => Some(*v),
        _ => None,
    })
}

/// Gets a boolean argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a boolean.
pub fn get_bool<S>(ctx: &CommandContext<S>, name: &str) -> Result<bool, CommandError> {
    get_typed(ctx, name, "a boolean", |r| match r {
        ArgumentResult::Bool(v) => Some(*v),
        _ => None,
    })
}

/// Gets a string argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a string.
pub fn get_string<'a, S>(ctx: &'a CommandContext<S>, name: &str) -> Result<&'a str, CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::String(v) => Ok(v.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a string"
        ))),
    }
}

/// Gets a gamemode argument by name as a raw string.
///
/// Returns the canonical gamemode name string. The caller is responsible for
/// mapping this to a game-specific enum if needed.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a game mode.
pub fn get_gamemode_str<'a, S>(
    ctx: &'a CommandContext<S>,
    name: &str,
) -> Result<&'a str, CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::Gamemode(s) => Ok(s.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a game mode"
        ))),
    }
}

/// Gets a time argument by name (in ticks).
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a time or integer value.
pub fn get_time<S>(ctx: &CommandContext<S>, name: &str) -> Result<i32, CommandError> {
    match get_arg_result(ctx, name)? {
        // Accept both Time and raw Integer as ticks
        ArgumentResult::Time(v) | ArgumentResult::Integer(v) => Ok(*v),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a time value"
        ))),
    }
}

/// Gets a color argument by name as a raw string.
///
/// Returns the color name string. The caller is responsible for mapping this
/// to a game-specific formatting enum if needed.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a color.
pub fn get_color_str<'a, S>(
    ctx: &'a CommandContext<S>,
    name: &str,
) -> Result<&'a str, CommandError> {
    match get_arg_result(ctx, name)? {
        ArgumentResult::Color(s) => Ok(s.as_str()),
        _ => Err(CommandError::Parse(format!(
            "Argument '{name}' is not a color"
        ))),
    }
}

/// Gets a UUID argument by name.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if no argument named `name` exists or
/// the argument is not a UUID.
pub fn get_uuid<S>(ctx: &CommandContext<S>, name: &str) -> Result<uuid::Uuid, CommandError> {
    get_typed(ctx, name, "a UUID", |r| match r {
        ArgumentResult::Uuid(v) => Some(*v),
        _ => None,
    })
}
