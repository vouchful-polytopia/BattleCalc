use poise::CreateReply;
use serenity::all::CreateEmbed;

use super::{display, parse_battle_input};
use crate::combat;
use crate::unit::{Unit, UnitType};

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Resolve "kn" threshold: max defender HP at which a full-HP knight can one-shot.
fn knight_threshold(defender: &Unit) -> Option<f32> {
    let knight = Unit::new(UnitType::Knight);
    let hp = combat::eliminate_attacker(&knight, defender);
    if hp > 0.0 { Some(hp) } else { None }
}

/// Find minimum attackers to bring defender(s) to a target HP or below.
#[poise::command(slash_command)]
pub async fn threshold(
    ctx: Context<'_>,
    #[description = "Target HP or 'kn' for knight one-shot threshold"]
    target_str: String,
    #[description = "e.g. warrior, archer, rider vs giant"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let (attackers, defenders) = match parse_battle_input(&input) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("Error: {e}")).await?;
            return Ok(());
        }
    };

    let is_kn = matches!(target_str.to_lowercase().as_str(), "kn" | "knight");

    // Build per-defender targets
    let (targets, target_label) = if is_kn {
        let mut per_def_targets = Vec::new();
        let mut any_needs_work = false;
        let mut label_parts = Vec::new();

        for def in &defenders {
            match knight_threshold(def) {
                Some(hp) => {
                    per_def_targets.push(hp);
                    if hp < def.current_hp {
                        any_needs_work = true;
                    }
                    label_parts.push(format!("{}: ≤{}hp", def.display_name(), hp as i32));
                }
                None => {
                    ctx.say(format!(
                        "Error: A Knight cannot one-shot {} at any HP.",
                        def.display_name(),
                    )).await?;
                    return Ok(());
                }
            }
        }

        if !any_needs_work {
            ctx.say("All defenders can already be one-shot by a Knight at their current HP.").await?;
            return Ok(());
        }

        let label = if defenders.len() == 1 {
            format!("≤{}hp (Knight one-shot)", per_def_targets[0] as i32)
        } else {
            format!("Knight one-shot ({})", label_parts.join(", "))
        };

        (per_def_targets, label)
    } else {
        match target_str.parse::<f32>() {
            Ok(t) => {
                if t <= 0.0 {
                    ctx.say("Error: Target HP must be > 0. To find kill combos, use `/optim`.").await?;
                    return Ok(());
                }
                let targets = vec![t; defenders.len()];
                (targets, format!("≤{}hp", t as i32))
            }
            Err(_) => {
                ctx.say("Error: Target must be a number or `kn` (knight one-shot threshold).").await?;
                return Ok(());
            }
        }
    };

    // Validate
    if !is_kn {
        let all_already_met = defenders.iter().enumerate()
            .all(|(i, d)| targets[i] >= d.current_hp);
        if defenders.len() == 1 && all_already_met {
            ctx.say(format!(
                "Error: Target {}hp is already >= {}'s {}hp",
                targets[0] as i32,
                defenders[0].display_name(),
                defenders[0].current_hp as i32,
            )).await?;
            return Ok(());
        }
        if defenders.len() > 1 && all_already_met {
            ctx.say("All defenders are already at or below the target HP.").await?;
            return Ok(());
        }
    }

    ctx.defer().await?;
    let author_id = ctx.author().id.get();

    let compute_result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::task::spawn_blocking({
            let attackers = attackers.clone();
            let defenders = defenders.clone();
            let targets = targets.clone();
            move || {
                if defenders.len() == 1 {
                    ThresholdResult::Single(combat::threshold(&attackers, &defenders[0], targets[0]))
                } else {
                    ThresholdResult::Multi(combat::threshold_multi(&attackers, &defenders, &targets))
                }
            }
        }),
    )
    .await;

    let death_msg = ctx.data().config.get_random_death_message().await;

    match compute_result {
        Ok(Ok(ThresholdResult::Single(Some(result)))) => {
            let unique = {
                let mut seen = std::collections::HashSet::new();
                for &idx in &result.order { seen.insert(idx); }
                seen.len()
            };

            let embed = CreateEmbed::new()
                .color(0x2b7a78)
                .title(format!("Threshold: {} {}", defenders[0].display_name(), target_label))
                .field(
                    format!("Best combo ({} attacker{})", unique, if unique == 1 { "" } else { "s" }),
                    result.events.iter().map(|e| display::atk_line(e, &death_msg)).collect::<Vec<_>>().join("\n"),
                    false,
                )
                .field(
                    defenders[0].display_name(),
                    format!(
                        "{} -> {}hp (target: {})",
                        display::hp(defenders[0].current_hp),
                        display::hp(result.defender_hp_final),
                        target_label,
                    ),
                    false,
                );
            ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
        }
        Ok(Ok(ThresholdResult::Single(None))) => {
            ctx.say(format!(
                "No combination of those attackers can bring {} to {}",
                defenders[0].display_name(),
                target_label,
            )).await?;
        }
        Ok(Ok(ThresholdResult::Multi(Some((results, subset_indices))))) => {
            let unique = subset_indices.len();

            let mut embed = CreateEmbed::new()
                .color(0x2b7a78)
                .title(format!("Threshold (Multi-target): {}", target_label));

            let subset_names: Vec<String> = subset_indices.iter()
                .map(|&i| attackers[i].display_name())
                .collect();
            embed = embed.field(
                format!("Best combo ({} attacker{})", unique, if unique == 1 { "" } else { "s" }),
                subset_names.join(", "),
                false,
            );

            for (i, (result, def)) in results.iter().zip(defenders.iter()).enumerate() {
                let lines: Vec<String> = result.events.iter().map(|e| display::atk_line(e, &death_msg)).collect();
                let target_note = if is_kn {
                    format!(" (kn: ≤{}hp)", targets[i] as i32)
                } else {
                    String::new()
                };
                let field_name = format!(
                    "#{}: {} ({}) -> {}hp{}",
                    i + 1,
                    result.defender_name,
                    display::hp(def.current_hp),
                    display::hp(result.defender_hp_final),
                    target_note,
                );
                embed = embed.field(
                    field_name,
                    if lines.is_empty() { "Already at threshold".into() } else { lines.join("\n") },
                    false,
                );
            }

            ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
        }
        Ok(Ok(ThresholdResult::Multi(None))) => {
            ctx.say("No combination of those attackers can bring all defenders to threshold").await?;
        }
        Ok(Err(e)) => {
            ctx.say(format!("Error: {e}")).await?;
        }
        Err(_) => {
            ctx.say("The computation was too large — try entering a simpler input.").await?;
        }
    }

    Ok(())
}

enum ThresholdResult {
    Single(Option<combat::MultiCombatResult>),
    Multi(Option<(Vec<combat::MultiCombatResult>, Vec<usize>)>),
}
