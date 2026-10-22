use poise::CreateReply;
use serenity::all::CreateEmbed;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Show credits for the bot
#[poise::command(slash_command)]
pub async fn credits(ctx: Context<'_>) -> Result<(), crate::Error> {
    let embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title("BattleCalc Credits")
        .field("Creator", "Vouchful", false)
        .field("Testing & Feature Suggestions", "PeacefulBerry, CrashGames, AtomicShuriken, OOOOOTS", false)
        .field("Prior Infrastructure", "Tafini", false);

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
