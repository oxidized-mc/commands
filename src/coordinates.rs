//! Coordinate parsing for command arguments.
//!
//! Supports three forms matching vanilla Minecraft:
//! - **Absolute**: `100 64 -200`
//! - **Relative**: `~10 ~ ~-5` (offset from source position)
//! - **Local**: `^1 ^0 ^2` (left/up/forward relative to facing)
//!
//! The `^` form cannot be mixed with `~` or absolute coordinates.

use crate::CommandError;
use crate::string_reader::StringReader;

/// A single coordinate component that may be absolute, relative (`~`), or
/// local (`^`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCoordinate {
    /// The numeric value (offset if relative/local, absolute otherwise).
    pub value: f64,
    /// Whether this coordinate is relative to the source position.
    pub is_relative: bool,
}

/// The kind of coordinate system used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateKind {
    /// World coordinates — each axis is absolute or `~` relative.
    World,
    /// Local coordinates — all axes use `^` (relative to facing direction).
    Local,
}

/// Three-component coordinates parsed from command input.
///
/// Vanilla supports two forms:
/// - **World coordinates**: `100 64 -200`, `~10 ~ ~-5` (absolute/relative per axis)
/// - **Local coordinates**: `^1 ^0 ^2` (left/up/forwards relative to facing)
///
/// The two forms cannot be mixed (all `^` or none `^`).
#[derive(Debug, Clone, PartialEq)]
pub struct Coordinates {
    /// X component (or "left" in local mode).
    pub x: WorldCoordinate,
    /// Y component (or "up" in local mode).
    pub y: WorldCoordinate,
    /// Z component (or "forwards" in local mode).
    pub z: WorldCoordinate,
    /// Whether these are world or local coordinates.
    pub kind: CoordinateKind,
}

impl Coordinates {
    /// Resolves these coordinates to absolute (x, y, z) using the given
    /// source position and rotation.
    ///
    /// For world coordinates, relative axes (`~`) add to the source position.
    /// For local coordinates (`^`), the offsets are rotated by the source's
    /// yaw and pitch.
    pub fn resolve(&self, position: (f64, f64, f64), rotation: (f32, f32)) -> (f64, f64, f64) {
        match self.kind {
            CoordinateKind::World => {
                let resolve_axis = |coord: &WorldCoordinate, base: f64| -> f64 {
                    if coord.is_relative {
                        base + coord.value
                    } else {
                        coord.value
                    }
                };
                (
                    resolve_axis(&self.x, position.0),
                    resolve_axis(&self.y, position.1),
                    resolve_axis(&self.z, position.2),
                )
            },
            CoordinateKind::Local => {
                let (yaw, pitch) = rotation;
                let yaw_rad = (yaw as f64).to_radians();
                let pitch_rad = (pitch as f64).to_radians();

                let (sin_yaw, cos_yaw) = yaw_rad.sin_cos();
                let (sin_pitch, cos_pitch) = pitch_rad.sin_cos();

                // Vanilla's local coordinate system:
                //   left  = x component (perpendicular to facing, horizontal)
                //   up    = y component (perpendicular to facing, vertical plane)
                //   fwd   = z component (in facing direction)
                let left = self.x.value;
                let up = self.y.value;
                let fwd = self.z.value;

                // Forward vector (from yaw/pitch)
                let fwd_x = -sin_yaw * cos_pitch;
                let fwd_y = -sin_pitch;
                let fwd_z = cos_yaw * cos_pitch;

                // Up vector (perpendicular to forward in vertical plane)
                let up_x = -sin_yaw * (-sin_pitch);
                let up_y = cos_pitch;
                let up_z = cos_yaw * (-sin_pitch);

                // Left vector (cross product of up and forward, simplified)
                let left_x = cos_yaw;
                let left_y = 0.0;
                let left_z = sin_yaw;

                let x = position.0 + left * left_x + up * up_x + fwd * fwd_x;
                let y = position.1 + left * left_y + up * up_y + fwd * fwd_y;
                let z = position.2 + left * left_z + up * up_z + fwd * fwd_z;
                (x, y, z)
            },
        }
    }

    /// Resolves to integer block position (floors after resolving).
    pub fn resolve_block_pos(
        &self,
        position: (f64, f64, f64),
        rotation: (f32, f32),
    ) -> (i32, i32, i32) {
        let (x, y, z) = self.resolve(position, rotation);
        (x.floor() as i32, y.floor() as i32, z.floor() as i32)
    }

