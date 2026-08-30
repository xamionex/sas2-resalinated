/// Artifact (talisman subtype 3/4/5) boost metadata.
///
/// An equipped artifact contributes 35 percentage values (1.0 = 1%), consumed by the game's stat formulas via GetArtifactVal(field) * 0.01.
/// The values are normally rolled when the artifact is obtained, the editor overrides them with a fixed or rolled value between Min and Max.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArtifactBoostRange {
    #[serde(default)]
    pub min: f32,
    #[serde(default)]
    pub max: f32,
    #[serde(default)]
    pub static_boost: bool,
    #[serde(default)]
    pub static_value: f32,
}

impl Default for ArtifactBoostRange {
    fn default() -> Self {
        Self {
            min: 5.0,
            max: 20.0,
            static_boost: false,
            static_value: 5.0,
        }
    }
}

/// (field id, name, vanilla min, vanilla max, main stat?).
const ARTIFACT_FIELDS: &[(i32, &str, f32, f32, bool)] = &[
    // Attack (subtype 3)
    (0, "Attack Damage", 0.25, 20.0, true),
    (1, "Attack Speed", 5.0, 20.0, false),
    (2, "Attack Poise Dmg", 5.0, 20.0, false),
    (3, "Attack Stamina Reduction", 5.0, 20.0, false),
    (4, "Damage vs Mages", 5.0, 20.0, false),
    (5, "Damage vs Minions", 5.0, 20.0, false),
    (6, "Damage vs Undead", 5.0, 20.0, false),
    (7, "Damage vs Mobs", 5.0, 20.0, false),
    (8, "Damage vs Guardians", 5.0, 20.0, false),
    (9, "Damage vs Hazeburnt", 5.0, 20.0, false),
    (10, "Attack Rage Buildup", 5.0, 20.0, false),
    (11, "Attack Reach", 5.0, 20.0, false),
    (12, "Damage vs Players", 5.0, 20.0, false),
    // Defense (subtype 4)
    (13, "Add HP", 0.25, 50.0, true),
    (14, "Add MP", 5.0, 20.0, false),
    (15, "Add Stamina", 5.0, 20.0, false),
    (16, "Reduce Damage Received", 5.0, 20.0, false),
    (17, "Add Stamina Recover", 5.0, 20.0, false),
    (18, "Phys Defense", 5.0, 20.0, false),
    (19, "Fire Defense", 5.0, 20.0, false),
    (20, "Cold Defense", 5.0, 20.0, false),
    (21, "Poison Defense", 5.0, 20.0, false),
    (22, "Light Defense", 5.0, 20.0, false),
    (23, "Dark Defense", 5.0, 20.0, false),
    (24, "Poise Recovery", 5.0, 20.0, false),
    (25, "Poise", 5.0, 20.0, false),
    // Utility (subtype 5)
    (26, "Ranged Dmg", 0.25, 20.0, true),
    (27, "Free Ammo", 5.0, 60.0, false),
    (28, "Item Find", 5.0, 20.0, false),
    (29, "Silver Find", 5.0, 20.0, false),
    (30, "XP Find", 5.0, 20.0, false),
    (31, "Silver Save", 5.0, 50.0, false),
    (32, "XP Save", 5.0, 50.0, false),
    (33, "Alchemy Dmg", 5.0, 20.0, false),
    (34, "Runic Attack", 5.0, 20.0, false),
];

pub fn artifact_field_count() -> usize {
    ARTIFACT_FIELDS.len()
}

/// Field info by table index: (id, name, vanilla min, vanilla max, main stat?).
pub fn artifact_field_info(index: usize) -> (i32, &'static str, f32, f32, bool) {
    let (id, name, min, max, main) = ARTIFACT_FIELDS[index];
    (id, name, min, max, main)
}

impl ArtifactBoostRange {
    /// A range seeded with the field's vanilla roll bounds.
    pub fn vanilla(field: i32) -> Self {
        let (min, max) = ARTIFACT_FIELDS
            .iter()
            .find(|(id, _, _, _, _)| *id == field)
            .map(|(_, _, min, max, _)| (*min, *max))
            .unwrap_or((5.0, 20.0));
        Self {
            min,
            max,
            static_boost: false,
            static_value: min,
        }
    }

    /// Whether the config differs from the vanilla roll bounds.
    pub fn is_modified(&self, field: i32) -> bool {
        let v = Self::vanilla(field);
        (self.min - v.min).abs() > 0.0001
            || (self.max - v.max).abs() > 0.0001
            || self.static_boost
            || (self.static_value - v.min).abs() > 0.0001
    }

    /// The effective value this boost contributes (midpoint of the roll, or the static value).
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
