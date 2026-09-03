/// Talisman (charm) boost metadata.
///
/// Every charm flag has a fixed vanilla magnitude baked into the game's stat formulas (e.g. Damage = 10%, Max HP = 5%, Phys Def = 10 flat).
/// The editor edits those actual values; the loader converts them back to the scalar `GetCharmVal` factor by dividing by the vanilla magnitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CharmBoostUnit {
    /// Shown/edited as a percentage (e.g. 10 = 10%).
    Percent,
    /// Shown/edited as a flat number.
    Flat,
}

/// A talisman boost configuration: the boost value rolls uniformly between `min` and `max`; when `static_boost` is enabled the value is fixed instead.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CharmBoostRange {
    #[serde(default)]
    pub min: f32,
    #[serde(default)]
    pub max: f32,
    #[serde(default)]
    pub static_boost: bool,
    #[serde(default)]
    pub static_value: f32,
}

impl Default for CharmBoostRange {
    fn default() -> Self {
        Self {
            min: 1.0,
            max: 1.0,
            static_boost: false,
            static_value: 1.0,
        }
    }
}

impl CharmBoostRange {
    /// A range seeded with the flag's vanilla magnitude.
    pub fn vanilla(flag: i32) -> Self {
        let v = charm_boost_vanilla(flag);
        Self {
            min: v,
            max: v,
            static_boost: false,
            static_value: v,
        }
    }

    /// Whether the config differs from the vanilla value.
    pub fn is_modified(&self, flag: i32) -> bool {
        let v = charm_boost_vanilla(flag);
        (self.min - v).abs() > 0.0001
            || (self.max - v).abs() > 0.0001
            || self.static_boost
            || (self.static_value - v).abs() > 0.0001
    }

    /// The effective value this boost contributes, given the configured range.
    pub fn effective(&self) -> f32 {
        if self.static_boost {
            self.static_value
        } else if self.max > self.min {
            (self.min + self.max) * 0.5
        } else {
            self.min
        }
    }
}

/// Vanilla magnitude + display unit for a charm flag.
/// Flags without a known formula (multiplayer, runes, etc.) default to 1.0 flat.
pub fn charm_boost_vanilla(flag: i32) -> f32 {
    match flag {
        0 => 10.0,      // Phys Def: 20 * v * 0.5
        1 => 20.0,      // Fire Def: 20 * v
        2 => 20.0,      // Cold Def
        3 => 20.0,      // Poison Def
        4 => 20.0,      // Light Def
        5 => 20.0,      // Dark Def
        6 => 10.0,      // Item Find: v * 10 (+ v * 0.2 drop rate)
        7 => 0.15,      // Rage Gain: v * 0.15 per sec
        8 => 1.0,       // Rage Window: +v seconds
        9 => 1.0,       // Wood Runes (no magnitude formula)
        10 => 1.0,      // Poise (no magnitude formula)
        11 => 2.0,      // Fast grapple/climb: 15 - v * 2 stamina
        12 => 10.0,     // Stamina Regen: v * 10
        13 => 50.0,     // Silver Find: v * 0.5 = 50%
        14 => 10.0,     // Damage: v * 0.1 = 10%
        15 => 5.0,      // Gold: v * 0.05 dmg = 5% (also v*2.5 def, v*2 stamina)
        16 => 20.0,     // Fire Atk: v * 0.2 = 20%
        17 => 20.0,     // Cold Atk
        18 => 20.0,     // Poison Atk
        19 => 20.0,     // Light Atk
        20 => 20.0,     // Dark Atk
        21..=28 => 1.0, // Multiplayer flags (boolean)
        29 => 5.0,      // Carry Weight: v * 5
        30 => 5.0,      // HP Kill Gain: maxHP * 0.05 * v = 5%
        31 => 5.0,      // MP Kill Gain: 5%
        32 => 50.0,     // Parry Stagger Damage: poiseAtk * v * 0.5 = 50%
        33 => 25.0,     // MP Regain: 1 + v * 0.25 = 25%
        34 => 50.0,     // Riposte Dmg: 1 + v * 0.5 = 50%
        35 => 50.0,     // Dying Boost: v * 0.5 = 50%
        36 => 5.0,      // Max HP Boost: 1 + 0.05 * v = 5%
        37 => 5.0,      // Max Rage Boost: 5%
        38 => 10.0,     // Max MP Boost: v * 10
        39 => 5.0,      // Max Stamina Boost: 5%
        40 => 2.5,      // MP Parry regain: maxMP * 0.025 * v = 2.5%
        41 => 2.5,      // HP Parry regain: 2.5%
        42 => 50.0,     // MP Riposte regain: maxMP * 0.5 * v = 50%
        43 => 50.0,     // HP Riposte regain: 50%
        44 => 50.0,     // Restock speed: 1 + v * 0.5 = 50%
        45 => 12.5,     // Rage Parry regain: maxRage * 0.125 * v = 12.5%
        46 => 12.5,     // Rage Riposte regain: 12.5%
        47 => 1.0,      // Stamina coverage (complex formula)
        48 => 10.0,     // Blocking stamina cheap: v * 0.1 = 10%
        49 => 15.0,     // Runic art boost: 1 + v * 0.15 = 15%
        50 => 50.0,     // Faster Drinking: 1 + v * 0.5 = 50%
        51 => 3.1,      // Overall defense: v * 3.1
        52 => 10.0,     // Haze HP: maxHP * 0.1 * v = 10%
        53 => 10.0,     // Haze MP: 10%
        54 => 3.0,      // Haze Rage: v * 3
        _ => 1.0,
    }
}

/// Display unit for a charm flag.
pub fn charm_boost_unit(flag: i32) -> CharmBoostUnit {
    match flag {
        13..=20 | 30..=37 | 39..=46 | 48..=50 | 52..=53 => CharmBoostUnit::Percent,
        _ => CharmBoostUnit::Flat,
    }
}
