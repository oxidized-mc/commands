//! Command parsing context, parsed arguments, and result types.

use crate::coordinates::{Coordinates, EntityAnchorKind};
use crate::nodes::CommandFn;
use std::collections::HashMap;

/// A parsed command ready for execution.
pub struct CommandContext<S> {
    /// The source that invoked the command.
    pub source: S,
    /// The full input string.
    pub input: String,
    /// Parsed arguments keyed by name.
    pub arguments: HashMap<String, ParsedArgument>,
    /// The command function to execute (if any).
    pub command: Option<CommandFn<S>>,
}

/// A single parsed argument with its position and value.
#[derive(Debug, Clone)]
pub struct ParsedArgument {
    /// Range in the input string.
    pub range: StringRange,
    /// The parsed value.
    pub result: ArgumentResult,
}

/// Typed result of parsing an argument.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentResult {
    /// A boolean value.
    Bool(bool),
    /// A 32-bit integer.
    Integer(i32),
    /// A 64-bit integer.
    Long(i64),
    /// A 32-bit float.
    Float(f32),
    /// A 64-bit float.
    Double(f64),
    /// A string.
    String(String),
    /// An (x, y, z) integer position, possibly relative.
    BlockPos(i32, i32, i32),
    /// An (x, y, z) double-precision position.
    Vec3(f64, f64, f64),
    /// World coordinates supporting `~` relative and `^` local syntax.
    Coordinates(Coordinates),
    /// A game mode (stored as the canonical name string).
    Gamemode(String),
    /// A resource location string.
    ResourceLocation(String),
    /// A UUID.
    Uuid(uuid::Uuid),
    /// A time value in ticks.
    Time(i32),
    /// A named color (stored as the canonical name string).
    Color(String),
    /// An angle value (possibly relative).
    Angle {
        /// The angle in degrees.
        value: f32,
        /// Whether relative to the source's current angle.
        is_relative: bool,
    },
    /// An entity anchor point.
    EntityAnchor(EntityAnchorKind),
    /// A set of axes (e.g. `xy`, `xz`, `xyz`).
    Swizzle(u8),
    /// A min..max integer range.
    IntRange {
        /// Inclusive minimum (if any).
        min: Option<i32>,
        /// Inclusive maximum (if any).
        max: Option<i32>,
    },
    /// A min..max float range.
    FloatRange {
        /// Inclusive minimum (if any).
        min: Option<f64>,
        /// Inclusive maximum (if any).
        max: Option<f64>,
    },
}

/// A range within the input string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringRange {
    /// Start index (inclusive).
    pub start: usize,
    /// End index (exclusive).
    pub end: usize,
}

impl StringRange {
    /// Creates a new range.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the length of the range.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A tab-completion suggestion.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Range in the input string to replace.
    pub range: StringRange,
    /// The suggested text.
    pub text: String,
    /// An optional tooltip shown to the player.
    pub tooltip: Option<String>,
}

/// Results from parsing input against the command graph.
pub struct ParseResults<S> {
    /// The built context (with parsed args and command fn).
    pub context: CommandContext<S>,
    /// How far we parsed successfully.
    pub cursor: usize,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::argument_access::get_integer;
    use crate::argument_parser::{parse_argument, validate_range};
    use crate::arguments::{ArgumentType, StringKind};
    use crate::coordinates::CoordinateKind;
    use crate::string_reader::StringReader;

    // ── StringReader tests ──────────────────────────────────────────

    #[test]
    fn quotable_phrase_handles_escape_sequences() {
        let input = r#""hello \"world\"""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, r#"hello "world""#);
    }

    #[test]
    fn quotable_phrase_handles_escaped_backslash() {
        let input = r#""path\\to\\file""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, r"path\to\file");
    }

    #[test]
    fn quotable_phrase_no_escapes() {
        let input = r#""simple text""#;
        let mut reader = StringReader::new(input, 0);
        let result = reader.read_string(StringKind::QuotablePhrase);
        assert_eq!(result, "simple text");
    }

