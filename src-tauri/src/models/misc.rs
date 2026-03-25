use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadResult {
    pub settings: Settings,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EncounterPreview {
    pub id: i32,
    pub fight_start: i64,
    pub boss_name: String,
    pub duration: i64,
    pub classes: Vec<i32>,
    pub names: Vec<String>,
    pub difficulty: Option<String>,
    pub local_player: String,
    pub my_dps: i64,
    pub favorite: bool,
    pub cleared: bool,
    pub spec: Option<String>,
    pub support_ap: Option<f32>,
    pub support_brand: Option<f32>,
    pub support_identity: Option<f32>,
    pub support_hyper: Option<f32>,
    pub udps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wipe_bars: Option<i32>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EncountersOverview {
    pub encounters: Vec<EncounterPreview>,
    pub total_encounters: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilter {
    pub bosses: Vec<String>,
    pub min_duration: i32,
    pub max_duration: i32,
    pub cleared: bool,
    pub favorite: bool,
    pub difficulty: String,
    pub boss_only_damage: bool,
    pub sort: String,
    pub order: String,
    pub raids_only: bool,
    pub local_player: String,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    pub name: String,
    pub max_gear_score: f32,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressionPlayerStats {
    pub name: String,
    pub class_id: i32,
    pub dps: i64,
    pub is_dead: bool,
    pub support_ap: Option<f32>,
    pub support_brand: Option<f32>,
    pub support_identity: Option<f32>,
    pub support_hyper: Option<f32>,
    pub unbuffed_dps: Option<i64>,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressionEncounterStats {
    pub id: i32,
    pub total_dps: i64,
    pub players: Vec<ProgressionPlayerStats>,
    /// party index -> list of player names (from encounter misc)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party_info: Option<HashMap<i32, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phases: Option<Vec<crate::models::BossPhase>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wipe_phase: Option<u32>,
    /// Boss HP remaining (0.0–1.0) at the end of the last phase. Use with wipe_phase to compare progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wipe_phase_end_hp: Option<f32>,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterDbInfo {
    pub size: String,
    pub total_encounters: i32,
    pub total_encounters_filtered: i32,
}

#[derive(Debug, Clone)]
pub struct CastEvent {
    pub timestamp: i64,
    pub cooldown_duration_ms: i64,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InspectInfo {
    pub combat_power: Option<CombatPower>,
    pub ark_passive_enabled: bool,
    pub ark_passive_data: Option<ArkPassiveData>,
    pub engravings: Option<Vec<u32>>,
    pub gems: Option<Vec<GemData>>,
    pub loadout_snapshot: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkPassiveData {
    pub evolution: Option<Vec<ArkPassiveNode>>,
    pub enlightenment: Option<Vec<ArkPassiveNode>>,
    pub leap: Option<Vec<ArkPassiveNode>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArkPassiveNode {
    pub id: u32,
    pub lv: u8,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CombatPower {
    // 1 for dps, 2 for support
    pub id: u32,
    pub score: f32,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GemData {
    pub tier: u8,
    pub skill_id: u32,
    pub gem_type: u8,
    pub value: u32,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Engraving {
    pub id: u32,
    pub level: u8,
}

pub struct SupportBuffs {
    pub brand: f64,
    pub buff: f64,
    pub identity: f64,
    pub hyper: f64,
}
