use hashbrown::HashMap;

use crate::database::utils::get_boss_hp_bars;
use crate::models::encounter::{BossHpLog, BossPhase, PhaseType};

const ENCORE_LOW_THRESHOLD: f32 = 0.01;
const ENCORE_HIGH_THRESHOLD: f32 = 0.10;
const INTERMISSION_GAP_SECONDS: i32 = 3;

/// Detect boss phases from boss HP log data.
///
/// Handles three types of phase transitions:
/// - Transformation: boss name changes (multiple keys in boss_hp_log)
/// - Encore: same boss HP drops near 0 then jumps back up
/// - Intermission: time gaps between boss activities
pub fn detect_phases(
    boss_hp_log: &HashMap<String, Vec<BossHpLog>>,
    _intermission_start: Option<i64>,
    _intermission_end: Option<i64>,
) -> Vec<BossPhase> {
    if boss_hp_log.is_empty() {
        return Vec::new();
    }

    // Sort boss entries by their first timestamp
    let mut boss_entries: Vec<(&String, &Vec<BossHpLog>)> = boss_hp_log
        .iter()
        .filter(|(_, log)| !log.is_empty())
        .collect();
    boss_entries.sort_by_key(|(_, log)| log[0].time);

    let is_multi_boss = boss_entries.len() > 1;
    let mut all_phases: Vec<BossPhase> = Vec::new();

    for (boss_name, log) in &boss_entries {
        let phase_type = if is_multi_boss && !all_phases.is_empty() {
            PhaseType::Transformation
        } else {
            PhaseType::Normal
        };

        // Check for intermission gap before this boss
        if let Some(last_phase) = all_phases.last() {
            if last_phase.phase_type != PhaseType::Intermission {
                let gap = log[0].time - last_phase.end_time;
                if gap > INTERMISSION_GAP_SECONDS {
                    all_phases.push(BossPhase {
                        phase_number: 0, // numbered later
                        phase_type: PhaseType::Intermission,
                        boss_name: String::new(),
                        start_time: last_phase.end_time,
                        end_time: log[0].time,
                        start_hp_percent: 0.0,
                        end_hp_percent: 0.0,
                        total_bars: None,
                    });
                }
            }
        }

        // Split this boss's log into sub-phases (encore detection)
        let sub_phases = detect_encore_phases(boss_name, log, phase_type);
        all_phases.extend(sub_phases);
    }

    // Number non-intermission phases sequentially
    let mut phase_num = 1u32;
    for phase in &mut all_phases {
        if phase.phase_type != PhaseType::Intermission {
            phase.phase_number = phase_num;
            phase_num += 1;
        }
    }

    all_phases
}

/// Split a single boss's HP log into sub-phases if encore resets are detected.
fn detect_encore_phases(
    boss_name: &str,
    log: &[BossHpLog],
    initial_phase_type: PhaseType,
) -> Vec<BossPhase> {
    if log.is_empty() {
        return Vec::new();
    }

    let max_hp = log.iter().map(|e| e.hp).max().unwrap_or(1);
    let total_bars = get_boss_hp_bars(boss_name, max_hp);

    let mut phases = Vec::new();
    let mut phase_start_idx = 0;
    let mut saw_low = false;
    let mut current_type = initial_phase_type;

    for i in 1..log.len() {
        let prev = &log[i - 1];
        let curr = &log[i];

        if !saw_low && prev.p < ENCORE_LOW_THRESHOLD && prev.p >= 0.0 {
            saw_low = true;
        }

        if saw_low && curr.p > ENCORE_HIGH_THRESHOLD {
            // Encore reset detected — end current phase at the low point
            let phase_log = &log[phase_start_idx..i];
            if !phase_log.is_empty() {
                phases.push(BossPhase {
                    phase_number: 0,
                    phase_type: current_type.clone(),
                    boss_name: boss_name.to_string(),
                    start_time: phase_log[0].time,
                    end_time: phase_log[phase_log.len() - 1].time,
                    start_hp_percent: phase_log[0].p,
                    end_hp_percent: phase_log[phase_log.len() - 1].p,
                    total_bars,
                });
            }

            // Start new encore phase
            phase_start_idx = i;
            saw_low = false;
            current_type = PhaseType::Encore;
        }
    }

    // Final phase from last split point to end
    let phase_log = &log[phase_start_idx..];
    if !phase_log.is_empty() {
        phases.push(BossPhase {
            phase_number: 0,
            phase_type: current_type,
            boss_name: boss_name.to_string(),
            start_time: phase_log[0].time,
            end_time: phase_log[phase_log.len() - 1].time,
            start_hp_percent: phase_log[0].p,
            end_hp_percent: phase_log[phase_log.len() - 1].p,
            total_bars,
        });
    }

    phases
}

