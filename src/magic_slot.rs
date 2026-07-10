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
