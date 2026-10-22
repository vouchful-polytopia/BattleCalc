use poise::CreateReply;
use serenity::all::CreateEmbed;

use super::{display, parse_battle_input};
use crate::combat;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Find the minimum HP to one-shot kill, or max defender HP that dies to a one-shot
#[poise::command(slash_command)]
pub async fn elim(
    ctx: Context<'_>,
    #[description = "e.g. warrior vs defender d"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let (attackers, defenders) = match parse_battle_input(&input) {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("❌ {e}")).await?;
            return Ok(());
        }
    };

    if attackers.len() != 1 || defenders.len() != 1 {
        ctx.say("❌ Elim needs exactly 1 attacker and 1 defender.\nExample: `!e warrior vs defender d`").await?;
        return Ok(());
    }

    let attacker = &attackers[0];
    let defender = &defenders[0];

    let min_hp = combat::eliminate_defender(attacker, defender);
    let max_def = combat::eliminate_attacker(attacker, defender);

    let mut embed = CreateEmbed::new().color(0x2b7a78);

    match min_hp {
        Some(hp) => {
            embed = embed
                .title(format!(
                    "Min {} HP to kill {} ({}hp):",
                    attacker.display_name(),
                    defender.display_name(),
                    defender.current_hp as i32,
                ))
                .field("Min attacker HP", format!("**{}**", hp as i32), true);
        }
        None => {
            embed = embed.title(format!(
                "A full HP {} cannot one-shot {} ({}hp)",
                attacker.display_name(),
                defender.display_name(),
                defender.current_hp as i32,
            ));
        }
    }

    if max_def > 0.0 {
        embed = embed.field(
            format!("Max {} HP for one-shot", defender.display_name()),
            format!("**{}**", max_def as i32),
            true,
        );
    }

    let author_id = ctx.author().id.get();
    ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
    Ok(())
}
