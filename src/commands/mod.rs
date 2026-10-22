mod calc;
mod optim;
mod bulk;
mod elim;
mod stats;
mod help;
mod units;
mod threshold;
mod survive;
pub mod display;
mod assign;
mod deathmsg;
mod credits;

pub use calc::calc;
pub use optim::optim;
pub use bulk::bulk;
pub use elim::elim;
pub use stats::stats;
pub use help::help;
pub use units::units;
pub use threshold::threshold;
pub use survive::survive;
pub use assign::assign;
pub use deathmsg::deathmsg;
pub use credits::credits;

use crate::unit::{find_unit, StatusEffects, Unit, UnitType, UNIT_NAMES};

pub fn parse_unit(input: &str) -> Result<Unit, String> {
    let tokens: Vec<&str> = input.trim().split_whitespace().collect();
    if tokens.is_empty() {
        return Err("Empty unit specification".into());
    }

    // Classify each token to find unit name, naval code, and modifiers in any order.
    // A token is a naval code if it matches sc/bo/rm/rf etc.
    // A token is a modifier if it matches v/d/w/p/b/x/s/r/nr/f/fzn etc.
    // A token is a number if it parses as f32.
    // A token is a unit name if find_unit() succeeds.
    // Some tokens are ambiguous (e.g. "rm" is both unit and naval, "def" is both unit and modifier).

    fn is_naval_token(s: &str) -> Option<&'static str> {
        match s {
            "sc" | "scout" | "scoutship" => Some("sc"),
            "bo" | "bomb" | "bomber" | "bombership" => Some("bo"),
            "rm" | "ram" | "rammer" | "rammership" => Some("rm"),
            "rf" | "raft" | "transport" | "transportship" => Some("rf"),
            _ => None,
        }
    }

    fn is_modifier_token(s: &str) -> bool {
        matches!(s,
            "v" | "vet" | "veteran" | "d" | "protected" | "def" |
            "w" | "wall" | "walled" | "p" | "psn" | "poison" | "poisoned" |
            "b" | "boost" | "swarm" | "x" | "explode" | "exploding" |
            "ax" | "attackexplode" | "axi" | "attackexplodeinstant" |
            "s" | "sp" | "splash" | "splashing" | "fl" | "floor" |
            "r" | "nr" | "f" | "final" | "fzn" | "frozen" |
            "1h" | "onehit"
        )
    }

    // Pass 1: Find a definite unit name (matches find_unit, NOT a naval code or modifier or number)
    let mut unit_idx: Option<usize> = None;
    for (i, tok) in tokens.iter().enumerate() {
        let lower = tok.to_lowercase();
        if lower.parse::<f32>().is_ok() { continue; }
        if is_modifier_token(&lower) { continue; }
        if is_naval_token(&lower).is_some() && find_unit(&lower).is_some() {
            continue; // ambiguous — skip for now
        }
        if find_unit(&lower).is_some() {
            unit_idx = Some(i);
            break;
        }
    }

    // Pass 2: If no definite unit, try ambiguous tokens (unit + naval)
    if unit_idx.is_none() {
        for (i, tok) in tokens.iter().enumerate() {
            let lower = tok.to_lowercase();
            if find_unit(&lower).is_some() {
                unit_idx = Some(i);
                break;
            }
        }
    }

    // Pass 3: Naval-only fallback. If no unit name matched but a naval token
    // is present (e.g. `scout`, `bomber`, `rammer`, `raft` with no attached
    // unit), default to a Warrior carrying that ship. Matches the competitor's
    // "You didn't provide the hp of your naval unit, so we made it a Warrior"
    // behavior.
    let naval_only_fallback = if unit_idx.is_none() {
        tokens.iter().any(|t| is_naval_token(&t.to_lowercase()).is_some())
    } else {
        false
    };

    let (unit_type, unit_idx_for_skip, unit_name) = if naval_only_fallback {
        (UnitType::Warrior, usize::MAX, "warrior")
    } else {
        let unit_idx = unit_idx.unwrap_or(0);
        let name = tokens[unit_idx];
        let ut = find_unit(name).ok_or_else(|| {
            let suggestions = suggest_unit(name);
            if suggestions.is_empty() {
                format!("Unknown unit: `{}`. Try `!s list` to see all units.", name)
            } else {
                format!("Unknown unit: `{}`. Did you mean: {}?", name, suggestions)
            }
        })?;
        (ut, unit_idx, name)
    };
    let unit_idx = unit_idx_for_skip;

    let mut unit = Unit::new(unit_type);

    // Find naval code among remaining tokens
    let mut naval_idx: Option<usize> = None;
    for (i, tok) in tokens.iter().enumerate() {
        if i == unit_idx { continue; }
        let lower = tok.to_lowercase();
        if is_naval_token(&lower).is_some() {
            naval_idx = Some(i);
            break;
        }
    }

    // Apply naval conversion
    if let Some(ni) = naval_idx {
        let code = is_naval_token(&tokens[ni].to_lowercase()).unwrap();
        let base_hp = unit.current_hp;
        let base_max = unit.max_hp;
        match code {
            "sc" => {
                unit.attack = 2.0;
                unit.defense = 1.0;
                unit.range = 2;
                unit.retaliates = true;
                unit.surprise = false;
                unit.can_splash = false;
                unit.naval_type = Some("Scout");
            }
            "bo" => {
                unit.attack = 3.0;
                unit.defense = 2.0;
                unit.range = 3;
                unit.retaliates = false;
                unit.surprise = false;
                unit.can_splash = true;
                unit.naval_type = Some("Bomber");
            }
            "rm" => {
                unit.attack = 3.0;
                unit.defense = 3.0;
                unit.range = 1;
                unit.retaliates = true;
                unit.surprise = false;
                unit.can_splash = false;
                unit.naval_type = Some("Rammer");
            }
            "rf" => {
                unit.attack = 0.0;
                unit.defense = 1.0;
                unit.range = 0;
                unit.retaliates = false;
                unit.surprise = false;
                unit.can_splash = false;
                unit.naval_type = Some("Raft");
            }
            _ => {}
        }
        unit.current_hp = base_hp;
        unit.max_hp = base_max;
    }

    // Pass 1: apply all non-HP modifiers. Done before HP so that VETERAN
    // (which bumps max_hp and current_hp by 5) lands before any user HP is
    // assigned — otherwise `warrior 10 v` would set hp=10 and then have
    // apply_status bump current_hp back to 15.
    for (i, &tok) in tokens.iter().enumerate() {
        if i == unit_idx { continue; }
        if Some(i) == naval_idx { continue; }
        let lower = tok.to_lowercase();
        if lower.parse::<f32>().is_ok() { continue; }
        match lower.as_str() {
            "v" | "vet" | "veteran" => unit.apply_status(StatusEffects::VETERAN),
            "d" | "protected" | "def" => unit.apply_status(StatusEffects::FORTIFIED),
            "w" | "wall" | "walled" => {
                if unit.can_fortify {
                    unit.apply_status(StatusEffects::WALLED);
                } else {
                    return Err(format!(
                        "{} can't benefit from walls (no Fortify ability). Use `d` for terrain defense instead.",
                        unit_name
                    ));
                }
            }
            "p" | "psn" | "poison" | "poisoned" => unit.apply_status(StatusEffects::POISONED),
            "b" | "boost" | "swarm" => {
                unit.apply_status(StatusEffects::BOOSTED);
            }
            "x" | "explode" | "exploding" => {
                if unit.can_explode {
                    unit.status_effects.insert(StatusEffects::EXPLODING);
                } else {
                    return Err(format!("{} can't explode", unit_name));
                }
            }
            "axi" | "attackexplodeinstant" => {
                if unit.can_explode {
                    unit.status_effects.insert(StatusEffects::ATTACK_EXPLODE | StatusEffects::INSTANT_EXPLODE);
                } else {
                    return Err(format!("{} can't explode", unit_name));
                }
            }
            "ax" | "attackexplode" => {
                if unit.can_explode {
                    unit.status_effects.insert(StatusEffects::ATTACK_EXPLODE);
                } else {
                    return Err(format!("{} can't explode", unit_name));
                }
            }
            "s" | "sp" | "splash" | "splashing" => {
                if unit.can_splash {
                    unit.status_effects.insert(StatusEffects::SPLASHING);
                } else {
                    return Err(format!("{} can't splash", unit_name));
                }
            }
            "fl" | "floor" => unit.status_effects.insert(StatusEffects::FLOOR_SPLASH),
            "r" => unit.status_effects.insert(StatusEffects::FORCE_RETALIATE),
            "nr" => unit.status_effects.insert(StatusEffects::NO_RETALIATE),
            "f" | "final" => { unit.is_final = true; }
            "fzn" | "frozen" => unit.apply_status(StatusEffects::FROZEN),
            "1h" | "onehit" => {
                unit.double_attack = false;
                if unit.can_explode {
                    unit.status_effects.insert(StatusEffects::NO_EXPLODE);
                }
            }
            _ => {
                return Err(format!(
                    "Unknown modifier: `{}`. Valid: v(eteran), d(efense), w(all), p(oison), b(oost), x(plode), ax, axi, 1h, s(plash), fl(oor splash), r(etaliate), nr, f(inal), fzn, or HP number",
                    tok
                ));
            }
        }
    }

    // Pass 2: apply HP. Modifiers are already in place, so max_hp already
    // reflects VETERAN if the user asked for it — the HP is clamped against
    // the correct max and assigned exactly once at the end.
    for (i, &tok) in tokens.iter().enumerate() {
        if i == unit_idx { continue; }
        if Some(i) == naval_idx { continue; }
        let Ok(hp) = tok.to_lowercase().parse::<f32>() else { continue; };
        if hp <= 0.0 {
            return Err("HP must be greater than 0".into());
        }
        if hp > unit.max_hp {
            if unit.promotable && hp <= unit.max_hp + 5.0 {
                unit.apply_status(StatusEffects::VETERAN);
            }
            if hp > unit.max_hp {
                let hint = if unit.promotable && !unit.status_effects.contains(StatusEffects::VETERAN) {
                    ". Max with veteran: ".to_string() + &((unit.max_hp + 5.0) as i32).to_string()
                } else {
                    String::new()
                };
                return Err(format!(
                    "HP {} exceeds max {} for {}{}",
                    hp as i32,
                    unit.max_hp as i32,
                    unit_name,
                    hint
                ));
            }
        }
        unit.current_hp = hp;
    }

    Ok(unit)
}

