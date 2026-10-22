use poise::CreateReply;
use serenity::all::CreateEmbed;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Show all unit abbreviations
#[poise::command(slash_command)]
pub async fn units(
    ctx: Context<'_>,
) -> Result<(), crate::Error> {
    let embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title("Unit Abbreviations")
        .field(
            "Standard",
            "\
`wa` Warrior  `ri` Rider  `kn` Knight
`de` Defender  `ar` Archer  `ca` Catapult
`sw` Swordsman  `gi` Giant  `mb` Mindbender",
            false,
        )
        .field(
            "Cymanti",
            "\
`he` Hexapod  `ki` Kiton  `ph` Phychi
`ex` Exida  `do` Doomux  `ce` Centipede
`se` Segment  `sh` Shaman  `ra` Raychi
`bm` Boomchi  `ma` Mantis  `ci` Ciru
`be` Bugegg  `mt` Moth  `la` Larva
`li` Island",
            false,
        )
        .field(
            "Aquarion",
            "\
`am` Amphibian  `tr` Tridention  `cr` Crab
`je` Jelly  `sk` Shark  `si` Siren  `aq` Aquapult
`mw` Mermaid Warrior  `md` Mermaid Defender",
            false,
        )
        .field(
            "Polaris",
            "\
`mo` Mooni  `bs` Battle Sled  `ia` Ice Archer
`if` Ice Fortress  `ga` Gaami",
            false,
        )
        .field(
            "Elyrion",
            "\
`po` Polytaur  `egg` Dragon Egg
`bd` Baby Dragon  `dr` Fire Dragon",
            false,
        )
        .field(
            "Diplomacy / Naval",
            "\
`da` Dagger  `cl` Cloak  `pi` Pirate
`bo` Bomber  `rm` Rammer  `ju` Juggernaut
`rf` Raft  `scsh` Scout",
            false,
        )
        .field(
            "Naval Shorthand",
            "\
Put ship type after unit: `wa sc` `wa bo` `wa rm` `wa rf`
**sc** Scout  **bo** Bomber  **rm** Rammer  **rf** Raft",
            false,
        );

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
