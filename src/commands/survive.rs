use poise::CreateReply;
use serenity::all::CreateEmbed;

use super::{display, parse_battle_input};
use crate::combat;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Check if your units can clear a siege and your unit survives.
#[poise::command(slash_command)]
pub async fn survive(
    ctx: Context<'_>,
    #[description = "e.g. ri, ri 3, ri 5 vs ri, ri d, warrior (last = survive target)"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let (attackers, all_defenders) = match parse_battle_input(&input) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("Error: {e}")).await?;
            return Ok(());
        }
    };

    if all_defenders.len() < 2 {
        ctx.say("Error: Need at least 2 defenders — blockers first, survive target last.\nExample: `/survive ri, ri, ri vs ri, ri d, warrior`").await?;
        return Ok(());
    }

    if attackers.len() > 20 {
        ctx.say("Error: Max 20 attackers").await?;
        return Ok(());
    }

    ctx.defer().await?;

    let blockers: Vec<crate::unit::Unit> = all_defenders[..all_defenders.len() - 1].to_vec();
    let survive_target = all_defenders[all_defenders.len() - 1].clone();

    let compute_result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::task::spawn_blocking({
            let attackers = attackers.clone();
            let blockers = blockers.clone();
            let survive_target = survive_target.clone();
            move || compute_survive(&attackers, &blockers, &survive_target)
        }),
    )
    .await;

    let (result, was_greedy_fallback) = match compute_result {
        Ok(Ok(r)) => (r, false),
        Ok(Err(e)) => {
            ctx.say(format!("Error: {e}")).await?;
            return Ok(());
        }
        Err(_) => {
            // Exact solve timed out — fall back to greedy
            let fallback = tokio::task::spawn_blocking({
                let attackers = attackers.clone();
                let blockers = blockers.clone();
                let survive_target = survive_target.clone();
                move || compute_survive_greedy(&attackers, &blockers, &survive_target)
            })
            .await;
            match fallback {
                Ok(r) => (r, true),
                Err(e) => {
                    ctx.say(format!("Error: {e}")).await?;
                    return Ok(());
                }
            }
        }
    };

    let blockers_cleared = result.phase1_results.iter().all(|r| r.defender_hp_final <= 0.0);
    let target_killed = result.target_final_hp <= 0.0;

    let (status, color) = match (blockers_cleared, target_killed) {
        (true, true) => ("Blockers cleared + target killed", 0xe74c3c),
        (true, false) => ("Blockers cleared — target survives", 0x27ae60),
        (false, true) => ("Blockers NOT fully cleared — target killed", 0xe74c3c),
        (false, false) => ("Blockers NOT fully cleared — target survives", 0xf39c12),
    };

    let title = if was_greedy_fallback {
        "Survive Check (greedy — exact solve timed out)"
    } else {
        "Survive Check"
    };

    let death_msg = ctx.data().config.get_random_death_message().await;

    let mut embed = CreateEmbed::new()
        .color(color)
        .title(title)
        .description(status);

    // Blocker results
    for (i, (phase1_result, blocker)) in result.phase1_results.iter().zip(blockers.iter()).enumerate() {
        let lines: Vec<String> = phase1_result.events.iter().map(|e| display::atk_line(e, &death_msg)).collect();
        let field_name = format!(
            "Blocker #{}: {} ({}) -> {}",
            i + 1,
            phase1_result.defender_name,
            display::hp(blocker.current_hp),
            display::def_final(phase1_result.defender_hp_final, &death_msg),
        );
        embed = embed.field(
            field_name,
            if lines.is_empty() { "No attackers assigned".into() } else { lines.join("\n") },
            false,
        );
    }

    // Target result
    if let Some(ref p2) = result.phase2_result {
        if !p2.events.is_empty() {
            let lines: Vec<String> = p2.events.iter().map(|e| display::atk_line(e, &death_msg)).collect();
            embed = embed.field(
                format!(
                    "Remaining vs {} ({})",
                    survive_target.display_name(),
                    display::hp(survive_target.current_hp),
                ),
                lines.join("\n"),
                false,
            );
        }
    }

    embed = embed.field(
        survive_target.display_name(),
        format!(
            "{} -> {}",
            display::hp(survive_target.current_hp),
            display::def_final(result.target_final_hp, &death_msg),
        ),
        false,
    );

    let author_id = ctx.author().id.get();
    ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
    Ok(())
}

struct SurviveResult {
    phase1_results: Vec<combat::MultiCombatResult>,
    phase2_result: Option<combat::MultiCombatResult>,
    target_final_hp: f32,
    score: i64,
}

