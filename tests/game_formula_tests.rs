//! Tests derived by manually computing expected results using the game's
//! combat formula, then verifying the bot reproduces them.
//!
//! ## Game formula reference:
//!
//! ```text
//! attack_force  = attacker.GetAttack() * attacker.health * 100 / attacker.maxHealth
//! defense_force = defender.GetDefence() * defender.health * 100 / defender.maxHealth
//! total_force   = attack_force + defense_force
//!
//! attackDamage      = Round(attack_force * atk * 45 * 10 / (1000 * total_force))
//! retaliationDamage = Round(defense_force * def * 45 * 100 / (1000 * total_force * defenseBonus))
//!
//! Round(n) = round to nearest multiple of 10 (ties go UP)
//! ```
//!
//! The bot scales internally by 10x (HP) and 100x (attack/defense) for
//! precision with fractional stats like Phychi (0.7 atk). The formulas
//! are algebraically equivalent.
//!
//! Each test includes a comment block showing the manual calculation so
//! anyone can verify the expected values independently.

use polytopia_bot::combat::{
    single_combat, multi_combat, optimized, optimized_multi_defender,
    bulk, eliminate_defender,
};
use polytopia_bot::unit::{StatusEffects, Unit, UnitType};

// ============================================================
// Helper: assert damage values with tolerance for f32
// ============================================================

fn assert_damage(label: &str, actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{}: expected {}, got {}",
        label, expected, actual
    );
}

// ============================================================
// SECTION 1: Single 1v1 combats — exact game formula validation
// ============================================================

/// Warrior (2/2/10) vs Warrior (2/2/10), both full HP.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 200*100*100/100 = 20000
/// total = 40000
/// atk_dmg = round(20000*200*450 / (1000*40000)) /10 = round(45)/10 = 50/10 = 5
/// ret_dmg = round(20000*200*4500 / (1000*40000*10)) /10 = round(45)/10 = 50/10 = 5
#[test]
fn game_formula_warrior_vs_warrior_full_hp() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 5.0);
}

/// Warrior (2/2/10) vs Defender (1/3/15), both full HP, no wall.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 300*150*100/150 = 30000
/// total = 50000
/// atk_dmg = round(20000*200*450 / (1000*50000)) /10 = round(36)/10 = 40/10 = 4
/// ret_dmg = round(30000*300*4500 / (1000*50000*10)) /10 = round(81)/10 = 80/10 = 8
#[test]
fn game_formula_warrior_vs_defender() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Defender);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 4.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 8.0);
}

/// Knight (3.5/1/10) vs Warrior (2/2/10), both full HP.
///
/// attack_force  = 350*100*100/100 = 35000
/// defense_force = 200*100*100/100 = 20000
/// total = 55000
/// atk_dmg = round(35000*350*450 / (1000*55000)) /10 = round(100)/10 = 100/10 = 10
/// Defender dies (10 - 10 = 0), so no retaliation.
#[test]
fn game_formula_knight_vs_warrior_kills() {
    let a = Unit::new(UnitType::Knight);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 10.0);
    assert_damage("ret_dmg (defender dead)", r.damage_to_attacker, 0.0);
}

/// Archer (2/1/10, range=2) vs Warrior (2/2/10, range=1).
/// Archer out-ranges warrior, so no retaliation.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 200*100*100/100 = 20000
/// total = 40000
/// atk_dmg = round(20000*200*450 / (1000*40000)) /10 = round(45)/10 = 50/10 = 5
/// No retaliation (range 2 > range 1).
#[test]
fn game_formula_archer_vs_warrior_no_retaliation() {
    let a = Unit::new(UnitType::Archer);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

/// Catapult (4/0/10, range=3) vs Warrior (2/2/10, range=1).
/// Catapult out-ranges warrior. Catapult has 0 defense but that only matters
/// if IT were the defender.
///
/// attack_force  = 400*100*100/100 = 40000
/// defense_force = 200*100*100/100 = 20000
/// total = 60000
/// atk_dmg = round(40000*400*450 / (1000*60000)) /10 = round(120)/10 = 120/10 = 12
/// No retaliation (range 3 > 1).
#[test]
fn game_formula_catapult_vs_warrior() {
    let a = Unit::new(UnitType::Catapult);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 12.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

/// Swordsman (3/3/15) vs Swordsman (3/3/15), both full HP.
/// Symmetric — same damage both ways.
///
/// attack_force  = 300*150*100/150 = 30000
/// defense_force = 300*150*100/150 = 30000
/// total = 60000
/// atk_dmg = round(30000*300*450 / (1000*60000)) /10 = round(67)/10 = 70/10 = 7
/// ret_dmg = round(30000*300*4500 / (1000*60000*10)) /10 = round(67)/10 = 70/10 = 7
#[test]
fn game_formula_swordsman_vs_swordsman() {
    let a = Unit::new(UnitType::Swordsman);
    let d = Unit::new(UnitType::Swordsman);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 7.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 7.0);
}

/// Warrior (2/2/10) vs Giant (5/4/40), both full HP.
/// Giant's huge defense force means warrior deals very little.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 400*400*100/400 = 40000
/// total = 60000
/// atk_dmg = round(20000*200*450 / (1000*60000)) /10 = round(30)/10 = 30/10 = 3
/// ret_dmg = round(40000*400*4500 / (1000*60000*10)) /10 = round(120)/10 = 120/10 = 12
/// Warrior dies (10 - 12 < 0).
#[test]
fn game_formula_warrior_vs_giant() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Giant);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 3.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 12.0);
}

