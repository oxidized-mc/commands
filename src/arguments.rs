//! Argument types for the Brigadier command graph.
//!
//! Each variant maps to a vanilla parser identifier and serializes to the
//! wire format used by `ClientboundCommandsPacket`.

use bytes::BufMut;

/// Describes how a `brigadier:string` argument is parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    /// Reads a single word (no spaces).
    SingleWord = 0,
    /// Reads a quoted phrase or a single word.
    QuotablePhrase = 1,
    /// Reads the rest of the input.
    GreedyPhrase = 2,
}

/// All argument types supported by the command system.
///
/// Each variant knows its Brigadier parser identifier and how to
/// serialize its properties to the wire format.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum ArgumentType {
    // ── Brigadier built-ins ──
    /// `brigadier:bool`
    Bool,
    /// `brigadier:integer` with optional min/max.
    Integer { min: Option<i32>, max: Option<i32> },
    /// `brigadier:float` with optional min/max.
    Float { min: Option<f32>, max: Option<f32> },
    /// `brigadier:double` with optional min/max.
    Double { min: Option<f64>, max: Option<f64> },
    /// `brigadier:long` with optional min/max.
    Long { min: Option<i64>, max: Option<i64> },
    /// `brigadier:string` with word/quotable/greedy mode.
    String(StringKind),

    // ── Minecraft argument types ──
    /// `minecraft:entity` — single or multiple, player-only flag.
    Entity { single: bool, player_only: bool },
    /// `minecraft:game_profile`
    GameProfile,
    /// `minecraft:block_pos`
    BlockPos,
    /// `minecraft:column_pos`
    ColumnPos,
    /// `minecraft:vec3`
    Vec3,
    /// `minecraft:vec2`
    Vec2,
    /// `minecraft:block_state`
    BlockState,
    /// `minecraft:block_predicate`
    BlockPredicate,
    /// `minecraft:item_stack`
    ItemStack,
    /// `minecraft:item_predicate`
    ItemPredicate,
    /// `minecraft:color`
    Color,
    /// `minecraft:hex_color`
    HexColor,
    /// `minecraft:component`
    Component,
    /// `minecraft:style`
    McStyle,
    /// `minecraft:message`
    Message,
    /// `minecraft:nbt_compound_tag`
    NbtCompoundTag,
    /// `minecraft:nbt_tag`
    NbtTag,
    /// `minecraft:nbt_path`
    NbtPath,
    /// `minecraft:objective`
    Objective,
    /// `minecraft:objective_criteria`
    ObjectiveCriteria,
    /// `minecraft:operation`
    Operation,
    /// `minecraft:particle`
    Particle,
    /// `minecraft:angle`
    Angle,
    /// `minecraft:rotation`
    Rotation,
    /// `minecraft:scoreboard_slot`
    ScoreboardSlot,
    /// `minecraft:score_holder` — allow multiple flag.
    ScoreHolder { allow_multiple: bool },
    /// `minecraft:swizzle`
    Swizzle,
    /// `minecraft:team`
    Team,
    /// `minecraft:item_slot`
    ItemSlot,
    /// `minecraft:item_slots`
    ItemSlots,
    /// `minecraft:resource_location`
    ResourceLocation,
    /// `minecraft:function`
    Function,
    /// `minecraft:entity_anchor`
    EntityAnchor,
    /// `minecraft:int_range`
    IntRange,
    /// `minecraft:float_range`
    FloatRange,
    /// `minecraft:dimension`
    Dimension,
    /// `minecraft:gamemode`
    Gamemode,
    /// `minecraft:time` with minimum ticks.
    Time { min: i32 },
    /// `minecraft:resource_or_tag` with registry key.
    ResourceOrTag { registry: String },
    /// `minecraft:resource_or_tag_key` with registry key.
    ResourceOrTagKey { registry: String },
    /// `minecraft:resource` with registry key.
    Resource { registry: String },
    /// `minecraft:resource_key` with registry key.
    ResourceKey { registry: String },
    /// `minecraft:resource_selector` with registry key.
    ResourceSelector { registry: String },
    /// `minecraft:template_mirror`
    TemplateMirror,
    /// `minecraft:template_rotation`
    TemplateRotation,
    /// `minecraft:heightmap`
    Heightmap,
    /// `minecraft:loot_table`
    LootTable,
    /// `minecraft:loot_predicate`
    LootPredicate,
    /// `minecraft:loot_modifier`
    LootModifier,
    /// `minecraft:dialog`
    Dialog,
    /// `minecraft:uuid`
    Uuid,
}

