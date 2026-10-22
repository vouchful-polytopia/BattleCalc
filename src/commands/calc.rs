use poise::CreateReply;

use super::{display, parse_battle_input};
use crate::combat;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Simulate combat between units. Use "vs" to separate attackers from defenders.
#[poise::command(slash_command)]
pub async fn calc(
    ctx: Context<'_>,
    #[description = "e.g. warrior, rider vs defender d"]
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

    let valid_attackers: Vec<_> = attackers
        .iter()
        .filter(|a| a.attack > 0.0 || a.status_effects.contains(crate::unit::StatusEffects::EXPLODING))
        .collect();

    if valid_attackers.is_empty() {
        ctx.say("Error: No attackers with attack > 0").await?;
        return Ok(());
    }

    let death_msg = ctx.data().config.get_random_death_message().await;
    let author_id = ctx.author().id.get();

    let components = display::calc_row(author_id, &input, "v120");

    if defenders.len() == 1 {
        let order: Vec<usize> = (0..attackers.len()).collect();
        let result = combat::multi_combat(&attackers, &defenders[0], &order);
        let embed = display::single_result_embed(&result, &defenders[0], "Combat Result", &death_msg);
        ctx.send(CreateReply::default().embed(embed).components(components)).await?;
    } else {
        let results = combat::sequential_multi_defender(&attackers, &defenders);
        let embed = display::multi_result_embed(&results, &defenders, "Sequential Combat", &death_msg);
        ctx.send(CreateReply::default().embed(embed).components(components)).await?;
    }

    Ok(())
}
