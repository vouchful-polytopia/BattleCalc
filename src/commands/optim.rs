use poise::CreateReply;

use super::{display, parse_battle_input};
use crate::combat::{self, Target};

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Extract a `t12` / `t<12` target-HP token from the defender section of the input,
/// returning (cleaned_input, parsed_target). If no `vs` separator exists or no target
/// token is found, returns the input unchanged with None.
fn extract_target(input: &str) -> (String, Option<Target>) {
    let lower = input.to_lowercase();
    let Some(vs_pos) = lower.find(" vs ") else {
        return (input.to_string(), None);
    };
    let (prefix, def_section) = input.split_at(vs_pos + 4);

    let mut target: Option<Target> = None;
    // Rebuild def_section without the target token, preserving commas and whitespace.
    let mut cleaned = String::new();
    for part in def_section.split(',') {
        if !cleaned.is_empty() { cleaned.push(','); }
        let mut first_tok = true;
        for tok in part.split_whitespace() {
            let tok_lower = tok.to_lowercase();
            if target.is_none() {
                if let Some(rest) = tok_lower.strip_prefix('t') {
                    if let Some(n) = rest.strip_prefix('<') {
                        if let Ok(h) = n.parse::<f32>() {
                            target = Some(Target::Below(h));
                            continue;
                        }
                    } else if let Ok(h) = rest.parse::<f32>() {
                        target = Some(Target::Exact(h));
                        continue;
                    }
                }
            }
            if first_tok {
                if !part.starts_with(|c: char| c.is_whitespace()) && !cleaned.is_empty() {
                    // we just added a comma; no leading space needed unless original had one
                }
                cleaned.push(' ');
                first_tok = false;
            } else {
                cleaned.push(' ');
            }
            cleaned.push_str(tok);
        }
    }
    let cleaned = format!("{}{}", prefix, cleaned.trim_start());
    (cleaned, target)
}

/// Find the optimal attack order. Supports multiple defenders after "vs".
#[poise::command(slash_command)]
pub async fn optim(
    ctx: Context<'_>,
    #[description = "e.g. warrior, archer, rider vs giant, defender w (add t12 or t<12 for target HP)"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let (cleaned_input, target) = extract_target(&input);

    let (attackers, defenders) = match parse_battle_input(&cleaned_input) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("Error: {e}")).await?;
            return Ok(());
        }
    };

    if attackers.len() > 20 {
        ctx.say("Error: Max 20 attackers for optimization").await?;
        return Ok(());
    }

    if target.is_some() && defenders.len() > 1 {
        ctx.say("Error: Target HP (`t12` / `t<12`) only works with a single defender.").await?;
        return Ok(());
    }

    let use_greedy = attackers.len() > 10;

    if target.is_some() && use_greedy {
        ctx.say("Error: Target HP optimization is capped at 10 attackers.").await?;
        return Ok(());
    }

    ctx.defer().await?;

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::task::spawn_blocking({
            let attackers = attackers.clone();
            let defenders = defenders.clone();
            let target = target;
            move || {
                if let Some(target) = target {
                    // target implies single defender (guarded above) and n <= 10
                    match combat::optimized_with_target(&attackers, &defenders[0], target) {
                        Ok(r) => OptimResult::Single(r),
                        Err(e) => OptimResult::Error(e),
                    }
                } else if use_greedy {
                    if defenders.len() == 1 {
                        OptimResult::Single(combat::greedy_optimized(&attackers, &defenders[0]))
                    } else {
                        OptimResult::Multi(combat::greedy_optimized_multi_defender(&attackers, &defenders))
                    }
                } else if defenders.len() == 1 {
                    OptimResult::Single(combat::optimized(&attackers, &defenders[0]))
                } else {
                    OptimResult::Multi(combat::optimized_multi_defender(&attackers, &defenders))
                }
            }
        }),
    )
    .await;

    let death_msg = ctx.data().config.get_random_death_message().await;
    let author_id = ctx.author().id.get();

    match result {
        Ok(Ok(OptimResult::Error(e))) => {
            ctx.say(format!("Error: {e}")).await?;
        }
        Ok(Ok(OptimResult::Single(result))) => {
            let title = if target.is_some() {
                match target.unwrap() {
                    Target::Exact(h) => format!("Optimal Order — target: defender at exactly {} HP", h as i32),
                    Target::Below(h) => format!("Optimal Order — target: defender below {} HP", h as i32),
                }
            } else if use_greedy {
                "Optimal Attack Order (greedy — may not be exact)".to_string()
            } else {
                "Optimal Attack Order".to_string()
            };
            let embed = display::single_result_embed(&result, &defenders[0], &title, &death_msg);
            let mut reply = CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)]);
            if use_greedy {
                reply = reply.content("⚠ Too many units for exact solve — a greedy approximation was used instead.");
            }
            ctx.send(reply).await?;
        }
        Ok(Ok(OptimResult::Multi(results))) => {
            let title = if use_greedy {
                "Optimal Order — Multi-target (greedy — may not be exact)"
            } else {
                "Optimal Order (Multi-target)"
            };
            let embed = display::multi_result_embed(&results, &defenders, title, &death_msg);
            let mut reply = CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)]);
            if use_greedy {
                reply = reply.content("⚠ Too many units for exact solve — a greedy approximation was used instead.");
            }
            ctx.send(reply).await?;
        }
        Ok(Err(e)) => {
            ctx.say(format!("Error: {e}")).await?;
        }
        Err(_) => {
            // Exact solve timed out — fall back to greedy
            let fallback = tokio::task::spawn_blocking({
                let attackers = attackers.clone();
                let defenders = defenders.clone();
                move || {
                    if defenders.len() == 1 {
                        OptimResult::Single(combat::greedy_optimized(&attackers, &defenders[0]))
                    } else {
                        OptimResult::Multi(combat::greedy_optimized_multi_defender(&attackers, &defenders))
                    }
                }
            })
            .await;

            match fallback {
                Ok(OptimResult::Single(result)) => {
                    let embed = display::single_result_embed(&result, &defenders[0], "Optimal Attack Order (greedy — exact solve timed out)", &death_msg);
                    ctx.send(CreateReply::default()
                        .content("⚠ Exact solve timed out — falling back to greedy approximation.")
                        .embed(embed)
                        .components(vec![display::delete_row(author_id)])
                    ).await?;
                }
                Ok(OptimResult::Multi(results)) => {
                    let embed = display::multi_result_embed(&results, &defenders, "Optimal Order (greedy — exact solve timed out)", &death_msg);
                    ctx.send(CreateReply::default()
                        .content("⚠ Exact solve timed out — falling back to greedy approximation.")
                        .embed(embed)
                        .components(vec![display::delete_row(author_id)])
                    ).await?;
                }
                Ok(OptimResult::Error(msg)) => {
                    ctx.say(format!("Error: {msg}")).await?;
                }
                Err(e) => {
                    ctx.say(format!("Error: {e}")).await?;
                }
            }
        }
    }

    Ok(())
}

enum OptimResult {
    Single(combat::MultiCombatResult),
    Multi(Vec<combat::MultiCombatResult>),
    Error(String),
}
