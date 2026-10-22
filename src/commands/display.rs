use crate::combat::{CombatEvent, MultiCombatResult};
use crate::unit::Unit;
use serenity::all::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, ReactionType};

/// Format HP: show 1 decimal only if fractional, otherwise integer.
/// Never displays negative HP — negatives are overkill indicators handled elsewhere.
pub fn hp(val: f32) -> String {
    let rounded = (val * 10.0).round() / 10.0;
    if (rounded - rounded.floor()).abs() < 0.01 {
        format!("{}", rounded as i32)
    } else {
        format!("{:.1}", rounded)
    }
}


/// Per-attacker event line. Format:
///     **Name:** startHP → endHP (defHP)
///
/// - Attacker HP capped at 0 (no negatives)
/// - Defender HP shown raw (can go negative to indicate overkill margin)
/// - NO death message on the attacker line — death messages only attach
///   to the defender summary below
pub fn atk_line(event: &CombatEvent, _death_msg: &str) -> String {
    let poisoned = if event.poison_applied { " [poisoned]" } else { "" };
    let raw_def_hp = event.defender_hp_before - event.damage_to_defender;
    format!(
        "**{}:** {} → {} ({}){}",
        event.attacker_name,
        hp(event.attacker_hp_before),
        hp(event.attacker_hp_after.max(0.0)),
        hp(raw_def_hp),
        poisoned,
    )
}

/// Build the "Attacker: startHP ➔ endHP (enemyHP)" header line.
pub fn attacker_header() -> String {
    "**Attacker: startHP ➔ endHP (enemyHP)**".to_string()
}

/// Defender summary line. Format:
///     startHP → finalHP                  (alive)
///     startHP → (N) DEATH_MSG            (dead — shows overkill as negative)
pub fn def_summary(defender: &Unit, result: &MultiCombatResult, death_msg: &str) -> String {
    let start = hp(defender.current_hp);
    if result.defender_hp_final <= 0.0 {
        let raw_final = result.events.last()
            .map(|e| e.defender_hp_before - e.damage_to_defender)
            .unwrap_or(0.0);
        format!("{} → ({}) {}", start, hp(raw_final), death_msg)
    } else {
        format!("{} → {}", start, hp(result.defender_hp_final))
    }
}

/// Compact "final HP or death" label used by survive for a single target.
pub fn def_final(hp_val: f32, death_msg: &str) -> String {
    if hp_val <= 0.0 {
        format!("({}) {}", hp(hp_val), death_msg)
    } else {
        format!("{}hp", hp(hp_val))
    }
}

/// Build embed for single-defender result.
/// Format matches the competitor: attacker header + lines in one field,
/// defender section in a second field with only its HP transition.
pub fn single_result_embed(result: &MultiCombatResult, defender: &Unit, title: &str, death_msg: &str) -> CreateEmbed {
    let mut attacker_block = vec![attacker_header()];
    for e in &result.events {
        attacker_block.push(atk_line(e, death_msg));
    }

    CreateEmbed::new()
        .color(0x2b7a78)
        .title(title)
        .description("The outcome of the fight is:")
        .field("\u{200B}", attacker_block.join("\n"), false)
        .field(
            format!("**{}:**", result.defender_name),
            def_summary(defender, result, death_msg),
            false,
        )
}

/// Build embed for multi-defender results.
/// One attacker-header block per defender so the user can see which
/// attackers were spent on which target.
pub fn multi_result_embed(results: &[MultiCombatResult], defenders: &[Unit], title: &str, death_msg: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .color(0x2b7a78)
        .title(title)
        .description("The outcome of the fight is:");

    for (i, (result, defender)) in results.iter().zip(defenders.iter()).enumerate() {
        let mut attacker_block = vec![attacker_header()];
        for e in &result.events {
            attacker_block.push(atk_line(e, death_msg));
        }
        if result.events.is_empty() {
            attacker_block.push("_No attackers committed to this defender_".to_string());
        }

        let field_name = format!("#{}: **{}**", i + 1, result.defender_name);
        let body = format!(
            "{}\n\n**{}:** {}",
            attacker_block.join("\n"),
            defender.display_name(),
            def_summary(defender, result, death_msg),
        );
        embed = embed.field(field_name, body, false);
    }

    embed
}

/// Build a one-row component with a trash-bin button the original command
/// author can click to delete the reply. The handler is registered globally
/// in `main.rs` and dispatches by `custom_id = del:<author_id>`.
pub fn delete_row(author_id: u64) -> CreateActionRow {
    CreateActionRow::Buttons(vec![
        CreateButton::new(format!("del:{}", author_id))
            .style(ButtonStyle::Secondary)
            .emoji(ReactionType::Unicode("\u{1F5D1}\u{FE0F}".to_string())),
    ])
}

/// Build a row with version toggle buttons (v120 / v122) and a trash button.
/// `active_version` should be `"v120"` or `"v122"`. v122 uses floored splash/explode damage.
/// The original input is encoded into the custom_id so the interaction handler
/// can recalculate when the version is toggled.
pub fn calc_row(author_id: u64, input: &str, active_version: &str) -> Vec<CreateActionRow> {
    // Truncate input to stay within Discord's 100-char custom_id limit.
    // Format: "ver:<version>:<author_id>:<input>" — reserve room for prefix.
    let prefix_len = "ver:v122:".len() + format!("{}", author_id).len() + 1; // +1 for ':'
    let max_input = 100 - prefix_len;
    let safe_input = if input.len() > max_input { &input[..max_input] } else { input };

    let v120_style = if active_version == "v120" { ButtonStyle::Primary } else { ButtonStyle::Secondary };
    let v122_style = if active_version == "v122" { ButtonStyle::Primary } else { ButtonStyle::Secondary };

    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("ver:v120:{}:{}", author_id, safe_input))
            .label("v120")
            .style(v120_style),
        CreateButton::new(format!("ver:v122:{}:{}", author_id, safe_input))
            .label("v122")
            .style(v122_style),
        CreateButton::new(format!("del:{}", author_id))
            .style(ButtonStyle::Secondary)
            .emoji(ReactionType::Unicode("\u{1F5D1}\u{FE0F}".to_string())),
    ])]
}
