use poise::CreateReply;

use crate::combat;
use crate::unit::Unit;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Optimal attack order with range constraints (tag attackers a/b/c for reachable defenders)
#[poise::command(slash_command)]
pub async fn assign(
    ctx: Context<'_>,
    #[description = "e.g. ri ab, ar a, wa b vs warrior, defender d"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let result = match parse_assign_input(&input) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("Error: {e}")).await?;
            return Ok(());
        }
    };

    let AssignInput { attackers, defenders, reach_masks } = result;

    if defenders.len() > 3 {
        ctx.say("Error: Max 3 defenders for /assign").await?;
        return Ok(());
    }

    if attackers.len() > 20 {
        ctx.say("Error: Max 20 attackers").await?;
        return Ok(());
    }

    let use_greedy = attackers.len() > 10;

    ctx.defer().await?;

    let reach_masks_fallback = reach_masks.clone();

    let compute_result = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tokio::task::spawn_blocking({
            let attackers = attackers.clone();
            let defenders = defenders.clone();
            move || {
                if use_greedy {
                    combat::greedy_optimized_multi_defender_constrained(&attackers, &defenders, &reach_masks)
                } else {
                    combat::optimized_multi_defender_constrained(&attackers, &defenders, &reach_masks)
                }
            }
        }),
    )
    .await;

    let death_msg = ctx.data().config.get_random_death_message().await;
    let author_id = ctx.author().id.get();

    match compute_result {
        Ok(Ok(results)) => {
            let title = if use_greedy {
                "Optimal Order — Range-Constrained (greedy — may not be exact)"
            } else {
                "Optimal Order (Range-Constrained)"
            };
            let embed = super::display::multi_result_embed(&results, &defenders, title, &death_msg);
            let mut reply = CreateReply::default().embed(embed).components(vec![super::display::delete_row(author_id)]);
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
                move || combat::greedy_optimized_multi_defender_constrained(&attackers, &defenders, &reach_masks_fallback)
            })
            .await;

            match fallback {
                Ok(results) => {
                    let embed = super::display::multi_result_embed(&results, &defenders, "Optimal Order — Range-Constrained (greedy — exact solve timed out)", &death_msg);
                    ctx.send(CreateReply::default()
                        .content("⚠ Exact solve timed out — falling back to greedy approximation.")
                        .embed(embed)
                        .components(vec![super::display::delete_row(author_id)])
                    ).await?;
                }
                Err(e) => {
                    ctx.say(format!("Error: {e}")).await?;
                }
            }
        }
    }

    Ok(())
}

struct AssignInput {
    attackers: Vec<Unit>,
    defenders: Vec<Unit>,
    /// Bitmask per attacker: bit 0 = can reach defender A, bit 1 = B, bit 2 = C
    reach_masks: Vec<u32>,
}

/// Parse input like "ri ab, ar a, wa b vs warrior, defender d"
/// Defenders are labeled A, B, C by position.
/// Attacker tags are lowercase letters after all modifiers.
fn parse_assign_input(input: &str) -> Result<AssignInput, String> {
    // Split on " vs "
    let lower = input.to_lowercase();
    let vs_pos = lower
        .find(" vs ")
        .ok_or("Use `vs` to separate attackers from defenders: `ri ab, wa a vs warrior, defender`")?;
    let atk_str = &input[..vs_pos];
    let def_str = &input[vs_pos + 4..];

    // Parse defenders (standard CSV)
    let def_parts: Vec<&str> = def_str.split(',').collect();
    let mut defenders = Vec::new();
    for part in &def_parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        defenders.push(super::parse_unit(trimmed)?);
    }
    if defenders.is_empty() {
        return Err("No defenders specified after `vs`".into());
    }
    if defenders.len() > 3 {
        return Err("Max 3 defenders (A, B, C)".into());
    }

    let num_defs = defenders.len();

    // Parse attackers with range tags
    let atk_parts: Vec<&str> = atk_str.split(',').collect();
    let mut attackers = Vec::new();
    let mut reach_masks = Vec::new();

    for part in &atk_parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (unit, mask) = parse_unit_with_tags(trimmed, num_defs)?;
        attackers.push(unit);
        reach_masks.push(mask);
    }

    if attackers.is_empty() {
        return Err("No attackers specified before `vs`".into());
    }

    // Label duplicates
    label_duplicates(&mut attackers);

    Ok(AssignInput {
        attackers,
        defenders,
        reach_masks,
    })
}

/// Parse a single unit spec that may end with range tags (a/b/c letters).
/// Returns (Unit, reach_bitmask). If no tags, mask = all defenders reachable.
fn parse_unit_with_tags(input: &str, num_defs: usize) -> Result<(Unit, u32), String> {
    let tokens: Vec<&str> = input.trim().split_whitespace().collect();
    if tokens.is_empty() {
        return Err("Empty unit specification".into());
    }

    // Check if the last token is a range tag (only letters a-c, no other meaning)
    let (unit_tokens, mask) = if let Some(last) = tokens.last() {
        let lower = last.to_lowercase();
        if is_range_tag(&lower, num_defs) && tokens.len() > 1 {
            let mask = tag_to_mask(&lower);
            (&tokens[..tokens.len() - 1], mask)
        } else {
            // No tag — can reach all defenders
            (&tokens[..], (1u32 << num_defs) - 1)
        }
    } else {
        (&tokens[..], (1u32 << num_defs) - 1)
    };

    let unit_str = unit_tokens.join(" ");
    let unit = super::parse_unit(&unit_str)?;
    Ok((unit, mask))
}

/// Check if a token is purely a range tag (only contains a, b, c)
fn is_range_tag(s: &str, num_defs: usize) -> bool {
    if s.is_empty() || num_defs == 1 {
        return false;
    }
    let max_letter = b'a' + (num_defs as u8) - 1;
    for ch in s.chars() {
        if ch < 'a' || ch as u8 > max_letter {
            return false;
        }
    }
    true
}

/// Convert range tag string to bitmask: 'a' = bit 0, 'b' = bit 1, 'c' = bit 2
fn tag_to_mask(s: &str) -> u32 {
    let mut mask = 0u32;
    for ch in s.chars() {
        let idx = (ch as u8) - b'a';
        mask |= 1 << idx;
    }
    mask
}

fn label_duplicates(units: &mut [Unit]) {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for u in units.iter() {
        *counts.entry(u.display_name()).or_insert(0) += 1;
    }
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for u in units.iter_mut() {
        let name = u.display_name();
        if counts[&name] > 1 {
            let idx = seen.entry(name).or_insert(0);
            *idx += 1;
            u.label = Some(idx.to_string());
        }
    }
}