impl ArgumentType {
    /// Returns the registry ID used in `ClientboundCommandsPacket`.
    ///
    /// These IDs match the registration order in
    /// `ArgumentTypeInfos.bootstrap()`.
    pub fn registry_id(&self) -> i32 {
        match self {
            Self::Bool => 0,
            Self::Float { .. } => 1,
            Self::Double { .. } => 2,
            Self::Integer { .. } => 3,
            Self::Long { .. } => 4,
            Self::String(_) => 5,
            Self::Entity { .. } => 6,
            Self::GameProfile => 7,
            Self::BlockPos => 8,
            Self::ColumnPos => 9,
            Self::Vec3 => 10,
            Self::Vec2 => 11,
            Self::BlockState => 12,
            Self::BlockPredicate => 13,
            Self::ItemStack => 14,
            Self::ItemPredicate => 15,
            Self::Color => 16,
            Self::HexColor => 17,
            Self::Component => 18,
            Self::McStyle => 19,
            Self::Message => 20,
            Self::NbtCompoundTag => 21,
            Self::NbtTag => 22,
            Self::NbtPath => 23,
            Self::Objective => 24,
            Self::ObjectiveCriteria => 25,
            Self::Operation => 26,
            Self::Particle => 27,
            Self::Angle => 28,
            Self::Rotation => 29,
            Self::ScoreboardSlot => 30,
            Self::ScoreHolder { .. } => 31,
            Self::Swizzle => 32,
            Self::Team => 33,
            Self::ItemSlot => 34,
            Self::ItemSlots => 35,
            Self::ResourceLocation => 36,
            Self::Function => 37,
            Self::EntityAnchor => 38,
            Self::IntRange => 39,
            Self::FloatRange => 40,
            Self::Dimension => 41,
            Self::Gamemode => 42,
            Self::Time { .. } => 43,
            Self::ResourceOrTag { .. } => 44,
            Self::ResourceOrTagKey { .. } => 45,
            Self::Resource { .. } => 46,
            Self::ResourceKey { .. } => 47,
            Self::ResourceSelector { .. } => 48,
            Self::TemplateMirror => 49,
            Self::TemplateRotation => 50,
            Self::Heightmap => 51,
            Self::LootTable => 52,
            Self::LootPredicate => 53,
            Self::LootModifier => 54,
            Self::Dialog => 55,
            Self::Uuid => 56,
        }
    }

    /// Serializes parser-specific properties to the wire format.
    pub fn write_properties(&self, buf: &mut impl BufMut) {
        match self {
            Self::Float { min, max } => {
                let flags =
                    (if min.is_some() { 0x01 } else { 0 }) | (if max.is_some() { 0x02 } else { 0 });
                buf.put_u8(flags);
                if let Some(v) = min {
                    buf.put_f32(*v);
                }
                if let Some(v) = max {
                    buf.put_f32(*v);
                }
            },
            Self::Double { min, max } => {
                let flags =
                    (if min.is_some() { 0x01 } else { 0 }) | (if max.is_some() { 0x02 } else { 0 });
                buf.put_u8(flags);
                if let Some(v) = min {
                    buf.put_f64(*v);
                }
                if let Some(v) = max {
                    buf.put_f64(*v);
                }
            },
            Self::Integer { min, max } => {
                let flags =
                    (if min.is_some() { 0x01 } else { 0 }) | (if max.is_some() { 0x02 } else { 0 });
                buf.put_u8(flags);
                if let Some(v) = min {
                    buf.put_i32(*v);
                }
                if let Some(v) = max {
                    buf.put_i32(*v);
                }
            },
            Self::Long { min, max } => {
                let flags =
                    (if min.is_some() { 0x01 } else { 0 }) | (if max.is_some() { 0x02 } else { 0 });
                buf.put_u8(flags);
                if let Some(v) = min {
                    buf.put_i64(*v);
                }
                if let Some(v) = max {
                    buf.put_i64(*v);
                }
            },
            Self::String(kind) => {
                write_varint(*kind as i32, buf);
            },
            Self::Entity {
                single,
                player_only,
            } => {
                let flags =
                    (if *single { 0x01 } else { 0 }) | (if *player_only { 0x02 } else { 0 });
                buf.put_u8(flags);
            },
            Self::ScoreHolder { allow_multiple } => {
                buf.put_u8(if *allow_multiple { 0x01 } else { 0x00 });
            },
            Self::Time { min } => {
                buf.put_i32(*min);
            },
            Self::ResourceOrTag { registry } | Self::ResourceOrTagKey { registry } => {
                write_string(registry, buf);
            },
            Self::Resource { registry }
            | Self::ResourceKey { registry }
            | Self::ResourceSelector { registry } => {
                write_string(registry, buf);
            },
            // All other argument types have no additional properties.
            _ => {},
        }
    }
}