/// Parse input string into (attackers, defenders)
/// Supports two syntaxes:
///   "wa, ri, de d"        — last unit is defender (legacy)
///   "wa, ri vs de d"      — explicit separator (new, supports multi-defender)
///   "wa, ri vs gi, de w"  — multi-defender: fight giant first, then walled defender
pub fn parse_battle_input(input: &str) -> Result<(Vec<Unit>, Vec<Unit>), String> {
    let vs_parts: Vec<&str> = split_on_vs(input);

    let (mut attackers, mut defenders) = if vs_parts.len() >= 2 {
        let attackers = parse_units_csv(vs_parts[0])?;
        let defenders = parse_units_csv(vs_parts[1])?;

        if attackers.is_empty() {
            return Err("No attackers specified before `vs`".into());
        }
        if defenders.is_empty() {
            return Err("No defenders specified after `vs`".into());
        }

        (attackers, defenders)
    } else {
        let all = parse_units_csv(input)?;
        if all.len() < 2 {
            return Err("Need at least 2 units. Use: `unit1, unit2` (last is defender) or `unit1 vs unit2`".into());
        }
        let mut attackers = all;
        let defender = attackers.pop().unwrap();
        (attackers, vec![defender])
    };

    // Label duplicate unit names with #1, #2, etc. for disambiguation
    label_duplicates(&mut attackers);
    label_duplicates(&mut defenders);

    Ok((attackers, defenders))
}