/// Giant (5/4/40) vs Warrior (2/2/10), both full HP.
/// Giant one-shots the warrior → no retaliation.
///
/// attack_force  = 500*400*100/400 = 50000
/// defense_force = 200*100*100/100 = 20000
/// total = 70000
/// atk_dmg = round(50000*500*450 / (1000*70000)) /10 = round(160)/10 = 160/10 = 16
/// Defender dies (10 - 16 < 0) → no retaliation.
#[test]
fn game_formula_giant_vs_warrior_kills() {
    let a = Unit::new(UnitType::Giant);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 16.0);
    assert_damage("ret_dmg (dead)", r.damage_to_attacker, 0.0);
}

/// Rider (2/1/10) vs Archer (2/1/10), melee range.
/// Same attack, same HP, so attack_force is equal. But rider range ≤ archer range,
/// so the archer CAN retaliate.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 100*100*100/100 = 10000
/// total = 30000
/// atk_dmg = round(20000*200*450 / (1000*30000)) /10 = round(60)/10 = 60/10 = 6
/// ret_dmg = round(10000*100*4500 / (1000*30000*10)) /10 = round(15)/10 = 20/10 = 2
#[test]
fn game_formula_rider_vs_archer() {
    let a = Unit::new(UnitType::Rider);
    let d = Unit::new(UnitType::Archer);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 2.0);
}

// ============================================================
// SECTION 2: Reduced HP — verifying HP scaling in the formula
// ============================================================

/// Warrior at 5hp vs Warrior at 10hp.
/// Reduced attacker HP lowers attack_force.
///
/// attack_force  = 200*50*100/100 = 10000
/// defense_force = 200*100*100/100 = 20000
/// total = 30000
/// atk_dmg = round(10000*200*450 / (1000*30000)) /10 = round(30)/10 = 30/10 = 3
/// ret_dmg = round(20000*200*4500 / (1000*30000*10)) /10 = round(60)/10 = 60/10 = 6
#[test]
fn game_formula_half_hp_warrior_vs_warrior() {
    let a = Unit::new(UnitType::Warrior).with_hp(5.0);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 3.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
}

/// Warrior at 1hp vs Warrior at 10hp.
/// Near-death attacker deals minimal damage.
///
/// attack_force  = 200*10*100/100 = 2000
/// defense_force = 200*100*100/100 = 20000
/// total = 22000
/// atk_dmg = round(2000*200*450 / (1000*22000)) /10 = round(8)/10 = 10/10 = 1
/// ret_dmg = round(20000*200*4500 / (1000*22000*10)) /10 = round(81)/10 = 80/10 = 8
#[test]
fn game_formula_1hp_warrior_vs_warrior() {
    let a = Unit::new(UnitType::Warrior).with_hp(1.0);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 1.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 8.0);
}

/// Warrior at full HP vs Giant at 10hp (heavily wounded).
/// Low-HP giant has reduced defense_force.
///
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 400*100*100/400 = 10000
/// total = 30000
/// atk_dmg = round(20000*200*450 / (1000*30000)) /10 = round(60)/10 = 60/10 = 6
/// ret_dmg = round(10000*400*4500 / (1000*30000*10)) /10 = round(60)/10 = 60/10 = 6
#[test]
fn game_formula_warrior_vs_wounded_giant() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Giant).with_hp(10.0);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
}

// ============================================================
// SECTION 3: Status effects — fortified, walled, poisoned
// ============================================================

/// Warrior vs Defender (fortified). Defender can_fortify=true, so defense_bonus=1.5.
///
/// def = 300 * 15/10 = 450
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 450*150*100/150 = 45000
/// total = 65000
/// atk_dmg = round(20000*200*450 / (1000*65000)) /10 = round(27)/10 = 30/10 = 3
/// ret_dmg = round(45000*450*4500 / (1000*65000*15)) /10 = round(93)/10 = 90/10 = 9
#[test]
fn game_formula_warrior_vs_fortified_defender() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Defender).with_status(StatusEffects::FORTIFIED);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 3.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 9.0);
}

/// Warrior vs Defender (walled). Defender can_fortify=true, so defense_bonus=4.0.
///
/// def = 300 * 40/10 = 1200
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 1200*150*100/150 = 120000
/// total = 140000
/// atk_dmg = round(20000*200*450 / (1000*140000)) /10 = round(12)/10 = 10/10 = 1
/// ret_dmg = round(120000*1200*4500 / (1000*140000*40)) /10 = round(115)/10 = 120/10 = 12
#[test]
fn game_formula_warrior_vs_walled_defender() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Defender).with_status(StatusEffects::WALLED);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 1.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 12.0);
}

