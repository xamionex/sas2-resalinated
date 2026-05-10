#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MagicSlotOverrides {
    #[serde(default = "default_damage")]
    pub damage: f32,
}

fn default_damage() -> f32 { 0.0 }

impl Default for MagicSlotOverrides {
    fn default() -> Self {
        Self { damage: default_damage() }
    }
}