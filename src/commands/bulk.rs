use poise::CreateReply;
use serenity::all::CreateEmbed;

use super::{display, parse_battle_input};
use crate::combat;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// How many hits from one unit type to kill another
#[poise::command(slash_command)]
pub async fn bulk(
    ctx: Context<'_>,
    #[description = "e.g. warrior vs giant w"]
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
        ctx.say("❌ Bulk needs exactly 1 attacker and 1 defender.\nExample: `!b warrior vs giant w`").await?;
        return Ok(());
    }

    let attacker = &attackers[0];
    let defender = &defenders[0];

    match combat::bulk(attacker, defender) {
        Some(count) => {
            let embed = CreateEmbed::new()
                .color(0x2b7a78)
                .title(format!(
                    "Hits to kill {} with {}:",
                    defender.display_name(),
                    attacker.display_name(),
                ))
                .field("Count", format!("**{count}**"), false);
            let author_id = ctx.author().id.get();
            ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
        }
        None => {
            ctx.say(format!(
                "❌ {} can't deal damage to {}",
                attacker.display_name(),
                defender.display_name(),
            ))
            .await?;
        }
    }

    Ok(())
}