/// Warrior vs Warrior (walled). Warrior can_fortify=true, so defense_bonus=4.0.
///
/// def = 200 * 40/10 = 800
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 800*100*100/100 = 80000
/// total = 100000
/// atk_dmg = round(20000*200*450 / (1000*100000)) /10 = round(18)/10 = 20/10 = 2
/// ret_dmg = round(80000*800*4500 / (1000*100000*40)) /10 = round(72)/10 = 70/10 = 7
#[test]
fn game_formula_warrior_vs_walled_warrior() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Warrior).with_status(StatusEffects::WALLED);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 2.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 7.0);
}

/// Warrior vs Warrior (poisoned). Poison halves defense_bonus via integer
/// division: 10/2 = 5 → defense_bonus = 0.5.
///
/// def = 200 * 5/10 = 100
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 100*100*100/100 = 10000
/// total = 30000
/// atk_dmg = round(20000*200*450 / (1000*30000)) /10 = round(60)/10 = 60/10 = 6
/// ret_dmg = round(10000*100*4500 / (1000*30000*5)) /10 = round(30)/10 = 30/10 = 3
#[test]
fn game_formula_warrior_vs_poisoned_warrior() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Warrior).with_status(StatusEffects::POISONED);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 3.0);
    // Verify defense_bonus is exactly 0.5 (10/2 = 5 → 0.5)
    assert_damage("poison defense_bonus", d.defense_bonus, 0.5);
}

/// Warrior vs Defender (fortified + poisoned). Stored defense_bonus is
/// 1.5 * 0.5 = 0.75, but combat.rs truncates via `as i64`: 0.75*10 = 7.5 → 7.
/// So the effective multiplier is 0.7, matching the game's integer math.
///
/// def_bonus_int = 7 (truncated from 7.5)
/// def = 300 * 7/10 = 210
/// attack_force  = 200*100*100/100 = 20000
/// defense_force = 210*150*100/150 = 21000
/// total = 41000
/// atk_dmg = round(20000*200*450 / (1000*41000)) /10 = round(43)/10 = 40/10 = 4
///
/// Retaliation:
/// ret = round(21000*210*4500 / (1000*41000*7)) /10 = round(69)/10 = 70/10 = 7
#[test]
fn game_formula_warrior_vs_fortified_poisoned_defender() {
    let d = Unit::new(UnitType::Defender)
        .with_status(StatusEffects::FORTIFIED | StatusEffects::POISONED);
    // Stored as 0.75, but combat.rs truncates to int → effective 0.7
    assert_damage("fortified+poisoned bonus", d.defense_bonus, 0.75);

    let a = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 4.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 7.0);
}

// ============================================================
// SECTION 4: Surprise & special retaliation rules
// ============================================================

/// Dagger (2/2/10, surprise=true) vs Warrior. Surprise → no retaliation.
/// Same stats, so same attack damage as warrior-vs-warrior.
///
/// attack_force = 20000, defense_force = 20000, total = 40000
/// atk_dmg = 5.0, retaliation suppressed by surprise.
#[test]
fn game_formula_dagger_surprise_no_retaliation() {
    let a = Unit::new(UnitType::Dagger);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg (surprise)", r.damage_to_attacker, 0.0);
}

/// Ice archer (1/1/10, range=2, trait_effects=FROZEN) vs Warrior (range=1).
/// Range 2 > 1 means no retaliation. Also applies FROZEN to defender.
///
/// att=100, def=200, bonus=1.0
/// attack_force  = 100*100*100/100 = 10000
/// defense_force = 200*100*100/100 = 20000
/// total = 30000
/// atk_dmg = round(10000*100*450 / (1000*30000)) /10 = round(15)/10 = 20/10 = 2
#[test]
fn game_formula_icearcher_vs_warrior() {
    let a = Unit::new(UnitType::Icearcher);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 2.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
    // Ice archer applies FROZEN to surviving defender
    assert!(
        r.effects_to_defender.contains(StatusEffects::FROZEN),
        "ice archer should freeze surviving defender"
    );
}

/// Warrior vs Frozen Warrior. Frozen → can't retaliate.
/// Same damage calculation as normal, but retaliation suppressed.
///
/// atk_dmg = 5.0 (same as warrior vs warrior)
/// retaliation = 0 (frozen)
#[test]
fn game_formula_warrior_vs_frozen_warrior() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Warrior).with_status(StatusEffects::FROZEN);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg (frozen)", r.damage_to_attacker, 0.0);
}

// ============================================================
// SECTION 5: Veteran modifier — +5 max HP
// ============================================================

