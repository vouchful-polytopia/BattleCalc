use poise::serenity_prelude as serenity;
use polytopia_bot::{commands, config::Config, Data};

struct Handler {
    config: Config,
}

#[serenity::async_trait]
impl serenity::EventHandler for Handler {
    async fn interaction_create(&self, ctx: serenity::Context, interaction: serenity::Interaction) {
        let component = match interaction {
            serenity::Interaction::Component(c) => c,
            _ => return,
        };

        let custom_id = &component.data.custom_id;

        // Delete-button handler. Custom id format: `del:<author_id>`.
        if let Some(author_id_str) = custom_id.strip_prefix("del:") {
            let Ok(author_id) = author_id_str.parse::<u64>() else { return };

            if component.user.id.get() == author_id {
                let _ = component.message.delete(&ctx).await;
            } else {
                use serenity::all::{CreateInteractionResponse, CreateInteractionResponseMessage};
                let _ = component.create_response(&ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Only the person who ran the command can delete it.")
                            .ephemeral(true),
                    ),
                ).await;
            }
            return;
        }

        // Version toggle handler. Custom id format: `ver:<version>:<author_id>:<input>`.
        if let Some(rest) = custom_id.strip_prefix("ver:") {
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() < 3 { return; }
            let version = parts[0];       // "v120" or "v122"
            let Ok(author_id) = parts[1].parse::<u64>() else { return };
            let input = parts[2];

            if component.user.id.get() != author_id {
                use serenity::all::{CreateInteractionResponse, CreateInteractionResponseMessage};
                let _ = component.create_response(&ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Only the person who ran the command can use these buttons.")
                            .ephemeral(true),
                    ),
                ).await;
                return;
            }

            // Re-parse and recalculate
            let (attackers, defenders) = match commands::parse_battle_input(input) {
                Ok(r) => r,
                Err(_) => return,
            };

            // For v122, apply FLOOR_SPLASH to all splash/explode units (floored damage)
            let attackers: Vec<_> = if version == "v122" {
                attackers.into_iter().map(|mut u| {
                    if u.status_effects.contains(polytopia_bot::unit::StatusEffects::SPLASHING)
                        || u.status_effects.contains(polytopia_bot::unit::StatusEffects::EXPLODING)
                    {
                        u.status_effects.insert(polytopia_bot::unit::StatusEffects::FLOOR_SPLASH);
                    }
                    u
                }).collect()
            } else {
                attackers
            };

            let valid_attackers: Vec<_> = attackers
                .iter()
                .filter(|a| a.attack > 0.0 || a.status_effects.contains(polytopia_bot::unit::StatusEffects::EXPLODING))
                .collect();
            if valid_attackers.is_empty() { return; }

            let death_msg = self.config.get_random_death_message().await;
            let components = commands::display::calc_row(author_id, input, version);

            use serenity::all::{CreateInteractionResponse, CreateInteractionResponseMessage};
            let embed = if defenders.len() == 1 {
                let order: Vec<usize> = (0..attackers.len()).collect();
                let result = polytopia_bot::combat::multi_combat(&attackers, &defenders[0], &order);
                commands::display::single_result_embed(&result, &defenders[0], "Combat Result", &death_msg)
            } else {
                let results = polytopia_bot::combat::sequential_multi_defender(&attackers, &defenders);
                commands::display::multi_result_embed(&results, &defenders, "Sequential Combat", &death_msg)
            };

            // Update the message in place with the recalculated result
            let _ = component.create_response(&ctx,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components),
                ),
            ).await;
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("Set DISCORD_TOKEN env var");
    let config = Config::load();
    let config_for_handler = config.clone();
    let config_for_setup = config.clone();

    let all_commands = vec![
        commands::calc(),
        commands::optim(),
        commands::bulk(),
        commands::elim(),
        commands::stats(),
        commands::help(),
        commands::units(),
        commands::threshold(),
        commands::survive(),
        commands::assign(),
        commands::deathmsg(),
        commands::credits(),
    ];

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: all_commands,
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;

                poise::builtins::register_globally(ctx, commands).await?;
                println!("Bot is online!");
                Ok(Data {
                    config: config_for_setup,
                })
            })
        })
        .build();

    // No MESSAGE_CONTENT intent — slash commands only for interactions
    let intents = serenity::GatewayIntents::non_privileged();

    let client = serenity::ClientBuilder::new(token, intents)
        .event_handler(Handler {
            config: config_for_handler,
        })
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