fn compute_survive(
    attackers: &[crate::unit::Unit],
    blockers: &[crate::unit::Unit],
    survive_target: &crate::unit::Unit,
) -> SurviveResult {
    let n = attackers.len();
    let mut best: Option<SurviveResult> = None;

    for mask in 1..(1u32 << n) {
        let phase1_attackers: Vec<crate::unit::Unit> = (0..n)
            .filter(|&i| mask & (1 << i) != 0)
            .map(|i| attackers[i].clone())
            .collect();

        let phase1_results = combat::optimized_multi_defender(&phase1_attackers, blockers);

        // Check all blockers killed
        if !phase1_results.iter().all(|r| r.defender_hp_final <= 0.0) {
            continue;
        }

        // Build remaining attackers: ONLY unused ones (not in Phase 1 mask)
        let mut remaining: Vec<crate::unit::Unit> = Vec::new();
        for i in 0..n {
            if mask & (1 << i) == 0 {
                remaining.push(attackers[i].clone());
            }
        }

        // Phase 2: remaining attackers vs survive target
        let phase2_result = if !remaining.is_empty() {
            let order: Vec<usize> = (0..remaining.len()).collect();
            Some(combat::multi_combat(&remaining, survive_target, &order))
        } else {
            None
        };

        let target_hp = phase2_result.as_ref()
            .map_or(survive_target.current_hp, |r| r.defender_hp_final);

        let score = survive_score(target_hp, &phase1_results, &phase2_result);

        let is_better = best.as_ref().map_or(true, |b| score > b.score);
        if is_better {
            best = Some(SurviveResult {
                phase1_results,
                phase2_result,
                target_final_hp: target_hp,
                score,
            });
        }
    }

    // Fallback: no subset clears all blockers
    if best.is_none() {
        let phase1_results = combat::optimized_multi_defender(attackers, blockers);
        let target_hp = survive_target.current_hp;
        best = Some(SurviveResult {
            phase1_results,
            phase2_result: None,
            target_final_hp: target_hp,
            score: i64::MIN,
        });
    }

    best.unwrap()
}

fn compute_survive_greedy(
    attackers: &[crate::unit::Unit],
    blockers: &[crate::unit::Unit],
    survive_target: &crate::unit::Unit,
) -> SurviveResult {
    let n = attackers.len();
    let mut best: Option<SurviveResult> = None;

    for mask in 1..(1u32 << n) {
        let phase1_attackers: Vec<crate::unit::Unit> = (0..n)
            .filter(|&i| mask & (1 << i) != 0)
            .map(|i| attackers[i].clone())
            .collect();

        let phase1_results = combat::greedy_optimized_multi_defender(&phase1_attackers, blockers);

        if !phase1_results.iter().all(|r| r.defender_hp_final <= 0.0) {
            continue;
        }

        let mut remaining: Vec<crate::unit::Unit> = Vec::new();
        for i in 0..n {
            if mask & (1 << i) == 0 {
                remaining.push(attackers[i].clone());
            }
        }

        let phase2_result = if !remaining.is_empty() {
            let order: Vec<usize> = (0..remaining.len()).collect();
            Some(combat::multi_combat(&remaining, survive_target, &order))
        } else {
            None
        };

        let target_hp = phase2_result.as_ref()
            .map_or(survive_target.current_hp, |r| r.defender_hp_final);

        let score = survive_score(target_hp, &phase1_results, &phase2_result);

        let is_better = best.as_ref().map_or(true, |b| score > b.score);
        if is_better {
            best = Some(SurviveResult {
                phase1_results,
                phase2_result,
                target_final_hp: target_hp,
                score,
            });
        }
    }

    if best.is_none() {
        let phase1_results = combat::greedy_optimized_multi_defender(attackers, blockers);
        let target_hp = survive_target.current_hp;
        best = Some(SurviveResult {
            phase1_results,
            phase2_result: None,
            target_final_hp: target_hp,
            score: i64::MIN,
        });
    }

    best.unwrap()
}

/// Score a survive outcome. Priority:
/// 1. Target killed (most important)
/// 2. Most damage to target (lowest remaining HP)
/// 3. Fewest total attacker casualties
fn survive_score(
    target_hp: f32,
    phase1: &[combat::MultiCombatResult],
    phase2: &Option<combat::MultiCombatResult>,
) -> i64 {
    let target_killed: i64 = if target_hp <= 0.0 { 1 } else { 0 };
    let target_remaining = (target_hp.max(0.0) * 10.0) as i64;
    let phase1_casualties: i64 = phase1.iter().map(|r| r.attacker_casualties as i64).sum();
    let phase2_casualties: i64 = phase2.as_ref().map_or(0, |r| r.attacker_casualties as i64);

    target_killed * 10_000_000_000
    - target_remaining * 1_000_000
    - (phase1_casualties + phase2_casualties) * 1_000
}