/// Compute a normalized progress score (0.0 = no progress, 1.0 = clear).
///
/// For multi-phase fights, completed phases contribute their full weight,
/// and the final phase contributes proportionally to HP depleted.
pub fn compute_progress_score(phases: &[BossPhase], cleared: bool) -> f64 {
    if cleared {
        return 1.0;
    }

    let combat_phases: Vec<&BossPhase> = phases
        .iter()
        .filter(|p| p.phase_type != PhaseType::Intermission)
        .collect();

    if combat_phases.is_empty() {
        return 0.0;
    }

    if combat_phases.len() == 1 {
        let p = combat_phases[0];
        return (1.0 - p.end_hp_percent as f64).clamp(0.0, 1.0);
    }

    // Weight each phase by its bar count (or 1.0 if unknown)
    let weights: Vec<f64> = combat_phases
        .iter()
        .map(|p| p.total_bars.unwrap_or(100) as f64)
        .collect();
    let total_weight: f64 = weights.iter().sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    let mut score = 0.0;
    for (i, phase) in combat_phases.iter().enumerate() {
        let phase_weight = weights[i] / total_weight;
        let hp_depleted = if phase.start_hp_percent > 0.0 {
            ((phase.start_hp_percent - phase.end_hp_percent) / phase.start_hp_percent) as f64
        } else {
            0.0
        };

        // A completed phase is one where end HP is near 0
        if i < combat_phases.len() - 1 {
            // Not the last phase — it was completed (otherwise we wouldn't have moved on)
            score += phase_weight;
        } else {
            // Last phase — partial credit
            score += phase_weight * hp_depleted.clamp(0.0, 1.0);
        }
    }

    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log(entries: Vec<(i32, f32)>) -> Vec<BossHpLog> {
        entries
            .into_iter()
            .map(|(time, p)| BossHpLog {
                time,
                hp: (p * 1_000_000.0) as i64,
                p,
            })
            .collect()
    }

    #[test]
    fn single_boss_single_phase() {
        let mut boss_hp_log = HashMap::new();
        boss_hp_log.insert(
            "Boss A".to_string(),
            make_log(vec![(0, 1.0), (10, 0.8), (20, 0.5), (30, 0.3)]),
        );

        let phases = detect_phases(&boss_hp_log, None, None);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].phase_number, 1);
        assert_eq!(phases[0].phase_type, PhaseType::Normal);
        assert_eq!(phases[0].boss_name, "Boss A");
        assert!((phases[0].start_hp_percent - 1.0).abs() < 0.01);
        assert!((phases[0].end_hp_percent - 0.3).abs() < 0.01);
    }

    #[test]
    fn multi_boss_transformation() {
        let mut boss_hp_log = HashMap::new();
        boss_hp_log.insert(
            "Boss A".to_string(),
            make_log(vec![(0, 1.0), (10, 0.5), (20, 0.0)]),
        );
        boss_hp_log.insert(
            "Boss B".to_string(),
            make_log(vec![(25, 1.0), (35, 0.7), (45, 0.4)]),
        );

        let phases = detect_phases(&boss_hp_log, None, None);
        // Boss A phase + intermission gap (20->25 = 5s) + Boss B phase
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0].phase_type, PhaseType::Normal);
        assert_eq!(phases[0].boss_name, "Boss A");
        assert_eq!(phases[0].phase_number, 1);
        assert_eq!(phases[1].phase_type, PhaseType::Intermission);
        assert_eq!(phases[2].phase_type, PhaseType::Transformation);
        assert_eq!(phases[2].boss_name, "Boss B");
        assert_eq!(phases[2].phase_number, 2);
    }

    #[test]
    fn encore_detection() {
        let mut boss_hp_log = HashMap::new();
        boss_hp_log.insert(
            "Boss A".to_string(),
            make_log(vec![
                (0, 1.0),
                (10, 0.5),
                (20, 0.1),
                (25, 0.005), // below encore low threshold
                (26, 0.5),   // jumps back above encore high threshold
                (36, 0.3),
                (46, 0.1),
            ]),
        );

        let phases = detect_phases(&boss_hp_log, None, None);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].phase_type, PhaseType::Normal);
        assert_eq!(phases[0].phase_number, 1);
        assert_eq!(phases[1].phase_type, PhaseType::Encore);
        assert_eq!(phases[1].phase_number, 2);
    }

    #[test]
    fn progress_score_single_phase() {
        let phases = vec![BossPhase {
            phase_number: 1,
            phase_type: PhaseType::Normal,
            boss_name: "Boss A".to_string(),
            start_time: 0,
            end_time: 30,
            start_hp_percent: 1.0,
            end_hp_percent: 0.3,
            total_bars: Some(100),
        }];

        let score = compute_progress_score(&phases, false);
        assert!((score - 0.7).abs() < 0.01);
    }

    #[test]
    fn progress_score_cleared() {
        let phases = vec![BossPhase {
            phase_number: 1,
            phase_type: PhaseType::Normal,
            boss_name: "Boss A".to_string(),
            start_time: 0,
            end_time: 30,
            start_hp_percent: 1.0,
            end_hp_percent: 0.0,
            total_bars: Some(100),
        }];

        let score = compute_progress_score(&phases, true);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn progress_score_multi_phase() {
        let phases = vec![
            BossPhase {
                phase_number: 1,
                phase_type: PhaseType::Normal,
                boss_name: "Boss A".to_string(),
                start_time: 0,
                end_time: 20,
                start_hp_percent: 1.0,
                end_hp_percent: 0.0,
                total_bars: Some(100),
            },
            BossPhase {
                phase_number: 2,
                phase_type: PhaseType::Transformation,
                boss_name: "Boss B".to_string(),
                start_time: 25,
                end_time: 45,
                start_hp_percent: 1.0,
                end_hp_percent: 0.5,
                total_bars: Some(100),
            },
        ];

        let score = compute_progress_score(&phases, false);
        // Phase 1: 100/200 weight, completed = 0.5
        // Phase 2: 100/200 weight, 50% depleted = 0.25
        // Total: 0.75
        assert!((score - 0.75).abs() < 0.01);
    }

    #[test]
    fn empty_log() {
        let boss_hp_log = HashMap::new();
        let phases = detect_phases(&boss_hp_log, None, None);
        assert!(phases.is_empty());

        let score = compute_progress_score(&phases, false);
        assert!((score - 0.0).abs() < 0.001);
    }
}