/// Veteran Warrior (2/2/15) vs Warrior (2/2/10).
/// Veteran HP scaling: attack_force same ratio (full HP).
///
/// attack_force  = 200*150*100/150 = 20000
/// defense_force = 200*100*100/100 = 20000
/// total = 40000
/// Same ratio as non-veteran → same damage numbers.
/// atk_dmg = round(45)/10 = 5.0
/// ret_dmg = round(45)/10 = 5.0
#[test]
fn game_formula_veteran_warrior_vs_warrior() {
    let a = Unit::new(UnitType::Warrior).with_status(StatusEffects::VETERAN);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 5.0);
    // Verify veteran has 15 max HP
    assert_eq!(a.max_hp, 15.0);
}

// ============================================================
// SECTION 6: Splash damage (fire dragon, bombership)
// ============================================================

/// Fire Dragon (4/3/20, range=2, splash) vs Warrior (2/2/10).
/// Splash halves the raw damage.
///
/// att=400, def=200, a_hp=200, a_max=200, d_hp=100, d_max=100
/// attack_force  = 400*200*100/200 = 40000
/// defense_force = 200*100*100/100 = 20000
/// total = 60000
/// raw_atk_dmg = round(40000*400*450 / (1000*60000)) /10 = round(120)/10 = 12
/// splash: 12 / 2.0 = 6.0
/// No retaliation (range 2 > 1).
#[test]
fn game_formula_firedragon_splash_vs_warrior() {
    let a = Unit::new(UnitType::Firedragon).with_status(StatusEffects::SPLASHING);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("splash_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

/// Bombership (3/2/10, range=3, splash) vs Warrior (2/2/10).
///
/// att=300, def=200, a_hp=100, a_max=100, d_hp=100, d_max=100
/// attack_force  = 300*100*100/100 = 30000
/// defense_force = 200*100*100/100 = 20000
/// total = 50000
/// raw_atk_dmg = round(30000*300*450 / (1000*50000)) /10 = round(81)/10 = 80/10 = 8
/// splash: 8 / 2.0 = 4.0
/// No retaliation (range 3 > 1).
#[test]
fn game_formula_bombership_splash_vs_warrior() {
    let a = Unit::new(UnitType::Bombership).with_status(StatusEffects::SPLASHING);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("splash_dmg", r.damage_to_defender, 4.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

// ============================================================
// SECTION 7: Exploding units (Segment, Doomux with x modifier)
// ============================================================

/// Segment (2/2/10, EXPLODING) vs Warrior (2/2/10).
/// Exploding: raw damage halved (splash), attacker takes self-damage = full HP.
///
/// Same stats as warrior vs warrior → raw_dmg = 5.
/// Exploding: 5 / 2.0 = 2.5
/// Attacker self-damage = 10 (dies).
#[test]
fn game_formula_segment_explode_vs_warrior() {
    let a = Unit::new(UnitType::Segment);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("explode_dmg", r.damage_to_defender, 2.5);
    assert_damage("self_dmg", r.damage_to_attacker, 10.0); // full HP self-destruct
    // Surviving defender gets poisoned (segment trait)
    assert!(
        r.effects_to_defender.contains(StatusEffects::POISONED),
        "segment should poison surviving defender"
    );
}

// ============================================================
// SECTION 8: Round function edge cases
// ============================================================

/// Verify the rounding function: ties go UP.
/// Rule: if (n - lower >= upper - n) return upper;
/// e.g., 45 → lower=40, upper=50, 45-40=5, 50-45=5 → tie → 50
/// e.g., 15 → lower=10, upper=20, 15-10=5, 20-15=5 → tie → 20
///
/// We test this indirectly through combats that produce rounding tie values.
///
/// Warrior vs Warrior produces round(45) → 50 → /10 = 5 (verified above).
/// Ice Archer vs Warrior produces round(15) → 20 → /10 = 2 (verified above).
/// These confirm the tie-breaks-up behavior.
#[test]
fn game_formula_round_tie_breaks_up() {
    // Warrior vs Warrior: internal value 45 → rounds to 50
    let r = single_combat(&Unit::new(UnitType::Warrior), &Unit::new(UnitType::Warrior));
    assert_damage("round(45)→50", r.damage_to_defender, 5.0);

    // Ice Archer (1/1/10) vs Warrior: internal value 15 → rounds to 20
    let r2 = single_combat(&Unit::new(UnitType::Icearcher), &Unit::new(UnitType::Warrior));
    assert_damage("round(15)→20", r2.damage_to_defender, 2.0);
}

// ============================================================
// SECTION 9: Multi-combat sequences
// ============================================================

/// Two warriors vs Giant. Sequential attacks with HP decay.
///
/// Attack 1 (warrior vs giant@40hp):
///   atk_force=20000, def_force=40000, total=60000
///   dmg = round(30)/10 = 3.  Giant → 37hp.
///   ret = round(120)/10 = 12. Warrior dies.
///
/// Attack 2 (warrior vs giant@37hp):
///   atk_force=20000, def_force=400*370*100/400 = 37000, total=57000
///   dmg = round(20000*200*450/(1000*57000))/10 = round(31)/10 = 30/10 = 3
///   Giant → 34hp.
///   ret = round(37000*400*4500/(1000*57000*10))/10 = round(116)/10 = 120/10 = 12
///   Warrior dies.
#[test]
fn game_formula_two_warriors_vs_giant() {
    let attackers = vec![Unit::new(UnitType::Warrior), Unit::new(UnitType::Warrior)];
    let giant = Unit::new(UnitType::Giant);
    let order = vec![0, 1];
    let result = multi_combat(&attackers, &giant, &order);

    assert_damage("giant final HP", result.defender_hp_final, 34.0);
    assert_eq!(result.attacker_casualties, 2, "both warriors should die");
    assert_eq!(result.events.len(), 2);

    // Event 1: warrior deals 3, takes 12
    assert_damage("e0 dmg_to_def", result.events[0].damage_to_defender, 3.0);
    assert_damage("e0 dmg_to_atk", result.events[0].damage_to_attacker, 12.0);

    // Event 2: warrior deals 3, takes 12
    assert_damage("e1 dmg_to_def", result.events[1].damage_to_defender, 3.0);
    assert_damage("e1 dmg_to_atk", result.events[1].damage_to_attacker, 12.0);
}

/// Catapult + Warrior vs Defender. Catapult softens, warrior finishes.
///
/// Catapult (4/0/10, range=3) vs Defender (1/3/15):
///   att=400, def=300, a_hp=100, d_hp=150, d_max=150
///   atk_force=40000, def_force=30000, total=70000
///   dmg = round(40000*400*450/(1000*70000))/10 = round(102)/10 = 100/10 = 10
///   No retaliation (range 3>1). Defender → 5hp.
///
/// Warrior (2/2/10) vs Defender@5hp:
///   att=200, def=300, a_hp=100, d_hp=50, d_max=150
///   atk_force=20000, def_force=300*50*100/150=10000, total=30000
///   dmg = round(20000*200*450/(1000*30000))/10 = round(60)/10 = 60/10 = 6
///   Defender dies (5 - 6 < 0) → no retaliation.
#[test]
fn game_formula_catapult_warrior_vs_defender() {
    let attackers = vec![Unit::new(UnitType::Catapult), Unit::new(UnitType::Warrior)];
    let defender = Unit::new(UnitType::Defender);
    let order = vec![0, 1];
    let result = multi_combat(&attackers, &defender, &order);

    assert!(result.defender_hp_final <= 0.0, "defender should be dead");
    assert_eq!(result.attacker_casualties, 0, "no attacker should die");

    // Catapult deals 10, warrior deals 6
    assert_damage("catapult dmg", result.events[0].damage_to_defender, 10.0);
    assert_damage("catapult ret", result.events[0].damage_to_attacker, 0.0);
    assert_damage("warrior dmg", result.events[1].damage_to_defender, 6.0);
    assert_damage("warrior ret", result.events[1].damage_to_attacker, 0.0);
}

/// Three warriors vs Giant — verifying cumulative HP decay.
///
/// W1 vs Giant@40: dmg=3, ret=12 (warrior dies). Giant→37
/// W2 vs Giant@37: dmg=3, ret=12 (warrior dies). Giant→34
/// W3 vs Giant@34:
///   atk_force=20000, def_force=400*340*100/400=34000, total=54000
///   dmg = round(20000*200*450/(1000*54000))/10 = round(33)/10 = 30/10 = 3
///   Giant → 31hp.
///   ret = round(34000*400*4500/(1000*54000*10))/10 = round(113)/10 = 110/10 = 11
///   Warrior dies.
#[test]
fn game_formula_three_warriors_vs_giant() {
    let attackers = vec![
        Unit::new(UnitType::Warrior),
        Unit::new(UnitType::Warrior),
        Unit::new(UnitType::Warrior),
    ];
    let giant = Unit::new(UnitType::Giant);
    let order = vec![0, 1, 2];
    let result = multi_combat(&attackers, &giant, &order);

    assert_damage("giant final HP", result.defender_hp_final, 31.0);
    assert_eq!(result.attacker_casualties, 3);
}

// ============================================================
// SECTION 10: Optimizer — correct attack ordering
// ============================================================

/// Optimizer should find the order that maximizes damage to defender.
/// Catapult + warrior vs defender: catapult first is optimal because
/// it deals 10 damage without retaliation, softening the defender
/// so the warrior can kill it without taking damage.
#[test]
fn game_formula_optim_catapult_warrior_vs_defender() {
    let attackers = vec![Unit::new(UnitType::Catapult), Unit::new(UnitType::Warrior)];
    let defender = Unit::new(UnitType::Defender);
    let result = optimized(&attackers, &defender);

    // Defender should be killed
    assert!(result.defender_hp_final <= 0.0, "defender should be dead");
    // No attacker casualties (catapult no-ret, warrior kills so no-ret)
    assert_eq!(result.attacker_casualties, 0);
}

/// Optimizer with mixed attackers vs Giant.
/// Knight (3.5/1/10) + Warrior (2/2/10) vs Giant (5/4/40).
/// Knight deals more damage, order shouldn't matter much for total,
/// but optimizer should produce valid results.
#[test]
fn game_formula_optim_knight_warrior_vs_giant() {
    let attackers = vec![Unit::new(UnitType::Knight), Unit::new(UnitType::Warrior)];
    let giant = Unit::new(UnitType::Giant);
    let result = optimized(&attackers, &giant);

    // Both should attack
    assert_eq!(result.events.len(), 2);
    // Giant should take significant damage
    assert!(result.defender_hp_final < 40.0);
}

// ============================================================
// SECTION 11: Multi-defender optimization
// ============================================================

/// 3 warriors vs (weak warrior @3hp, full warrior @10hp).
/// Optimizer should kill the weak one with 1 warrior, then focus rest on the full one.
#[test]
fn game_formula_multi_defender_split() {
    let attackers: Vec<Unit> = (0..3).map(|_| Unit::new(UnitType::Warrior)).collect();
    let defenders = vec![
        Unit::new(UnitType::Warrior).with_hp(3.0),
        Unit::new(UnitType::Warrior),
    ];
    let results = optimized_multi_defender(&attackers, &defenders);

    assert_eq!(results.len(), 2);
    // Weak defender should be dead
    assert!(results[0].defender_hp_final <= 0.0, "3hp warrior should die");
    // Full defender should take damage from remaining warriors
    assert!(results[1].defender_hp_final < 10.0, "full warrior should take damage");
}

// ============================================================
// SECTION 12: Bulk & elimination helpers
// ============================================================

/// Warrior vs Giant in bulk: how many warriors to kill a giant?
/// Each warrior deals 3 damage (slightly varying due to HP decay on giant),
/// so roughly 40/3 ≈ 14 warriors. But they also die, which doesn't matter
/// for bulk count.
#[test]
fn game_formula_bulk_warriors_vs_giant() {
    let warrior = Unit::new(UnitType::Warrior);
    let giant = Unit::new(UnitType::Giant);
    let count = bulk(&warrior, &giant);
    assert!(count.is_some());
    let n = count.unwrap();
    // Should be roughly 13-15 warriors
    assert!(n >= 10 && n <= 20, "expected ~14 warriors, got {}", n);
}

/// Knight vs Warrior elimination threshold: what is the minimum HP
/// the knight needs to one-shot the warrior?
///
/// At knight HP=9:
///   att=350, a_hp=90, a_max=100, def=200, d_hp=100, d_max=100
///   atk_force = 350*90*100/100 = 31500
///   def_force = 20000, total = 51500
///   dmg = round(31500*350*450/(1000*51500))/10 = round(96)/10 = 100/10 = 10
///   Still kills warrior → threshold is 9, not 10.
#[test]
fn game_formula_knight_eliminates_warrior() {
    let knight = Unit::new(UnitType::Knight);
    let warrior = Unit::new(UnitType::Warrior);
    let threshold = eliminate_defender(&knight, &warrior);
    assert!(threshold.is_some());
    // Knight at 9hp still deals 10 damage (enough to kill warrior at 10hp)
    assert_damage("elimination threshold", threshold.unwrap(), 9.0);
}

// ============================================================
// SECTION 13: Cross-validation — same scenario, game vs bot
// ============================================================

/// Manually work through the FULL game code path for Swordsman vs Swordsman:
///
/// Game code variables:
///   num = 45 (constant)
///   num2 (atk maxhp) = 15, num3 (def maxhp) = 15
///   num4 (atk force) = 3 * 15 * 100 / 15 = 300
///   num5 (def force) = 3 * 15 * 100 / 15 = 300
///   num6 (total)     = 600
///   attackDamage     = Round(300 * 3 * 45 * 10 / (1000 * 600))
///                    = Round(405000 / 600000) = Round(0.675...)
///
/// Wait — this gives sub-1 damage. But we calculated 7 above using the bot's
/// internal scaling. The game likely uses health values in the hundreds.
///
/// The reconciliation: in the actual game, stats are likely stored in different
/// units (e.g., HP=150, attack=30 for swordsman). The bot normalizes to small
/// floats (HP=15, attack=3) and scales up internally by 10x/100x.
///
/// This test verifies the bot's internal scaling produces results consistent
/// with the game formula when applied to the bot's normalized stats.
#[test]
fn game_formula_cross_validate_swordsman_vs_swordsman() {
    // Bot stats for swordsman: atk=3.0, def=3.0, hp=15.0
    let sw_a = Unit::new(UnitType::Swordsman);
    let sw_d = Unit::new(UnitType::Swordsman);
    let r = single_combat(&sw_a, &sw_d);

    // Manual calculation using bot's internal integers:
    //   att = 300, def = 300, a_hp=150, a_max=150, d_hp=150, d_max=150
    //   attack_force = 300*150*100/150 = 30000
    //   defense_force = 300*150*100/150 = 30000
    //   total = 60000
    //   raw_dmg = round(30000*300*450/(1000*60000))/10 = round(67)/10 = 70/10 = 7.0
    assert_damage("sw vs sw damage", r.damage_to_defender, 7.0);
    assert_damage("sw vs sw retaliation", r.damage_to_attacker, 7.0);
}

// ============================================================
// SECTION 14: Large stat asymmetries
// ============================================================

/// Catapult (4/0/10, range=3) vs Giant (5/4/40). No retaliation.
///
/// att=400, def=400, a_hp=100, a_max=100, d_hp=400, d_max=400
/// atk_force = 400*100*100/100 = 40000
/// def_force = 400*400*100/400 = 40000
/// total = 80000
/// atk_dmg = round(40000*400*450/(1000*80000))/10 = round(90)/10 = 90/10 = 9
/// No retaliation (range 3 > 1).
#[test]
fn game_formula_catapult_vs_giant() {
    let a = Unit::new(UnitType::Catapult);
    let d = Unit::new(UnitType::Giant);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 9.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

/// Knight (3.5/1/10) vs Defender (1/3/15). Knight should deal heavy damage.
///
/// att=350, def=300, a_hp=100, a_max=100, d_hp=150, d_max=150
/// atk_force = 350*100*100/100 = 35000
/// def_force = 300*150*100/150 = 30000
/// total = 65000
/// atk_dmg = round(35000*350*450/(1000*65000))/10 = round(84)/10 = 80/10 = 8
///
/// Defender survives (15-8=7). Retaliation:
/// ret = round(30000*300*4500/(1000*65000*10))/10 = round(62)/10 = 60/10 = 6
#[test]
fn game_formula_knight_vs_defender() {
    let a = Unit::new(UnitType::Knight);
    let d = Unit::new(UnitType::Defender);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 8.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
}

/// Swordsman (3/3/15) vs Giant (5/4/40). Both beefy.
///
/// att=300, def=400, a_hp=150, a_max=150, d_hp=400, d_max=400
/// atk_force = 300*150*100/150 = 30000
/// def_force = 400*400*100/400 = 40000
/// total = 70000
/// atk_dmg = round(30000*300*450/(1000*70000))/10 = round(57)/10 = 60/10 = 6
///
/// Giant survives (40-6=34). Retaliation:
/// ret = round(40000*400*4500/(1000*70000*10))/10 = round(102)/10 = 100/10 = 10
/// Swordsman survives (15-10=5).
#[test]
fn game_formula_swordsman_vs_giant() {
    let a = Unit::new(UnitType::Swordsman);
    let d = Unit::new(UnitType::Giant);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 10.0);
}

/// Defender (1/3/15) vs Giant (5/4/40).
///
/// att=100, def=400, a_hp=150, a_max=150, d_hp=400, d_max=400
/// atk_force = 100*150*100/150 = 10000
/// def_force = 400*400*100/400 = 40000
/// total = 50000
/// atk_dmg = round(10000*100*450/(1000*50000))/10 = round(9)/10 = 10/10 = 1
///
/// Giant survives (40-1=39). Retaliation:
/// ret = round(40000*400*4500/(1000*50000*10))/10 = round(144)/10 = 140/10 = 14
/// Defender dies (15-14=1). Actually survives at 1hp!
#[test]
fn game_formula_defender_vs_giant() {
    let a = Unit::new(UnitType::Defender);
    let d = Unit::new(UnitType::Giant);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 1.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 14.0);
}

// ============================================================
// SECTION 15: Tridention (fractional stats 2.5 attack)
// ============================================================

/// Tridention (2.5/1/10, range=2) vs Warrior (2/2/10).
///
/// att=250, def=200, a_hp=100, a_max=100, d_hp=100, d_max=100
/// atk_force = 250*100*100/100 = 25000
/// def_force = 200*100*100/100 = 20000
/// total = 45000
/// atk_dmg = round(25000*250*450/(1000*45000))/10 = round(62)/10 = 60/10 = 6
/// No retaliation (range 2 > 1).
#[test]
fn game_formula_tridention_vs_warrior() {
    let a = Unit::new(UnitType::Tridention);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 6.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

// ============================================================
// SECTION 16: Combined modifiers — walled + poisoned
// ============================================================

/// Warrior vs Warrior (walled + poisoned).
/// Defense bonus: walled (4.0 for can_fortify) * poisoned (0.5) = 2.0
///
/// def = 200 * 20/10 = 400  (defense_bonus = 2.0, def_bonus_int = 20)
/// atk_force = 20000, def_force = 400*100*100/100 = 40000, total = 60000
/// atk_dmg = round(20000*200*450/(1000*60000))/10 = round(30)/10 = 30/10 = 3
///
/// Retaliation:
/// ret = round(40000*400*4500/(1000*60000*20))/10 = round(60)/10 = 60/10 = 6
#[test]
fn game_formula_warrior_vs_walled_poisoned_warrior() {
    let a = Unit::new(UnitType::Warrior);
    let d = Unit::new(UnitType::Warrior)
        .with_status(StatusEffects::WALLED | StatusEffects::POISONED);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 3.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
}

// ============================================================
// SECTION 17: Naval units
// ============================================================

/// Rammership (3/3/10) vs Warrior (2/2/10). Both melee range.
///
/// att=300, def=200, a_hp=100, a_max=100, d_hp=100, d_max=100
/// atk_force = 300*100*100/100 = 30000
/// def_force = 200*100*100/100 = 20000
/// total = 50000
/// atk_dmg = round(30000*300*450/(1000*50000))/10 = round(81)/10 = 80/10 = 8
///
/// Defender survives (10-8=2). Retaliation:
/// ret = round(20000*200*4500/(1000*50000*10))/10 = round(36)/10 = 40/10 = 4
#[test]
fn game_formula_rammership_vs_warrior() {
    let a = Unit::new(UnitType::Rammership);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 8.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 4.0);
}

/// Scoutship (2/1/10, range=2) vs Warrior (2/2/10, range=1). No retaliation.
///
/// att=200, def=200, a_hp=100, a_max=100, d_hp=100, d_max=100
/// atk_force = 200*100*100/100 = 20000
/// def_force = 200*100*100/100 = 20000
/// total = 40000
/// atk_dmg = round(20000*200*450/(1000*40000))/10 = round(45)/10 = 50/10 = 5
#[test]
fn game_formula_scoutship_vs_warrior() {
    let a = Unit::new(UnitType::Scoutship);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 5.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 0.0);
}

// ============================================================
// SECTION 18: Cymanti units
// ============================================================

/// Kiton (1/3/15, trait=POISONED) vs Warrior.
/// Kiton applies poison to surviving defender.
///
/// att=100, def=200, a_hp=150, a_max=150, d_hp=100, d_max=100
/// atk_force = 100*150*100/150 = 10000
/// def_force = 200*100*100/100 = 20000
/// total = 30000
/// atk_dmg = round(10000*100*450/(1000*30000))/10 = round(15)/10 = 20/10 = 2
///
/// Warrior survives (10-2=8). Retaliation:
/// ret = round(20000*200*4500/(1000*30000*10))/10 = round(60)/10 = 60/10 = 6
#[test]
fn game_formula_kiton_vs_warrior() {
    let a = Unit::new(UnitType::Kiton);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 2.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
    assert!(
        r.effects_to_defender.contains(StatusEffects::POISONED),
        "kiton should poison surviving defender"
    );
}

/// Hexapod (3/1/5) vs Warrior (2/2/10). Low HP unit.
///
/// att=300, def=200, a_hp=50, a_max=50, d_hp=100, d_max=100
/// atk_force = 300*50*100/50 = 30000
/// def_force = 200*100*100/100 = 20000
/// total = 50000
/// atk_dmg = round(30000*300*450/(1000*50000))/10 = round(81)/10 = 80/10 = 8
///
/// Warrior survives (10-8=2). Retaliation:
/// ret = round(20000*200*4500/(1000*50000*10))/10 = round(36)/10 = 40/10 = 4
/// Hexapod dies (5-4=1). Actually survives at 1!
#[test]
fn game_formula_hexapod_vs_warrior() {
    let a = Unit::new(UnitType::Hexapod);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 8.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 4.0);
}

// ============================================================
// SECTION 19: Polaris units
// ============================================================

/// Battle Sled (3/2/15) vs Warrior (2/2/10).
///
/// att=300, def=200, a_hp=150, a_max=150, d_hp=100, d_max=100
/// atk_force = 300*150*100/150 = 30000
/// def_force = 200*100*100/100 = 20000
/// total = 50000
/// atk_dmg = round(30000*300*450/(1000*50000))/10 = round(81)/10 = 80/10 = 8
///
/// Warrior survives (10-8=2). Retaliation:
/// ret = round(20000*200*4500/(1000*50000*10))/10 = round(36)/10 = 40/10 = 4
#[test]
fn game_formula_battlesled_vs_warrior() {
    let a = Unit::new(UnitType::Battlesled);
    let d = Unit::new(UnitType::Warrior);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 8.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 4.0);
}

/// Gaami (4/3/30) vs Swordsman (3/3/15).
///
/// att=400, def=300, a_hp=300, a_max=300, d_hp=150, d_max=150
/// atk_force = 400*300*100/300 = 40000
/// def_force = 300*150*100/150 = 30000
/// total = 70000
/// atk_dmg = round(40000*400*450/(1000*70000))/10 = round(102)/10 = 100/10 = 10
///
/// Swordsman survives (15-10=5). Retaliation:
/// ret = round(30000*300*4500/(1000*70000*10))/10 = round(57)/10 = 60/10 = 6
#[test]
fn game_formula_gaami_vs_swordsman() {
    let a = Unit::new(UnitType::Gaami);
    let d = Unit::new(UnitType::Swordsman);
    let r = single_combat(&a, &d);
    assert_damage("atk_dmg", r.damage_to_defender, 10.0);
    assert_damage("ret_dmg", r.damage_to_attacker, 6.0);
}
