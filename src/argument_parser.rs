//! Per-type argument parsers and dispatch.

use crate::CommandError;
use crate::arguments::ArgumentType;
use crate::context::ArgumentResult;
use crate::coordinates::{
    EntityAnchorKind, parse_coordinates2, parse_coordinates3, parse_int_coordinates3,
};
use crate::string_reader::StringReader;
use std::fmt::Display;
use std::str::FromStr;

/// Ticks per in-game second (20 TPS).
const TICKS_PER_SECOND: i32 = 20;

/// Ticks per in-game day (24 000 ticks).
const TICKS_PER_GAME_DAY: i32 = 24_000;

// ── Range validation ────────────────────────────────────────────────

/// Validates that a value is within optional min/max bounds.
pub(crate) fn validate_range<T: PartialOrd + Display>(
    value: T,
    min: Option<&T>,
    max: Option<&T>,
    type_name: &str,
) -> Result<T, CommandError> {
    if let Some(lo) = min {
        if value < *lo {
            return Err(CommandError::Parse(format!(
                "{type_name} must not be less than {lo}, found {value}"
            )));
        }
    }
    if let Some(hi) = max {
        if value > *hi {
            return Err(CommandError::Parse(format!(
                "{type_name} must not be more than {hi}, found {value}"
            )));
        }
    }
    Ok(value)
}

// ── Generic range parsing ───────────────────────────────────────────

/// Parses a `min..max` range from a string. Handles open ranges (`5..`,
/// `..10`) and exact values (`42`).
pub fn parse_range<T: FromStr + Copy>(
    input: &str,
    type_name: &str,
) -> Result<(Option<T>, Option<T>), CommandError> {
    if let Some((min_s, max_s)) = input.split_once("..") {
        let min = if min_s.is_empty() {
            None
        } else {
            Some(min_s.parse::<T>().map_err(|_| {
                CommandError::Parse(format!("Invalid {type_name} range minimum: '{min_s}'"))
            })?)
        };
        let max = if max_s.is_empty() {
            None
        } else {
            Some(max_s.parse::<T>().map_err(|_| {
                CommandError::Parse(format!("Invalid {type_name} range maximum: '{max_s}'"))
            })?)
        };
        Ok((min, max))
    } else {
        let v = input
            .parse::<T>()
            .map_err(|_| CommandError::Parse(format!("Invalid {type_name} range: '{input}'")))?;
        Ok((Some(v), Some(v)))
    }
}

// ── Per-type argument parsers ───────────────────────────────────────

fn parse_bool_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    Ok(ArgumentResult::Bool(reader.read_bool()?))
}

fn parse_integer_arg(
    reader: &mut StringReader<'_>,
    min: &Option<i32>,
    max: &Option<i32>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_integer()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Integer")?;
    Ok(ArgumentResult::Integer(v))
}

fn parse_long_arg(
    reader: &mut StringReader<'_>,
    min: &Option<i64>,
    max: &Option<i64>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_long()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Long")?;
    Ok(ArgumentResult::Long(v))
}

fn parse_float_arg(
    reader: &mut StringReader<'_>,
    min: &Option<f32>,
    max: &Option<f32>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_float()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Float")?;
    Ok(ArgumentResult::Float(v))
}

fn parse_double_num_arg(
    reader: &mut StringReader<'_>,
    min: &Option<f64>,
    max: &Option<f64>,
) -> Result<ArgumentResult, CommandError> {
    let v = reader.read_double()?;
    let v = validate_range(v, min.as_ref(), max.as_ref(), "Double")?;
    Ok(ArgumentResult::Double(v))
}

fn parse_block_pos_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_int_coordinates3(reader)?;
    if !coords.has_relative() {
        return Ok(ArgumentResult::BlockPos(
            coords.x.value as i32,
            coords.y.value as i32,
            coords.z.value as i32,
        ));
    }
    Ok(ArgumentResult::Coordinates(coords))
}

fn parse_vec3_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates3(reader)?;
    if !coords.has_relative() {
        return Ok(ArgumentResult::Vec3(
            coords.x.value,
            coords.y.value,
            coords.z.value,
        ));
    }
    Ok(ArgumentResult::Coordinates(coords))
}

fn parse_vec2_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates2(reader)?;
    if !coords.has_relative() {
        return Ok(ArgumentResult::Vec3(coords.x.value, 0.0, coords.z.value));
    }
    Ok(ArgumentResult::Coordinates(coords))
}

fn parse_gamemode_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    Ok(ArgumentResult::Gamemode(word.to_string()))
}

fn parse_time_arg(reader: &mut StringReader<'_>, min: i32) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let (num_str, multiplier) = if let Some(stripped) = word.strip_suffix('d') {
        (stripped, TICKS_PER_GAME_DAY)
    } else if let Some(stripped) = word.strip_suffix('s') {
        (stripped, TICKS_PER_SECOND)
    } else if let Some(stripped) = word.strip_suffix('t') {
        (stripped, 1)
    } else {
        (word, 1) // default: ticks
    };
    let v: i32 = num_str
        .parse()
        .map_err(|_| CommandError::Parse(format!("Expected time value, got '{word}'")))?;
    let ticks = v
        .checked_mul(multiplier)
        .ok_or_else(|| CommandError::Parse(format!("Time value too large: '{word}'")))?;
    if ticks < min {
        return Err(CommandError::Parse(format!(
            "Time must not be less than {min} ticks, found {ticks}"
        )));
    }
    Ok(ArgumentResult::Time(ticks))
}

