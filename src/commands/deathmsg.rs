use poise::CreateReply;
use serenity::all::CreateEmbed;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// View or manage custom death messages
#[poise::command(
    slash_command,
    subcommands("add", "remove", "list"),
    subcommand_required
)]
pub async fn deathmsg(_ctx: Context<'_>) -> Result<(), crate::Error> {
    Ok(())
}

/// Add a custom death message (admin only)
#[poise::command(slash_command)]
async fn add(
    ctx: Context<'_>,
    #[description = "Message to show when a unit dies (without brackets)"]
    message: String,
    #[description = "Who contributed this message"]
    credited_to: String,
) -> Result<(), crate::Error> {
    if ctx.author().id.get() != ctx.data().config.admin_user_id {
        ctx.send(
            CreateReply::default()
                .content("You are not authorized to use this command.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let msg = message.trim().to_string();
    let credit = credited_to.trim().to_string();
    if msg.is_empty() {
        ctx.send(
            CreateReply::default()
                .content("Message cannot be empty.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let was_new = ctx.data().config.add_death_message(msg.clone(), credit.clone()).await;

    let response = if was_new {
        format!("Added death message: [{}] — credited to **{}**", msg, credit)
    } else {
        format!("That message already exists: [{}]", msg)
    };

    ctx.send(CreateReply::default().content(response).ephemeral(true))
        .await?;
    Ok(())
}

/// Remove a custom death message (admin only)
#[poise::command(slash_command)]
async fn remove(
    ctx: Context<'_>,
    #[description = "Exact message text to remove"]
    #[rest]
    message: String,
) -> Result<(), crate::Error> {
    if ctx.author().id.get() != ctx.data().config.admin_user_id {
        ctx.send(
            CreateReply::default()
                .content("You are not authorized to use this command.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let msg = message.trim();
    let was_removed = ctx.data().config.remove_death_message(msg).await;

    let response = if was_removed {
        format!("Removed death message: [{}]", msg)
    } else {
        format!("Message not found: [{}]", msg)
    };

    ctx.send(CreateReply::default().content(response).ephemeral(true))
        .await?;
    Ok(())
}

/// List all custom death messages and their contributors
#[poise::command(slash_command)]
async fn list(ctx: Context<'_>) -> Result<(), crate::Error> {
    let messages = ctx.data().config.list_death_messages().await;

    if messages.is_empty() {
        ctx.send(
            CreateReply::default()
                .content("No custom death messages yet. Using default: [killed]")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let entries: Vec<String> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| format!("{}. [{}] — *contributed by* **{}**", i + 1, m.message, m.credited_to))
        .collect();

    let embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title("Custom Death Messages")
        .description(entries.join("\n"))
        .footer(serenity::all::CreateEmbedFooter::new(
            "Want your own? Contact vouchful to contribute a death message!",
        ));

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
