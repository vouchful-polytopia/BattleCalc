use poise::CreateReply;
use serenity::all::CreateEmbed;

use super::display;
use crate::unit::{find_unit, StatusEffects, Unit, UNIT_NAMES};

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Show a unit's stats, or list all units
#[poise::command(slash_command)]
pub async fn stats(
    ctx: Context<'_>,
    #[description = "Unit name, or 'list' to see all"]
    #[rest]
    input: String,
) -> Result<(), crate::Error> {
    let name = input.trim();

    if name.is_empty() || name.eq_ignore_ascii_case("list") {
        let mut categories: Vec<(&str, Vec<&str>)> = vec![
            ("Basic", vec![]),
            ("Cymanti", vec![]),
            ("Aquarion", vec![]),
            ("Polaris", vec![]),
            ("Elyrion", vec![]),
            ("Naval", vec![]),
            ("Diplomacy", vec![]),
            ("Other", vec![]),
        ];

        for &(name, _) in UNIT_NAMES {
            let cat = match name {
                "warrior" | "rider" | "knight" | "defender" | "catapult" | "archer"
                | "mindbender" | "swordsman" | "giant" => 0,
                "hexapod" | "kiton" | "phychi" | "exida" | "centipede" | "segment"
                | "raychi" | "shaman" | "doomux" | "boomchi" | "ciru" | "mantis"
                | "bugegg" | "moth" | "larva" | "island" => 1,
                "amphibian" | "tridention" | "crab" | "mermaidwarrior" | "mermaidarcher"
                | "mermaidswordsman" | "mermaiddefender" | "mermaidcloak" | "mermaiddagger"
                | "jelly" | "shark" | "siren" | "aquapult" => 2,
                "mooni" | "battlesled" | "icefortress" | "icearcher" | "gaami" => 3,
                "polytaur" | "dragonegg" | "babydragon" | "firedragon" => 4,
                "bombership" | "scoutship" | "transportship" | "rammership"
                | "juggernaut" | "pirate" => 5,
                "cloak" | "cloak_boat" | "dagger" => 6,
                _ => 7,
            };
            categories[cat].1.push(name);
        }

        let mut desc = String::new();
        for (label, units) in &categories {
            if !units.is_empty() {
                desc.push_str(&format!("**{}:** {}\n", label, units.join(", ")));
            }
        }

        let embed = CreateEmbed::new()
            .color(0x2b7a78)
            .title(format!("All Units ({})", UNIT_NAMES.len()))
            .description(desc);
        let author_id = ctx.author().id.get();
        ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
        return Ok(());
    }

    // Support comparing two units: "#s warrior vs defender"
    if let Some(pos) = name.to_lowercase().find(" vs ") {
        let name1 = &name[..pos].trim();
        let name2 = &name[pos + 4..].trim();
        return compare_units(ctx, name1, name2).await;
    }

    let unit_type = match find_unit(name) {
        Some(t) => t,
        None => {
            let suggestions = super::suggest_unit(name);
            let msg = if suggestions.is_empty() {
                format!("Unknown unit: `{name}`. Use `!s list` to see all units.")
            } else {
                format!("Unknown unit: `{name}`. Did you mean: {suggestions}?")
            };
            ctx.say(msg).await?;
            return Ok(());
        }
    };

    let u = Unit::new(unit_type);
    let type_name: &'static str = u.unit_type.into();

    let mut abilities = Vec::new();
    if u.surprise { abilities.push("Surprise (no retaliation)"); }
    if !u.retaliates { abilities.push("Stiff (can't retaliate)"); }
    if u.can_fortify { abilities.push("Fortify (city/wall bonus)"); }
    if u.can_explode { abilities.push("Explode (self-destruct AoE)"); }
    if u.can_splash { abilities.push("Splash (AoE half damage)"); }
    if u.promotable { abilities.push("Promotable (+5 HP veteran)"); }
    if u.trait_effects.contains(StatusEffects::POISONED) { abilities.push("Poison (applies -50% def)"); }
    if u.trait_effects.contains(StatusEffects::FROZEN) { abilities.push("Freeze (disables target)"); }
    if u.trait_effects.contains(StatusEffects::CONVERTED) { abilities.push("Convert (captures unit)"); }

    let embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title(format!("{} — Stats", type_name))
        .field("⚔️ Attack", format!("{}", u.attack), true)
        .field("🛡️ Defense", format!("{}", u.defense), true)
        .field("❤️ HP", format!("{}", u.max_hp as i32), true)
        .field("🎯 Range", format!("{}", u.range), true)
        .field(
            "Abilities",
            if abilities.is_empty() { "—".into() } else { abilities.join("\n") },
            false,
        );

    let author_id = ctx.author().id.get();
    ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
    Ok(())
}

async fn compare_units(ctx: Context<'_>, name1: &str, name2: &str) -> Result<(), crate::Error> {
    let t1 = find_unit(name1).ok_or_else(|| format!("Unknown unit: `{name1}`"))?;
    let t2 = find_unit(name2).ok_or_else(|| format!("Unknown unit: `{name2}`"))?;

    let u1 = Unit::new(t1);
    let u2 = Unit::new(t2);
    let n1: &'static str = u1.unit_type.into();
    let n2: &'static str = u2.unit_type.into();

    let fmt = |a: f32, b: f32| -> String {
        if a > b {
            format!("**{}** ✅", a)
        } else if a < b {
            format!("{}", a)
        } else {
            format!("{}", a)
        }
    };

    let embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title(format!("{} vs {}", n1, n2))
        .field(
            "Stat",
            "⚔️ Attack\n🛡️ Defense\n❤️ HP\n🎯 Range",
            true,
        )
        .field(
            n1,
            format!("{}\n{}\n{}\n{}",
                fmt(u1.attack, u2.attack),
                fmt(u1.defense, u2.defense),
                fmt(u1.max_hp, u2.max_hp),
                fmt(u1.range as f32, u2.range as f32),
            ),
            true,
        )
        .field(
            n2,
            format!("{}\n{}\n{}\n{}",
                fmt(u2.attack, u1.attack),
                fmt(u2.defense, u1.defense),
                fmt(u2.max_hp, u1.max_hp),
                fmt(u2.range as f32, u1.range as f32),
            ),
            true,
        );

    let author_id = ctx.author().id.get();
    ctx.send(CreateReply::default().embed(embed).components(vec![display::delete_row(author_id)])).await?;
    Ok(())
}

