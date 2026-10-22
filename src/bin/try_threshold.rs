//! One-off: simulate `/threshold target_str: kn input: <input>` against our engine.
//! Mirrors what commands/threshold.rs does, but prints plain text.

use polytopia_bot::combat::{self, MultiCombatResult};
use polytopia_bot::commands::parse_battle_input;
use polytopia_bot::unit::{Unit, UnitType};

fn knight_threshold(defender: &Unit) -> Option<f32> {
    let knight = Unit::new(UnitType::Knight);
    let hp = combat::eliminate_attacker(&knight, defender);
    if hp > 0.0 { Some(hp) } else { None }
}

fn fmt_unit(u: &Unit) -> String {
    format!("{} @ {}/{}hp", u.display_name(), u.current_hp, u.max_hp)
}

fn print_result(title: &str, r: &MultiCombatResult) {
    println!("  {title}: final HP {} after {} events", r.defender_hp_final, r.events.len());
    for e in &r.events {
        println!(
            "    • {} [{}->{}] → def {}",
            e.attacker_name, e.attacker_hp_before, e.attacker_hp_after, e.defender_hp_after,
        );
    }
}

fn main() {
    let input = "kn,kn, scout , ri 3 vs de , gi 28";
    println!("input: {}", input);

    let (attackers, defenders) = match parse_battle_input(input) {
        Ok(r) => r,
        Err(e) => { println!("parse error: {e}"); return; }
    };

    println!("\nattackers ({}):", attackers.len());
    for a in &attackers { println!("  - {}", fmt_unit(a)); }
    println!("defenders ({}):", defenders.len());
    for d in &defenders { println!("  - {}", fmt_unit(d)); }

    // Knight one-shot thresholds per defender
    let mut targets = Vec::new();
    println!("\nknight one-shot thresholds:");
    let mut any_needs_work = false;
    for d in &defenders {
        match knight_threshold(d) {
            Some(hp) => {
                println!("  {}: ≤{:.0}hp for kn one-shot (currently {}hp)", d.display_name(), hp, d.current_hp);
                if hp < d.current_hp { any_needs_work = true; }
                targets.push(hp);
            }
            None => {
                println!("  {}: NO knight HP can one-shot this", d.display_name());
                return;
            }
        }
    }

    if !any_needs_work {
        println!("\nAll defenders are already at or below the kn one-shot threshold — no work needed.");
        return;
    }

    // Route to the appropriate combat function
    println!("\nrunning threshold_multi...");
    if defenders.len() == 1 {
        match combat::threshold(&attackers, &defenders[0], targets[0]) {
            Some(r) => print_result(&defenders[0].display_name(), &r),
            None => println!("  UNREACHABLE — no subset of attackers can bring {} to ≤{}hp",
                defenders[0].display_name(), targets[0]),
        }
    } else {
        match combat::threshold_multi(&attackers, &defenders, &targets) {
            Some((results, subset)) => {
                println!("subset used: {:?}", subset.iter().map(|&i| attackers[i].display_name()).collect::<Vec<_>>());
                for (i, r) in results.iter().enumerate() {
                    print_result(&defenders[i].display_name(), r);
                }
            }
            None => {
                println!("  UNREACHABLE — no subset of attackers can bring all defenders to their kn one-shot thresholds");
                println!("  targets were: {:?}", defenders.iter().zip(targets.iter()).map(|(d, t)| format!("{} ≤{}", d.display_name(), t)).collect::<Vec<_>>());
            }
        }
    }
}