fn label_duplicates(units: &mut [Unit]) {
    // Count occurrences of each base display name
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for u in units.iter() {
        *counts.entry(u.display_name()).or_insert(0) += 1;
    }
    // Only label if there are duplicates
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

fn split_on_vs(input: &str) -> Vec<&str> {
    // Find " vs " case-insensitively, preserving original string slices
    let lower = input.to_lowercase();
    if let Some(pos) = lower.find(" vs ") {
        vec![&input[..pos], &input[pos + 4..]]
    } else {
        vec![input]
    }
}

fn parse_units_csv(input: &str) -> Result<Vec<Unit>, String> {
    let parts: Vec<&str> = input.split(',').collect();
    let mut units = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        units.push(parse_unit(trimmed)?);
    }
    Ok(units)
}

fn suggest_unit(input: &str) -> String {
    let lower = input.to_lowercase();
    let matches: Vec<&str> = UNIT_NAMES
        .iter()
        .filter(|&&(name, _)| {
            name.starts_with(&lower) || name.contains(&lower)
        })
        .map(|&(name, _)| name)
        .take(5)
        .collect();
    matches.join(", ")
}

// Autocomplete function for poise slash commands
pub async fn autocomplete_unit<'a>(
    _ctx: poise::Context<'a, crate::Data, crate::Error>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let lower = partial.to_lowercase();
    UNIT_NAMES
        .iter()
        .filter(move |&&(name, _)| name.starts_with(&lower) || name.contains(&lower))
        .take(25)
        .map(|&(name, _)| name.to_string())
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    use crate::unit::StatusEffects;

    #[test]
    fn veteran_before_hp_sets_hp_at_damaged_level() {
        let u = parse_unit("warrior v 10").unwrap();
        assert!(u.status_effects.contains(StatusEffects::VETERAN));
        assert_eq!(u.max_hp, 15.0);
        assert_eq!(u.current_hp, 10.0);
    }

    #[test]
    fn veteran_after_hp_does_not_rebump_current_hp() {
        // Regression: previously, "warrior 10 v" set hp=10 then apply_status
        // bumped current_hp by 5, yielding 15/15 instead of 10/15.
        let u = parse_unit("warrior 10 v").unwrap();
        assert!(u.status_effects.contains(StatusEffects::VETERAN));
        assert_eq!(u.max_hp, 15.0);
        assert_eq!(u.current_hp, 10.0);
    }

    #[test]
    fn implicit_veteran_from_high_hp_still_works() {
        // User writes just an HP above base max; parser auto-promotes.
        let u = parse_unit("warrior 14").unwrap();
        assert!(u.status_effects.contains(StatusEffects::VETERAN));
        assert_eq!(u.max_hp, 15.0);
        assert_eq!(u.current_hp, 14.0);
    }

    #[test]
    fn veteran_at_full_max_hp() {
        let u = parse_unit("warrior v").unwrap();
        assert_eq!(u.max_hp, 15.0);
        assert_eq!(u.current_hp, 15.0);
    }

    #[test]
    fn hp_exceeds_veteran_max_errors() {
        // Not yet veteran, hint should suggest veteran max.
        let err = parse_unit("warrior 20").unwrap_err();
        assert!(err.contains("Max with veteran: 15"), "got: {err}");
    }

    #[test]
    fn hp_exceeds_max_when_already_veteran_no_hint() {
        // Already veteran via modifier — don't suggest a phantom +5 max.
        let err = parse_unit("warrior v 20").unwrap_err();
        assert!(!err.contains("with veteran"), "hint should be suppressed: {err}");
    }

    #[test]
    fn modifier_order_independent_for_poison_and_hp() {
        let a = parse_unit("defender 8 p").unwrap();
        let b = parse_unit("defender p 8").unwrap();
        assert_eq!(a.current_hp, 8.0);
        assert_eq!(b.current_hp, 8.0);
        assert!(a.status_effects.contains(StatusEffects::POISONED));
        assert!(b.status_effects.contains(StatusEffects::POISONED));
    }
}