    /// Returns `true` if any component is relative or local.
    pub fn has_relative(&self) -> bool {
        self.kind == CoordinateKind::Local
            || self.x.is_relative
            || self.y.is_relative
            || self.z.is_relative
    }
}

/// Entity anchor points for `/tp facing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityAnchorKind {
    /// At the entity's feet.
    Feet,
    /// At the entity's eyes.
    Eyes,
}

// ── Parsing ─────────────────────────────────────────────────────────

/// Parses a single coordinate component. Returns `(coordinate, is_local)`.
fn parse_single_coord(
    reader: &mut StringReader<'_>,
    read_value: impl FnOnce(&mut StringReader<'_>) -> Result<f64, CommandError>,
) -> Result<(WorldCoordinate, bool), CommandError> {
    let remaining = reader.remaining();
    let (is_local, _) = match remaining.as_bytes().first() {
        Some(b'^') => (true, true),
        Some(b'~') => (false, true),
        _ => {
            let value = read_value(reader)?;
            return Ok((
                WorldCoordinate {
                    value,
                    is_relative: false,
                },
                false,
            ));
        },
    };

    reader.advance(1); // skip ^ or ~
    let value = if reader.can_read() && reader.peek() != Some(' ') {
        read_value(reader)?
    } else {
        0.0
    };
    Ok((
        WorldCoordinate {
            value,
            is_relative: true,
        },
        is_local,
    ))
}

/// Validates that all axes use the same coordinate system and returns the kind.
fn validate_coordinate_mix(locals: &[bool]) -> Result<CoordinateKind, CommandError> {
    if locals.iter().any(|&l| l != locals[0]) {
        return Err(CommandError::Parse(
            "Cannot mix world and local coordinates (^ and ~)".to_string(),
        ));
    }
    Ok(if locals[0] {
        CoordinateKind::Local
    } else {
        CoordinateKind::World
    })
}

/// Parses three whitespace-separated double coordinates supporting `~`/`^`.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if a coordinate component cannot be parsed
/// or if relative (`~`) and local (`^`) coordinates are mixed.
pub fn parse_coordinates3(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let read_double = |r: &mut StringReader<'_>| r.read_double();
    let (x, x_local) = parse_single_coord(reader, read_double)?;
    reader.skip_whitespace();
    let (y, y_local) = parse_single_coord(reader, |r| r.read_double())?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_coord(reader, |r| r.read_double())?;
    let kind = validate_coordinate_mix(&[x_local, y_local, z_local])?;
    Ok(Coordinates { x, y, z, kind })
}

/// Parses three whitespace-separated integer coordinates supporting `~`/`^`.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if a coordinate component cannot be parsed
/// or if relative (`~`) and local (`^`) coordinates are mixed.
pub fn parse_int_coordinates3(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let read_int = |r: &mut StringReader<'_>| Ok(r.read_integer()? as f64);
    let (x, x_local) = parse_single_coord(reader, read_int)?;
    reader.skip_whitespace();
    let (y, y_local) = parse_single_coord(reader, |r| Ok(r.read_integer()? as f64))?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_coord(reader, |r| Ok(r.read_integer()? as f64))?;
    let kind = validate_coordinate_mix(&[x_local, y_local, z_local])?;
    Ok(Coordinates { x, y, z, kind })
}

/// Parses two whitespace-separated coordinates (x z) for Vec2/Rotation.
///
/// # Errors
///
/// Returns [`CommandError::Parse`] if a coordinate component cannot be parsed
/// or if relative (`~`) and local (`^`) coordinates are mixed.
pub fn parse_coordinates2(reader: &mut StringReader<'_>) -> Result<Coordinates, CommandError> {
    let (x, x_local) = parse_single_coord(reader, |r| r.read_double())?;
    reader.skip_whitespace();
    let (z, z_local) = parse_single_coord(reader, |r| r.read_double())?;
    let kind = validate_coordinate_mix(&[x_local, z_local])?;
    Ok(Coordinates {
        x,
        y: WorldCoordinate {
            value: 0.0,
            is_relative: false,
        },
        z,
        kind,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_absolute() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 100.0,
                is_relative: false,
            },
            y: WorldCoordinate {
                value: 64.0,
                is_relative: false,
            },
            z: WorldCoordinate {
                value: -200.0,
                is_relative: false,
            },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((0.0, 0.0, 0.0), (0.0, 0.0));
        assert!((x - 100.0).abs() < f64::EPSILON);
        assert!((y - 64.0).abs() < f64::EPSILON);
        assert!((z - -200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resolve_relative() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 10.0,
                is_relative: true,
            },
            y: WorldCoordinate {
                value: 0.0,
                is_relative: true,
            },
            z: WorldCoordinate {
                value: -5.0,
                is_relative: true,
            },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((50.0, 100.0, 200.0), (0.0, 0.0));
        assert!((x - 60.0).abs() < f64::EPSILON);
        assert!((y - 100.0).abs() < f64::EPSILON);
        assert!((z - 195.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resolve_mixed() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 100.0,
                is_relative: false,
            },
            y: WorldCoordinate {
                value: 5.0,
                is_relative: true,
            },
            z: WorldCoordinate {
                value: -200.0,
                is_relative: false,
            },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve((50.0, 60.0, 200.0), (0.0, 0.0));
        assert!((x - 100.0).abs() < f64::EPSILON);
        assert!((y - 65.0).abs() < f64::EPSILON);
        assert!((z - -200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resolve_block_pos_floors() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 10.7,
                is_relative: false,
            },
            y: WorldCoordinate {
                value: -0.3,
                is_relative: false,
            },
            z: WorldCoordinate {
                value: 5.9,
                is_relative: false,
            },
            kind: CoordinateKind::World,
        };
        let (x, y, z) = coords.resolve_block_pos((0.0, 0.0, 0.0), (0.0, 0.0));
        assert_eq!((x, y, z), (10, -1, 5));
    }

    #[test]
    fn test_has_relative_false() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 1.0,
                is_relative: false,
            },
            y: WorldCoordinate {
                value: 2.0,
                is_relative: false,
            },
            z: WorldCoordinate {
                value: 3.0,
                is_relative: false,
            },
            kind: CoordinateKind::World,
        };
        assert!(!coords.has_relative());
    }

    #[test]
    fn test_has_relative_true() {
        let coords = Coordinates {
            x: WorldCoordinate {
                value: 1.0,
                is_relative: true,
            },
            y: WorldCoordinate {
                value: 2.0,
                is_relative: false,
            },
            z: WorldCoordinate {
                value: 3.0,
                is_relative: false,
            },
            kind: CoordinateKind::World,
        };
        assert!(coords.has_relative());
    }

    #[test]
    fn test_parse_coordinates3_tilde() {
        let mut reader = StringReader::new("~10 ~ ~-5", 0);
        let coords = parse_coordinates3(&mut reader).unwrap();
        assert_eq!(coords.kind, CoordinateKind::World);
        assert!(coords.x.is_relative);
        assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
        assert!(coords.y.is_relative);
        assert!((coords.y.value).abs() < f64::EPSILON);
        assert!(coords.z.is_relative);
        assert!((coords.z.value - -5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_coordinates3_caret() {
        let mut reader = StringReader::new("^1 ^0 ^2", 0);
        let coords = parse_coordinates3(&mut reader).unwrap();
        assert_eq!(coords.kind, CoordinateKind::Local);
        assert!((coords.x.value - 1.0).abs() < f64::EPSILON);
        assert!((coords.y.value).abs() < f64::EPSILON);
        assert!((coords.z.value - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_coordinates3_mixed_rejected() {
        let mut reader = StringReader::new("~1 ^0 ~2", 0);
        assert!(parse_coordinates3(&mut reader).is_err());
    }

    #[test]
    fn test_parse_coordinates3_bare_tilde() {
        let mut reader = StringReader::new("~ ~ ~", 0);
        let coords = parse_coordinates3(&mut reader).unwrap();
        assert!(coords.x.is_relative);
        assert!(coords.y.is_relative);
        assert!(coords.z.is_relative);
    }

    #[test]
    fn test_parse_int_coordinates3_relative() {
        let mut reader = StringReader::new("~5 ~0 ~-3", 0);
        let coords = parse_int_coordinates3(&mut reader).unwrap();
        assert!(coords.x.is_relative);
        assert!((coords.x.value - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_coordinates2_relative() {
        let mut reader = StringReader::new("~10 ~0", 0);
        let coords = parse_coordinates2(&mut reader).unwrap();
        assert!(coords.x.is_relative);
        assert!((coords.x.value - 10.0).abs() < f64::EPSILON);
    }
}