fn parse_uuid_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let uuid = uuid::Uuid::parse_str(word)
        .map_err(|_| CommandError::Parse(format!("Invalid UUID: '{word}'")))?;
    Ok(ArgumentResult::Uuid(uuid))
}

/// Reads a single word as a string argument (fallback for unimplemented types).
fn parse_word_as_string(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    Ok(ArgumentResult::String(reader.read_word().to_string()))
}

fn parse_color_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    Ok(ArgumentResult::Color(word.to_string()))
}

fn parse_angle_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let remaining = reader.remaining();
    if remaining.starts_with('~') {
        reader.advance(1);
        let value = if reader.can_read() && reader.peek() != Some(' ') {
            reader.read_float()?
        } else {
            0.0
        };
        Ok(ArgumentResult::Angle {
            value,
            is_relative: true,
        })
    } else {
        let value = reader.read_float()?;
        Ok(ArgumentResult::Angle {
            value,
            is_relative: false,
        })
    }
}

fn parse_entity_anchor_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    match word {
        "feet" => Ok(ArgumentResult::EntityAnchor(EntityAnchorKind::Feet)),
        "eyes" => Ok(ArgumentResult::EntityAnchor(EntityAnchorKind::Eyes)),
        _ => Err(CommandError::Parse(format!(
            "Invalid anchor: '{word}' (expected 'feet' or 'eyes')"
        ))),
    }
}

fn parse_swizzle_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let mut mask = 0u8;
    for ch in word.chars() {
        match ch {
            'x' => mask |= 0b001,
            'y' => mask |= 0b010,
            'z' => mask |= 0b100,
            _ => {
                return Err(CommandError::Parse(format!(
                    "Invalid axis in swizzle: '{ch}' (expected x, y, or z)"
                )));
            },
        }
    }
    if mask == 0 {
        return Err(CommandError::Parse(
            "Swizzle must contain at least one axis".to_string(),
        ));
    }
    Ok(ArgumentResult::Swizzle(mask))
}

fn parse_int_range_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let (min, max) = parse_range::<i32>(word, "integer")?;
    Ok(ArgumentResult::IntRange { min, max })
}

fn parse_float_range_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let word = reader.read_word();
    let (min, max) = parse_range::<f64>(word, "float")?;
    Ok(ArgumentResult::FloatRange { min, max })
}

/// Parses rotation (pitch yaw) with support for `~` relative syntax.
fn parse_rotation_arg(reader: &mut StringReader<'_>) -> Result<ArgumentResult, CommandError> {
    let coords = parse_coordinates2(reader)?;
    // Store rotation as Coordinates (x=pitch, z=yaw).
    Ok(ArgumentResult::Coordinates(coords))
}

// ── Main dispatch ───────────────────────────────────────────────────

/// Parses an argument value from a `StringReader` given the argument type.
pub fn parse_argument(
    reader: &mut StringReader<'_>,
    arg_type: &ArgumentType,
) -> Result<ArgumentResult, CommandError> {
    match arg_type {
        ArgumentType::Bool => parse_bool_arg(reader),
        ArgumentType::Integer { min, max } => parse_integer_arg(reader, min, max),
        ArgumentType::Long { min, max } => parse_long_arg(reader, min, max),
        ArgumentType::Float { min, max } => parse_float_arg(reader, min, max),
        ArgumentType::Double { min, max } => parse_double_num_arg(reader, min, max),
        ArgumentType::String(kind) => Ok(ArgumentResult::String(reader.read_string(*kind))),
        // Entity selectors / game profiles / messages are complex —
        // read a word as a raw string for now (full parsing in a later phase).
        ArgumentType::Entity { .. } | ArgumentType::GameProfile | ArgumentType::Message => {
            parse_word_as_string(reader)
        },
        ArgumentType::BlockPos => parse_block_pos_arg(reader),
        ArgumentType::Vec3 => parse_vec3_arg(reader),
        ArgumentType::Vec2 => parse_vec2_arg(reader),
        ArgumentType::Gamemode => parse_gamemode_arg(reader),
        ArgumentType::Color => parse_color_arg(reader),
        ArgumentType::Angle => parse_angle_arg(reader),
        ArgumentType::EntityAnchor => parse_entity_anchor_arg(reader),
        ArgumentType::Swizzle => parse_swizzle_arg(reader),
        ArgumentType::IntRange => parse_int_range_arg(reader),
        ArgumentType::FloatRange => parse_float_range_arg(reader),
        ArgumentType::Rotation => parse_rotation_arg(reader),
        ArgumentType::ResourceLocation
        | ArgumentType::Dimension
        | ArgumentType::Function
        | ArgumentType::ResourceOrTag { .. }
        | ArgumentType::ResourceOrTagKey { .. }
        | ArgumentType::Resource { .. }
        | ArgumentType::ResourceKey { .. }
        | ArgumentType::ResourceSelector { .. }
        | ArgumentType::LootTable
        | ArgumentType::LootPredicate
        | ArgumentType::LootModifier
        | ArgumentType::Dialog => parse_word_as_string(reader),
        ArgumentType::Time { min } => parse_time_arg(reader, *min),
        ArgumentType::Uuid => parse_uuid_arg(reader),
        // All remaining types: parse as a single word string for now.
        _ => parse_word_as_string(reader),
    }
}
