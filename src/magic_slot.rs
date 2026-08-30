use sas2_parser::loot_catalog::LootField;
use std::collections::HashMap;

/// Per-weapon, per-magic-slot overrides. All values are multipliers (1.0 = vanilla). The loader
/// treats a value of 1.0 (or <= 0) as "no change", so unset fields are harmless.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MagicSlotOverrides {
    #[serde(default = "default_mul")]
    pub damage: f32,
    /// MP / Rage cost multiplier (< 1 cheaper, > 1 costlier).
    #[serde(default = "default_mul")]
    pub cost: f32,
    /// Cooldown multiplier (< 1 shorter, > 1 longer).
    #[serde(default = "default_mul")]
    pub cooldown: f32,
}

/// Copied magic state (slot fields + per-slot multipliers) for the Paste magic button.
#[derive(Clone, Default)]
pub struct MagicClipboard {
    /// Magic slot fields (ids 14/15/16) copied from the source item.
    pub fields: Vec<LootField>,
    /// Per-slot multipliers (damage/cost/cooldown) copied from the source weapon.
    pub overrides: HashMap<i32, MagicSlotOverrides>,
    /// Display name of the item the magic was copied from (for UI feedback).
    pub source: Option<String>,
}

/// Copied flags for the Paste flags button.
#[derive(Clone, Default)]
pub struct FlagsClipboard {
    /// Flag indices copied from the source item.
    pub flags: Vec<i32>,
    /// Display name of the item the flags were copied from (for UI feedback).
    pub source: Option<String>,
}

/// Copied monster drops for the Paste drops button.
/// Drops are the monster fields 45-59 (five tiers of Type/Prob/Count).
#[derive(Clone, Default)]
pub struct DropsClipboard {
    /// Field ids 45..=59 with their values, as copied from the source monster.
    pub fields: Vec<(i32, sas2_parser::monster_catalog::MonsterFieldValue)>,
    /// Display name of the monster the drops were copied from (for UI feedback).
    pub source: Option<String>,
}

fn default_mul() -> f32 {
    1.0
}

impl Default for MagicSlotOverrides {
    fn default() -> Self {
        Self {
            damage: default_mul(),
            cost: default_mul(),
            cooldown: default_mul(),
        }
    }
}