/// Writes a VarInt to a buffer.
fn write_varint(mut value: i32, buf: &mut impl BufMut) {
    loop {
        if (value & !0x7F) == 0 {
            buf.put_u8(value as u8);
            return;
        }
        buf.put_u8(((value & 0x7F) | 0x80) as u8);
        value = ((value as u32) >> 7) as i32;
    }
}

/// Writes a length-prefixed UTF-8 string.
fn write_string(s: &str, buf: &mut impl BufMut) {
    write_varint(s.len() as i32, buf);
    buf.put_slice(s.as_bytes());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn registry_ids_match_vanilla() {
        assert_eq!(ArgumentType::Bool.registry_id(), 0);
        assert_eq!(
            ArgumentType::Integer {
                min: None,
                max: None
            }
            .registry_id(),
            3
        );
        assert_eq!(ArgumentType::BlockPos.registry_id(), 8);
        assert_eq!(ArgumentType::Gamemode.registry_id(), 42);
        assert_eq!(ArgumentType::Uuid.registry_id(), 56);
    }

    #[test]
    fn integer_properties_no_bounds() {
        let mut buf = Vec::new();
        ArgumentType::Integer {
            min: None,
            max: None,
        }
        .write_properties(&mut buf);
        assert_eq!(buf, vec![0x00]); // flags = 0 (no min, no max)
    }

    #[test]
    fn integer_properties_with_bounds() {
        let mut buf = Vec::new();
        ArgumentType::Integer {
            min: Some(1),
            max: Some(100),
        }
        .write_properties(&mut buf);
        assert_eq!(buf[0], 0x03); // flags = 0x01 | 0x02
        // min = 1 (big-endian i32)
        assert_eq!(&buf[1..5], &1i32.to_be_bytes());
        // max = 100 (big-endian i32)
        assert_eq!(&buf[5..9], &100i32.to_be_bytes());
    }

    #[test]
    fn entity_properties() {
        let mut buf = Vec::new();
        ArgumentType::Entity {
            single: true,
            player_only: false,
        }
        .write_properties(&mut buf);
        assert_eq!(buf, vec![0x01]); // single = 0x01, player_only = 0

        buf.clear();
        ArgumentType::Entity {
            single: false,
            player_only: true,
        }
        .write_properties(&mut buf);
        assert_eq!(buf, vec![0x02]); // single = 0, player_only = 0x02
    }

    #[test]
    fn string_kind_properties() {
        let mut buf = Vec::new();
        ArgumentType::String(StringKind::SingleWord).write_properties(&mut buf);
        assert_eq!(buf, vec![0x00]); // VarInt 0

        buf.clear();
        ArgumentType::String(StringKind::QuotablePhrase).write_properties(&mut buf);
        assert_eq!(buf, vec![0x01]); // VarInt 1

        buf.clear();
        ArgumentType::String(StringKind::GreedyPhrase).write_properties(&mut buf);
        assert_eq!(buf, vec![0x02]); // VarInt 2
    }

    #[test]
    fn bool_has_no_properties() {
        let mut buf = Vec::new();
        ArgumentType::Bool.write_properties(&mut buf);
        assert!(buf.is_empty());
    }
}