    // ── validate_range tests ────────────────────────────────────────

    #[test]
    fn validate_range_in_bounds() {
        assert_eq!(validate_range(5, Some(&1), Some(&10), "Int").unwrap(), 5);
    }

    #[test]
    fn validate_range_below_min() {
        let err = validate_range(0, Some(&1), Some(&10), "Int").unwrap_err();
        assert!(err.to_string().contains("must not be less than 1"));
    }

    #[test]
    fn validate_range_above_max() {
        let err = validate_range(11, Some(&1), Some(&10), "Int").unwrap_err();
        assert!(err.to_string().contains("must not be more than 10"));
    }

    #[test]
    fn validate_range_no_bounds() {
        assert_eq!(validate_range(42, None, None, "Int").unwrap(), 42);
    }

    #[test]
    fn validate_range_float_bounds() {
        assert!(validate_range(0.5_f64, Some(&0.0), Some(&1.0), "Double").is_ok());
        assert!(validate_range(-0.1_f64, Some(&0.0), Some(&1.0), "Double").is_err());
    }

    // ── Coordinate parsing tests ────────────────────────────────────

    #[test]
    fn parse_block_pos_valid_integers() {
        let mut reader = StringReader::new("10 20 30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(10, 20, 30));
    }

    #[test]
    fn parse_block_pos_negative_integers() {
        let mut reader = StringReader::new("-5 0 -10", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(-5, 0, -10));
    }

    #[test]
    fn parse_vec3_valid_doubles() {
        let mut reader = StringReader::new("1.5 2.5 3.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        assert_eq!(result, ArgumentResult::Vec3(1.5, 2.5, 3.5));
    }

    #[test]
    fn parse_vec2_valid_doubles() {
        let mut reader = StringReader::new("1.0 3.0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec2).unwrap();
        assert_eq!(result, ArgumentResult::Vec3(1.0, 0.0, 3.0));
    }

    // ── parse_argument tests ────────────────────────────────────────

    #[test]
    fn time_overflow_returns_error() {
        let mut reader = StringReader::new("89479d", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert!(
            result.is_err(),
            "should reject time values that overflow i32"
        );
    }

    #[test]
    fn time_valid_days() {
        let mut reader = StringReader::new("1d", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert_eq!(result.unwrap(), ArgumentResult::Time(24000));
    }

    #[test]
    fn time_valid_seconds() {
        let mut reader = StringReader::new("5s", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Time { min: 0 });
        assert_eq!(result.unwrap(), ArgumentResult::Time(100));
    }

    #[test]
    fn parse_integer_with_range() {
        let mut reader = StringReader::new("5", 0);
        let result = parse_argument(
            &mut reader,
            &ArgumentType::Integer {
                min: Some(1),
                max: Some(10),
            },
        );
        assert_eq!(result.unwrap(), ArgumentResult::Integer(5));
    }

    #[test]
    fn parse_integer_below_min() {
        let mut reader = StringReader::new("0", 0);
        let result = parse_argument(
            &mut reader,
            &ArgumentType::Integer {
                min: Some(1),
                max: None,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn parse_vec2_returns_vec3_with_zero_y() {
        let mut reader = StringReader::new("1.0 3.0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec2);
        assert_eq!(result.unwrap(), ArgumentResult::Vec3(1.0, 0.0, 3.0));
    }

    #[test]
    fn parse_block_pos_three_ints() {
        let mut reader = StringReader::new("10 64 -30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos);
        assert_eq!(result.unwrap(), ArgumentResult::BlockPos(10, 64, -30));
    }

    #[test]
    fn parse_uuid_valid() {
        let mut reader = StringReader::new("550e8400-e29b-41d4-a716-446655440000", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Uuid);
        assert!(matches!(result.unwrap(), ArgumentResult::Uuid(_)));
    }

    // ── Typed getter tests ──────────────────────────────────────────

    #[test]
    fn get_typed_missing_argument() {
        let ctx: CommandContext<()> = CommandContext {
            source: (),
            input: String::new(),
            arguments: HashMap::new(),
            command: None,
        };
        let err = get_integer(&ctx, "missing").unwrap_err();
        assert!(err.to_string().contains("No argument named 'missing'"));
    }

    #[test]
    fn get_typed_wrong_type() {
        let mut args = HashMap::new();
        args.insert(
            "val".to_string(),
            ParsedArgument {
                range: StringRange::new(0, 1),
                result: ArgumentResult::Bool(true),
            },
        );
        let ctx: CommandContext<()> = CommandContext {
            source: (),
            input: String::new(),
            arguments: args,
            command: None,
        };
        let err = get_integer(&ctx, "val").unwrap_err();
        assert!(err.to_string().contains("is not an integer"));
    }

    // ── Relative coordinate parsing tests ───────────────────────────

    #[test]
    fn parse_vec3_relative_tilde() {
        let mut reader = StringReader::new("~10 ~ ~-5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.is_relative);
                assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
                assert!(coords.y.is_relative);
                assert!((coords.y.value - 0.0).abs() < f64::EPSILON);
                assert!(coords.z.is_relative);
                assert!((coords.z.value - -5.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_vec3_absolute_returns_vec3() {
        let mut reader = StringReader::new("100.5 64.0 -200.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        // All absolute → should return Vec3, not Coordinates
        assert_eq!(result, ArgumentResult::Vec3(100.5, 64.0, -200.5));
    }

    #[test]
    fn parse_vec3_local_caret() {
        let mut reader = StringReader::new("^1 ^0 ^2", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::Local);
                assert!(coords.x.is_relative);
                assert!((coords.x.value - 1.0).abs() < f64::EPSILON);
                assert!((coords.y.value - 0.0).abs() < f64::EPSILON);
                assert!((coords.z.value - 2.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_vec3_mixed_tilde_caret_rejected() {
        let mut reader = StringReader::new("~1 ^0 ~2", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3);
        assert!(result.is_err());
    }

    #[test]
    fn parse_vec3_bare_tilde() {
        let mut reader = StringReader::new("~ ~ ~", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Vec3).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.is_relative);
                assert!((coords.x.value).abs() < f64::EPSILON);
                assert!(coords.y.is_relative);
                assert!(coords.z.is_relative);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_block_pos_relative() {
        let mut reader = StringReader::new("~5 ~0 ~-3", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert_eq!(coords.kind, CoordinateKind::World);
                assert!(coords.x.is_relative);
                assert!((coords.x.value - 5.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates, got {result:?}"),
        }
    }

    #[test]
    fn parse_block_pos_absolute_returns_block_pos() {
        let mut reader = StringReader::new("10 64 -30", 0);
        let result = parse_argument(&mut reader, &ArgumentType::BlockPos).unwrap();
        assert_eq!(result, ArgumentResult::BlockPos(10, 64, -30));
    }

    // ── Argument type parsing tests ─────────────────────────────────

    #[test]
    fn parse_color_valid() {
        let mut reader = StringReader::new("red", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color("red".to_string()));
    }

    #[test]
    fn parse_color_dark_aqua() {
        let mut reader = StringReader::new("dark_aqua", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color("dark_aqua".to_string()));
    }

    #[test]
    fn parse_color_unknown_still_parses() {
        // In the library, color validation is deferred to the game layer.
        let mut reader = StringReader::new("pink", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color("pink".to_string()));
    }

    #[test]
    fn parse_color_accepts_any_word_as_string() {
        // In the library, color validation is deferred to the game layer.
        let mut reader = StringReader::new("bold", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Color).unwrap();
        assert_eq!(result, ArgumentResult::Color("bold".to_string()));
    }

    #[test]
    fn parse_angle_absolute() {
        let mut reader = StringReader::new("45.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 45.5,
                is_relative: false
            }
        );
    }

    #[test]
    fn parse_angle_relative() {
        let mut reader = StringReader::new("~10", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 10.0,
                is_relative: true
            }
        );
    }

    #[test]
    fn parse_angle_bare_tilde() {
        let mut reader = StringReader::new("~", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Angle).unwrap();
        assert_eq!(
            result,
            ArgumentResult::Angle {
                value: 0.0,
                is_relative: true
            }
        );
    }

    #[test]
    fn parse_entity_anchor_feet() {
        let mut reader = StringReader::new("feet", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor).unwrap();
        assert_eq!(result, ArgumentResult::EntityAnchor(EntityAnchorKind::Feet));
    }

    #[test]
    fn parse_entity_anchor_eyes() {
        let mut reader = StringReader::new("eyes", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor).unwrap();
        assert_eq!(result, ArgumentResult::EntityAnchor(EntityAnchorKind::Eyes));
    }

    #[test]
    fn parse_entity_anchor_invalid() {
        let mut reader = StringReader::new("head", 0);
        let result = parse_argument(&mut reader, &ArgumentType::EntityAnchor);
        assert!(result.is_err());
    }

    #[test]
    fn parse_swizzle_xyz() {
        let mut reader = StringReader::new("xyz", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b111));
    }

    #[test]
    fn parse_swizzle_xz() {
        let mut reader = StringReader::new("xz", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b101));
    }

    #[test]
    fn parse_swizzle_y() {
        let mut reader = StringReader::new("y", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle).unwrap();
        assert_eq!(result, ArgumentResult::Swizzle(0b010));
    }

    #[test]
    fn parse_swizzle_invalid() {
        let mut reader = StringReader::new("w", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Swizzle);
        assert!(result.is_err());
    }

    #[test]
    fn parse_int_range_both_bounds() {
        let mut reader = StringReader::new("10..50", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(10),
                max: Some(50)
            }
        );
    }

    #[test]
    fn parse_int_range_open_max() {
        let mut reader = StringReader::new("5..", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(5),
                max: None
            }
        );
    }

    #[test]
    fn parse_int_range_open_min() {
        let mut reader = StringReader::new("..100", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: None,
                max: Some(100)
            }
        );
    }

    #[test]
    fn parse_int_range_exact() {
        let mut reader = StringReader::new("42", 0);
        let result = parse_argument(&mut reader, &ArgumentType::IntRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::IntRange {
                min: Some(42),
                max: Some(42)
            }
        );
    }

    #[test]
    fn parse_float_range_both_bounds() {
        let mut reader = StringReader::new("1.5..3.5", 0);
        let result = parse_argument(&mut reader, &ArgumentType::FloatRange).unwrap();
        assert_eq!(
            result,
            ArgumentResult::FloatRange {
                min: Some(1.5),
                max: Some(3.5)
            }
        );
    }

    #[test]
    fn parse_rotation_absolute() {
        let mut reader = StringReader::new("0 180", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Rotation).unwrap();
        match result {
            ArgumentResult::Vec3(x, _, z) => {
                assert!((x - 0.0).abs() < f64::EPSILON);
                assert!((z - 180.0).abs() < f64::EPSILON);
            },
            ArgumentResult::Coordinates(_) => {},
            _ => panic!("Expected Vec3 or Coordinates for rotation"),
        }
    }

    #[test]
    fn parse_rotation_relative() {
        let mut reader = StringReader::new("~10 ~0", 0);
        let result = parse_argument(&mut reader, &ArgumentType::Rotation).unwrap();
        match result {
            ArgumentResult::Coordinates(coords) => {
                assert!(coords.x.is_relative);
                assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
                assert!(coords.z.is_relative);
                assert!((coords.z.value - 0.0).abs() < f64::EPSILON);
            },
            _ => panic!("Expected Coordinates for relative rotation"),
        }
    }
}
