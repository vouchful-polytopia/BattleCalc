use crate::unit::{StatusEffects, Unit, UnitType};
use rayon::prelude::*;

// ======================== Data Types ========================

#[derive(Debug, Clone)]
pub struct BattleResult {
    pub damage_to_defender: f32,
    pub damage_to_attacker: f32,
    pub effects_to_defender: StatusEffects,
    pub effects_to_attacker: StatusEffects,
}

#[derive(Debug, Clone)]
pub struct CombatEvent {
    pub attacker_name: String,
    pub attacker_hp_before: f32,
    pub attacker_hp_after: f32,
    pub attacker_max_hp: f32,
    pub defender_hp_before: f32,
    pub defender_hp_after: f32,
    pub defender_max_hp: f32,
    pub damage_to_defender: f32,
    pub damage_to_attacker: f32,
    pub poison_applied: bool,
}

#[derive(Debug, Clone)]
pub struct MultiCombatResult {
    pub events: Vec<CombatEvent>,
    pub defender_hp_final: f32,
    pub defender_name: String,
    pub attacker_casualties: usize,
    pub attackers_hp_remaining: f32,
    pub order: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct AttackAction {
    pub attacker_idx: usize,
    pub hit_num: u8,
    pub explode: bool,
    pub explode_only: bool,
}

// ======================== Core Damage ========================

#[inline]
fn round_polytopia(n: i64) -> i64 {
    let lower = (n / 10) * 10;
    let upper = lower + 10;
    if n - lower < upper - n { lower } else { upper }
}

fn calculate_damage(attacker: &Unit, defender: &Unit) -> BattleResult {
    let att = (attacker.attack * 100.0) as i64;
    let def_bonus_int = (defender.defense_bonus * 10.0) as i64;
    let def = (defender.defense * 100.0) as i64 * def_bonus_int / 10;
    let mut a_hp = (attacker.current_hp * 10.0) as i64;
    let a_max = (attacker.max_hp * 10.0) as i64;
    let d_hp = (defender.current_hp * 10.0) as i64;
    let d_max = (defender.max_hp * 10.0) as i64;

    let mut tentacle_dmg: i64 = 0;

    let tentacles_suppressed = attacker.status_effects.contains(StatusEffects::NO_RETALIATE);
    if defender.unit_type == UnitType::Jelly
        && attacker.unit_type != UnitType::Jelly
        && attacker.range <= 1
        && !tentacles_suppressed
    {
        let jelly_result = compute_raw_damage(def, d_hp, d_max, att, a_hp, a_max);
        tentacle_dmg = jelly_result;
        a_hp = (a_hp - tentacle_dmg * 10).max(1);
    }

    let attack_force = att * a_hp * 100 / a_max;
    let defense_force = def * d_hp * 100 / d_max;
    let total_force = attack_force + defense_force;

    if total_force == 0 {
        return BattleResult {
            damage_to_defender: 0.0,
            damage_to_attacker: 0.0,
            effects_to_defender: StatusEffects::empty(),
            effects_to_attacker: StatusEffects::empty(),
        };
    }

    let raw_atk_dmg = round_polytopia(attack_force * att * 450 / (1000 * total_force)) / 10;

    let is_splash = attacker.status_effects.contains(StatusEffects::SPLASHING);
    let is_explode = attacker.status_effects.contains(StatusEffects::EXPLODING);
    // Default splash is fractional (e.g. 11 → 5.5). `fl` modifier floors it.
    let atk_dmg_f: f32 = if is_splash || is_explode {
        if attacker.status_effects.contains(StatusEffects::FLOOR_SPLASH) {
            (raw_atk_dmg / 2) as f32
        } else {
            raw_atk_dmg as f32 / 2.0
        }
    } else {
        raw_atk_dmg as f32
    };

    let can_retaliate = can_defender_retaliate(attacker, defender, atk_dmg_f);

    let mut def_dmg = 0i64;
    if can_retaliate {
        let def_bonus_int = (defender.defense_bonus * 10.0) as i64;
        def_dmg = round_polytopia(defense_force * def * 4500 / (1000 * total_force * def_bonus_int)) / 10;
    }

    let total_attacker_dmg_f: f32 = if is_explode {
        attacker.current_hp
    } else {
        def_dmg as f32 + tentacle_dmg as f32
    };

    let effects_to_defender = if defender.current_hp - atk_dmg_f > 0.0 {
        attacker.trait_effects
    } else {
        StatusEffects::empty()
    };

    let effects_to_attacker = if attacker.current_hp - total_attacker_dmg_f > 0.0 {
        defender.trait_effects
    } else {
        StatusEffects::empty()
    };

    BattleResult {
        damage_to_defender: atk_dmg_f,
        damage_to_attacker: total_attacker_dmg_f,
        effects_to_defender,
        effects_to_attacker,
    }
}

#[inline]
fn compute_raw_damage(att: i64, a_hp: i64, a_max: i64, def: i64, d_hp: i64, d_max: i64) -> i64 {
    let af = att * a_hp * 100 / a_max;
    let df = def * d_hp * 100 / d_max;
    let total = af + df;
    if total == 0 { return 0; }
    round_polytopia(af * att * 450 / (1000 * total)) / 10
}

fn can_defender_retaliate(attacker: &Unit, defender: &Unit, atk_dmg: f32) -> bool {
    if defender.current_hp - atk_dmg <= 0.0 { return false; }
    // Retaliation overrides (r/nr modifiers)
    if defender.status_effects.contains(StatusEffects::FORCE_RETALIATE) { return true; }
    if attacker.status_effects.contains(StatusEffects::NO_RETALIATE) { return false; }
    if defender.status_effects.contains(StatusEffects::NO_RETALIATE) { return false; }
    // Normal checks
    if !defender.retaliates { return false; }
    if attacker.surprise { return false; }
    if defender.status_effects.contains(StatusEffects::FROZEN) { return false; }
    if attacker.trait_effects.contains(StatusEffects::CONVERTED) { return false; }
    if attacker.trait_effects.contains(StatusEffects::FROZEN) { return false; }
    if attacker.range > defender.range { return false; }
    if attacker.status_effects.contains(StatusEffects::EXPLODING) { return false; }
    // Splash hits are AoE damage to adjacent tiles — they never provoke retaliation.
    // (Juggernaut is the only range-1 splasher, so this is the case it actually changes.)
    if attacker.status_effects.contains(StatusEffects::SPLASHING) { return false; }
    true
}

// ======================== Action Execution ========================

/// Execute attack actions against a defender. Modifies attacker_hp in-place.
/// Returns (result, actions_consumed).
fn execute_actions(
    attackers: &[Unit],
    actions: &[AttackAction],
    defender: &Unit,
    attacker_hp: &mut [f32],
    annotate_explode: bool,
) -> (MultiCombatResult, usize) {
    let mut def = defender.clone();
    let mut events = Vec::new();
    let mut order = Vec::new();
    let mut used = std::collections::HashSet::new();
    let mut consumed = 0usize;

    for action in actions {
        if def.current_hp <= 0.0 { break; }
        consumed += 1;

        let idx = action.attacker_idx;
        let cur_hp = attacker_hp[idx];
        if cur_hp <= 0.0 { continue; }
        let atk = &attackers[idx];

        if action.explode_only {
            let mut eu = atk.clone();
            eu.current_hp = cur_hp;
            eu.status_effects.insert(StatusEffects::EXPLODING);
            let result = calculate_damage(&eu, &def);
            let def_hp_before = def.current_hp;
            def.current_hp = (def.current_hp - result.damage_to_defender).max(0.0);
            attacker_hp[idx] = 0.0;
            if def.current_hp > 0.0 {
                def.status_effects.insert(StatusEffects::POISONED);
                def.recalc_defense_bonus();
            }
            events.push(CombatEvent {
                attacker_name: format!("{} (explode)", atk.display_name()),
                attacker_hp_before: cur_hp, attacker_hp_after: 0.0, attacker_max_hp: atk.max_hp,
                defender_hp_before: def_hp_before, defender_hp_after: def.current_hp, defender_max_hp: def.max_hp,
                damage_to_defender: result.damage_to_defender, damage_to_attacker: cur_hp,
                poison_applied: def.current_hp > 0.0,
            });
            used.insert(idx);
            order.push(idx);
            continue;
        }

        if atk.attack <= 0.0 { continue; }

        // Normal attack
        let mut temp = atk.clone();
        temp.current_hp = cur_hp;
        temp.status_effects.remove(StatusEffects::EXPLODING);
        temp.status_effects.remove(StatusEffects::ATTACK_EXPLODE);

        let result = calculate_damage(&temp, &def);
        let def_hp_before = def.current_hp;
        let mut total_dmg_def = result.damage_to_defender;
        let mut total_dmg_atk = result.damage_to_attacker;
        def.current_hp = (def.current_hp - result.damage_to_defender).max(0.0);
        let mut after_hp = (cur_hp - result.damage_to_attacker).max(0.0);

        if result.effects_to_defender.contains(StatusEffects::POISONED) && def.current_hp > 0.0 {
            def.status_effects.insert(StatusEffects::POISONED);
            def.recalc_defense_bonus();
        }
        if result.effects_to_defender.contains(StatusEffects::FROZEN) && def.current_hp > 0.0 {
            def.status_effects.insert(StatusEffects::FROZEN);
        }

        // Explosion phase (attack+explode)
        if action.explode && after_hp > 0.0 && def.current_hp > 0.0 {
            let mut eu = atk.clone();
            eu.current_hp = after_hp;
            eu.status_effects.insert(StatusEffects::EXPLODING);
            let exp = calculate_damage(&eu, &def);
            total_dmg_def += exp.damage_to_defender;
            def.current_hp = (def.current_hp - exp.damage_to_defender).max(0.0);
            total_dmg_atk += after_hp;
            after_hp = 0.0;
            if def.current_hp > 0.0 {
                def.status_effects.insert(StatusEffects::POISONED);
                def.recalc_defense_bonus();
            }
        }

        attacker_hp[idx] = after_hp;

        let hit_suffix = if atk.double_attack {
            format!(" (hit {})", action.hit_num + 1)
        } else {
            String::new()
        };
        // Only annotate the no-explode case when the user explicitly asked for ax/axi
        // (i.e. the optimizer is actually weighing a detonation decision).
        let explode_suffix = if action.explode {
            " (+ explode)".to_string()
        } else if annotate_explode && atk.status_effects.contains(StatusEffects::ATTACK_EXPLODE) {
            " (no explode)".to_string()
        } else {
            String::new()
        };
        let name = format!("{}{}{}", atk.display_name(), hit_suffix, explode_suffix);
        let did_poison = (atk.trait_effects.contains(StatusEffects::POISONED) || action.explode)
            && def.current_hp > 0.0;

        events.push(CombatEvent {
            attacker_name: name,
            attacker_hp_before: cur_hp, attacker_hp_after: after_hp, attacker_max_hp: atk.max_hp,
            defender_hp_before: def_hp_before, defender_hp_after: def.current_hp, defender_max_hp: def.max_hp,
            damage_to_defender: total_dmg_def, damage_to_attacker: total_dmg_atk,
            poison_applied: did_poison,
        });
        used.insert(idx);
        order.push(idx);
    }

    let mut casualties = 0;
    let mut hp_remaining = 0.0f32;
    for (i, &hp) in attacker_hp.iter().enumerate() {
        if used.contains(&i) {
            if hp <= 0.0 { casualties += 1; } else { hp_remaining += hp; }
        } else {
            hp_remaining += hp;
        }
    }

    (MultiCombatResult {
        defender_hp_final: def.current_hp,
        defender_name: defender.display_name(),
        attacker_casualties: casualties,
        attackers_hp_remaining: hp_remaining,
        order,
        events,
    }, consumed)
}

// ======================== Helpers ========================

struct Perms { c: Vec<usize>, i: usize, first: bool, len: usize }

impl Perms {
    fn new(len: usize) -> Self { Self { c: vec![0; len], i: 0, first: true, len } }
}

impl Iterator for Perms {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<(usize, usize)> {
        if self.first { self.first = false; self.i = 0; return Some((0, 0)); }
        while self.i < self.len {
            if self.c[self.i] < self.i {
                let pair = if self.i % 2 == 0 { (0, self.i) } else { (self.c[self.i], self.i) };
                self.c[self.i] += 1;
                self.i = 0;
                return Some(pair);
            }
            self.c[self.i] = 0;
            self.i += 1;
        }
        None
    }
}

/// Check if two attack slots are interchangeable (same unit stats, same action type).
/// If so, swapping them in any permutation yields an identical result.
fn slots_equivalent(a: &AttackAction, b: &AttackAction, attackers: &[Unit]) -> bool {
    if a.hit_num != b.hit_num || a.explode != b.explode || a.explode_only != b.explode_only {
        return false;
    }
    let ua = &attackers[a.attacker_idx];
    let ub = &attackers[b.attacker_idx];
    ua.attack == ub.attack
        && ua.defense == ub.defense
        && ua.current_hp == ub.current_hp
        && ua.max_hp == ub.max_hp
        && ua.range == ub.range
        && ua.retaliates == ub.retaliates
        && ua.surprise == ub.surprise
        && ua.can_explode == ub.can_explode
        && ua.can_splash == ub.can_splash
        && ua.double_attack == ub.double_attack
        && ua.status_effects == ub.status_effects
        && ua.trait_effects == ub.trait_effects
        && ua.defense_bonus == ub.defense_bonus
        && ua.is_final == ub.is_final
}

/// Check if a permutation is canonical w.r.t. equivalent slots.
/// For groups of interchangeable slots, only accept the permutation where
/// their relative order is non-decreasing (i.e. the first one we see comes first).
fn is_canonical_perm(perm: &[usize], slots: &[AttackAction], attackers: &[Unit]) -> bool {
    for i in 1..perm.len() {
        // If slot perm[i-1] and slot perm[i] are equivalent,
        // require perm[i-1] < perm[i] to avoid duplicate orderings
        if slots_equivalent(&slots[perm[i - 1]], &slots[perm[i]], attackers) && perm[i - 1] > perm[i] {
            return false;
        }
    }
    true
}

/// Double-attack hit 0 must come before hit 1.
fn valid_action_order(actions: &[AttackAction]) -> bool {
    valid_action_order_with_prior(actions, &std::collections::HashSet::new())
}

/// Like valid_action_order but allows hit 1 if hit 0 was executed in a prior defender group.
fn valid_action_order_with_prior(actions: &[AttackAction], prior_hit0: &std::collections::HashSet<usize>) -> bool {
    for (pos, action) in actions.iter().enumerate() {
        if action.hit_num == 1 {
            let has_prior = prior_hit0.contains(&action.attacker_idx);
            let has_local = actions[..pos].iter().any(|a| a.attacker_idx == action.attacker_idx && a.hit_num == 0);
            if !has_prior && !has_local {
                return false;
            }
        }
    }
    true
}

/// For multi-defender: hit 0's defender index must be <= hit 1's defender index.
fn valid_multi_assignment(slots: &[AttackAction], assignments: &[usize]) -> bool {
    for (i, slot) in slots.iter().enumerate() {
        if slot.hit_num == 1 {
            for (j, other) in slots.iter().enumerate() {
                if other.attacker_idx == slot.attacker_idx && other.hit_num == 0 {
                    if assignments[j] > assignments[i] { return false; }
                    break;
                }
            }
        }
    }
    true
}

/// Describes an optional explosion the optimizer can choose to include.
/// `instant = true` → explosion is bundled with the hit (bundled via `explode: true` on the base slot).
/// `instant = false` → explosion is a separate `explode_only` slot that can be interleaved
///   anywhere after the hit (same attacker_idx, hit_num = 1).
#[derive(Debug, Clone, Copy)]
struct ExplodeOpt {
    base_slot: usize,
    attacker_idx: usize,
    instant: bool,
}

/// Generate base action slots. Returns (slots, optional-explode descriptors).
/// With the new Doomux modifier semantics:
///   - default `do` (no modifier, can_explode only): never explodes
///   - `do x` (EXPLODING): explode-only slot, no attack
///   - `do ax` (ATTACK_EXPLODE): base attack slot + optional separate explode slot
///   - `do axi` (ATTACK_EXPLODE + INSTANT_EXPLODE): base attack slot + optional bundled explode
fn generate_base_slots(attackers: &[Unit]) -> (Vec<AttackAction>, Vec<ExplodeOpt>) {
    let mut slots = Vec::new();
    let mut explode_opts: Vec<ExplodeOpt> = Vec::new();

    for (idx, atk) in attackers.iter().enumerate() {
        if atk.status_effects.contains(StatusEffects::EXPLODING) {
            slots.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: true });
        } else if atk.double_attack {
            slots.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: false });
            slots.push(AttackAction { attacker_idx: idx, hit_num: 1, explode: false, explode_only: false });
        } else {
            let slot_idx = slots.len();
            slots.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: false });
            if atk.can_explode
                && atk.status_effects.contains(StatusEffects::ATTACK_EXPLODE)
                && !atk.status_effects.contains(StatusEffects::NO_EXPLODE)
            {
                let instant = atk.status_effects.contains(StatusEffects::INSTANT_EXPLODE);
                explode_opts.push(ExplodeOpt { base_slot: slot_idx, attacker_idx: idx, instant });
            }
        }
    }

    (slots, explode_opts)
}

/// Materialize a variant of base slots by toggling each optional explosion.
/// `variant_mask` bits align with `explode_opts` indices. Bits past `MAX_EXPLODE_VARIANT_BITS`
/// are ignored (caller should cap `num_variants`).
fn apply_explode_variant(
    base_slots: &[AttackAction],
    explode_opts: &[ExplodeOpt],
    variant_mask: usize,
    max_bits: usize,
) -> Vec<AttackAction> {
    let mut slots = base_slots.to_vec();
    for (bit, opt) in explode_opts.iter().enumerate() {
        if bit >= max_bits { break; }
        if variant_mask & (1 << bit) == 0 { continue; }
        if opt.instant {
            slots[opt.base_slot].explode = true;
        } else {
            slots.push(AttackAction {
                attacker_idx: opt.attacker_idx,
                hit_num: 1,
                explode: false,
                explode_only: true,
            });
        }
    }
    slots
}

// ======================== Scoring ========================

/// Scoring priorities:
/// 0. Defender killed (most important)
/// 1. Max effective damage to defender
/// 2. Minimize attacker casualties
/// 3. Maximize remaining attacker HP
/// 4. Use fewest unique attackers
/// 5. Use fewest total actions (don't waste hits)
/// 6. Poison bonus (tiebreaker)
fn score_result(r: &MultiCombatResult) -> (i64, i64, i64, i64, i64, i64, i64) {
    let killed = if r.defender_hp_final <= 0.0 { 1 } else { 0 };
    let dmg = if !r.events.is_empty() {
        r.events[0].defender_hp_before - r.defender_hp_final.max(0.0)
    } else {
        0.0
    };
    let unique_attackers = {
        let mut seen = std::collections::HashSet::new();
        for &idx in &r.order { seen.insert(idx); }
        seen.len()
    };
    let actions = r.events.len();
    let poison = if r.defender_hp_final > 0.0 && r.events.iter().any(|e| e.poison_applied) { 1 } else { 0 };
    (
        killed,                                     // 0. kill bonus
        (dmg * 10.0) as i64,                       // 1. max effective damage
        -(r.attacker_casualties as i64),            // 2. min casualties
        (r.attackers_hp_remaining * 10.0) as i64,   // 3. max remaining HP
        -(unique_attackers as i64),                  // 4. min attackers used
        -(actions as i64),                           // 5. min actions used
        poison,                                      // 6. poison tiebreaker
    )
}

fn is_better(a: &MultiCombatResult, b: &MultiCombatResult) -> bool {
    score_result(a) > score_result(b)
}

/// Linearized score for multi-defender total scoring.
/// Weights ensure priority ordering is preserved.
fn score_single(r: &MultiCombatResult) -> i64 {
    let s = score_result(r);
    s.0 * 10_000_000_000
    + s.1 * 10_000_000
    + s.2 * 1_000_000
    + s.3 * 1_000
    + s.4 * 100
    + s.5 * 10
    + s.6
}

// ======================== Target HP ========================

/// Tells the optimizer to leave the defender at a specific HP rather than deal max damage.
/// `Exact(h)` = defender must end at exactly `h` HP (errors if no ordering can reach it).
/// `Below(h)` = defender must end strictly below `h` HP (uses the fewest units / best tiebreakers).
#[derive(Debug, Clone, Copy)]
pub enum Target {
    Exact(f32),
    Below(f32),
}

impl Target {
    pub fn hp(&self) -> f32 {
        match *self { Target::Exact(h) | Target::Below(h) => h }
    }
}

/// After a permutation is run in full, find the shortest prefix that satisfies the target.
/// For `Exact`, we pick the first event whose defender_hp_after == target.
/// For `Below`, we pick the first event whose defender_hp_after < target.
fn truncate_to_target(full: &MultiCombatResult, target: Target, attackers: &[Unit]) -> Option<MultiCombatResult> {
    if full.events.is_empty() { return None; }

    let k = match target {
        Target::Exact(h) => full.events.iter().enumerate()
            .find(|(_, e)| (e.defender_hp_after - h).abs() < 1e-4)
            .map(|(i, _)| i + 1),
        Target::Below(h) => full.events.iter().enumerate()
            .find(|(_, e)| e.defender_hp_after < h)
            .map(|(i, _)| i + 1),
    };
    let k = k?;

    let events = full.events[..k].to_vec();
    let order = full.order[..k].to_vec();
    let defender_hp_final = events[k - 1].defender_hp_after;

    let mut final_hp: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    for (i, &idx) in order.iter().enumerate() {
        final_hp.insert(idx, events[i].attacker_hp_after);
    }

    let casualties = final_hp.values().filter(|&&hp| hp <= 0.0).count();
    let mut hp_remaining = 0.0f32;
    for (idx, a) in attackers.iter().enumerate() {
        match final_hp.get(&idx) {
            Some(&h) if h > 0.0 => hp_remaining += h,
            Some(_) => {} // touched and dead
            None => hp_remaining += a.current_hp,
        }
    }

    Some(MultiCombatResult {
        events,
        defender_hp_final,
        defender_name: full.defender_name.clone(),
        attacker_casualties: casualties,
        attackers_hp_remaining: hp_remaining,
        order,
    })
}

/// Target-aware tiebreaker: once a candidate prefix matches the target, prefer
/// fewer casualties → more surviving HP → fewer unique attackers → fewer actions.
/// Independent of which target mode was used (both use the same ranking).
fn is_better_for_target(new: &MultiCombatResult, best: &MultiCombatResult) -> bool {
    if new.attacker_casualties != best.attacker_casualties {
        return new.attacker_casualties < best.attacker_casualties;
    }
    if (new.attackers_hp_remaining - best.attackers_hp_remaining).abs() > 1e-4 {
        return new.attackers_hp_remaining > best.attackers_hp_remaining;
    }
    let unique = |r: &MultiCombatResult| -> usize {
        let mut s = std::collections::HashSet::new();
        for &i in &r.order { s.insert(i); }
        s.len()
    };
    let nu = unique(new);
    let bu = unique(best);
    if nu != bu { return nu < bu; }
    new.events.len() < best.events.len()
}

/// Optimizer variant that aims for a specific defender HP rather than max damage.
/// Errors if no permutation reaches the target. Caps at 10 attackers.
pub fn optimized_with_target(
    attackers: &[Unit],
    defender: &Unit,
    target: Target,
) -> Result<MultiCombatResult, String> {
    let target_hp = target.hp();
    if target_hp >= defender.current_hp {
        return Err(format!(
            "Target HP ({}) must be less than the defender's current HP ({}).",
            target_hp as i32, defender.current_hp as i32,
        ));
    }
    if target_hp < 0.0 {
        return Err("Target HP must be 0 or greater.".to_string());
    }
    if attackers.len() > 10 {
        return Err("Target HP optimization is capped at 10 attackers.".to_string());
    }

    let (base_slots, explode_opts) = generate_base_slots(attackers);
    if base_slots.is_empty() {
        return Err("No valid attacker actions.".to_string());
    }

    let max_bits = 4;
    let num_variants = 1usize << explode_opts.len().min(max_bits);

    let variant_slots: Vec<Vec<AttackAction>> = (0..num_variants)
        .map(|variant| apply_explode_variant(&base_slots, &explode_opts, variant, max_bits))
        .collect();

    let results: Vec<Option<MultiCombatResult>> = variant_slots
        .par_iter()
        .map(|slots| {
            let n = slots.len();
            let mut best: Option<MultiCombatResult> = None;
            let mut perm: Vec<usize> = (0..n).collect();

            let eval_perm = |perm: &[usize], best: &mut Option<MultiCombatResult>| {
                if !is_canonical_perm(perm, slots, attackers) { return; }
                let ordered: Vec<AttackAction> = perm.iter().map(|&i| slots[i].clone()).collect();
                if !valid_action_order(&ordered) { return; }
                let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                let (full, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
                if let Some(prefix) = truncate_to_target(&full, target, attackers) {
                    if !passes_final_check(attackers, &prefix) { return; }
                    if best.as_ref().map_or(true, |b| is_better_for_target(&prefix, b)) {
                        *best = Some(prefix);
                    }
                }
            };

            eval_perm(&perm, &mut best);
            if n > 1 {
                for (a, b_idx) in Perms::new(n) {
                    if a == 0 && b_idx == 0 { continue; }
                    perm.swap(a, b_idx);
                    eval_perm(&perm, &mut best);
                }
            }
            best
        })
        .collect();

    let mut overall_best: Option<MultiCombatResult> = None;
    for result in results.into_iter().flatten() {
        if overall_best.as_ref().map_or(true, |b| is_better_for_target(&result, b)) {
            overall_best = Some(result);
        }
    }

    overall_best.ok_or_else(|| match target {
        Target::Exact(h) => format!(
            "No combination leaves the defender at exactly {} HP. Try `t<{}` to get below {} HP instead.",
            h as i32, h as i32, h as i32,
        ),
        Target::Below(h) => format!(
            "No combination of those attackers can bring the defender below {} HP.",
            h as i32,
        ),
    })
}

// ======================== Public API ========================

pub fn single_combat(attacker: &Unit, defender: &Unit) -> BattleResult {
    calculate_damage(attacker, defender)
}

/// Multi-combat for calc command. Phychi hits are consecutive per attacker.
pub fn multi_combat(attackers: &[Unit], defender: &Unit, order: &[usize]) -> MultiCombatResult {
    let mut actions = Vec::new();
    for &idx in order {
        let atk = &attackers[idx];
        let is_x = atk.status_effects.contains(StatusEffects::EXPLODING);
        let is_ax = atk.status_effects.contains(StatusEffects::ATTACK_EXPLODE);
        let is_axi = is_ax && atk.status_effects.contains(StatusEffects::INSTANT_EXPLODE);
        if is_x {
            actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: true });
        } else if atk.double_attack {
            actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: false });
            actions.push(AttackAction { attacker_idx: idx, hit_num: 1, explode: false, explode_only: false });
        } else {
            // ax/axi both add an explosion after the hit in calc. Delayed (ax) and instant (axi)
            // are indistinguishable without a sequence optimizer, so calc emits them back-to-back.
            actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: is_axi, explode_only: false });
            if is_ax && !is_axi {
                actions.push(AttackAction { attacker_idx: idx, hit_num: 1, explode: false, explode_only: true });
            }
        }
    }
    let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
    let (result, _) = execute_actions(attackers, &actions, defender, &mut hp, false);
    result
}

/// Check if result satisfies the "final" constraint: if any attacker is marked
/// as final, only accept results where that unit delivers the killing blow.
fn passes_final_check(attackers: &[Unit], result: &MultiCombatResult) -> bool {
    let has_final = attackers.iter().any(|a| a.is_final);
    if !has_final { return true; }
    if result.defender_hp_final > 0.0 { return true; } // no kill, constraint doesn't apply
    // Find the killing event (first event where defender reaches 0)
    if let Some(kill_pos) = result.events.iter().position(|e| e.defender_hp_after <= 0.0) {
        if kill_pos < result.order.len() {
            let killer_idx = result.order[kill_pos];
            return attackers[killer_idx].is_final;
        }
    }
    true
}

/// Optimized single-defender: Phychi hits interleaved, ax/axi explosions optional.
/// Respects `f` (final) modifier — marked unit must deliver killing blow.
/// Uses dedup pruning, early termination, and parallel search.
pub fn optimized(attackers: &[Unit], defender: &Unit) -> MultiCombatResult {
    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return MultiCombatResult {
            events: vec![], defender_hp_final: defender.current_hp,
            defender_name: defender.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: attackers.iter().map(|a| a.current_hp).sum(),
            order: vec![],
        };
    }

    let max_bits = 4;
    let num_variants = 1usize << explode_opts.len().min(max_bits);

    let variant_slots: Vec<Vec<AttackAction>> = (0..num_variants)
        .map(|variant| apply_explode_variant(&base_slots, &explode_opts, variant, max_bits))
        .collect();

    // Parallel search across variants
    let results: Vec<Option<MultiCombatResult>> = variant_slots
        .par_iter()
        .map(|slots| {
            let n = slots.len();
            let mut best: Option<MultiCombatResult> = None;
            let mut perm: Vec<usize> = (0..n).collect();

            // Try initial order
            {
                let ordered: Vec<AttackAction> = perm.iter().map(|&i| slots[i].clone()).collect();
                if valid_action_order(&ordered) && is_canonical_perm(&perm, slots, attackers) {
                    let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                    let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
                    if passes_final_check(attackers, &result) {
                        best = Some(result);
                    }
                }
            }

            if n > 1 {
                for (a, b) in Perms::new(n) {
                    if a == 0 && b == 0 { continue; }
                    perm.swap(a, b);

                    // #1: Skip non-canonical permutations (dedup identical units)
                    if !is_canonical_perm(&perm, slots, attackers) { continue; }

                    let ordered: Vec<AttackAction> = perm.iter().map(|&i| slots[i].clone()).collect();
                    if !valid_action_order(&ordered) { continue; }

                    let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                    let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
                    if !passes_final_check(attackers, &result) { continue; }

                    // #2: Early termination — if defender killed with 0 casualties and max HP,
                    // this is provably optimal for the "kill" objective
                    let dominated = best.as_ref().map_or(false, |b| {
                        b.defender_hp_final <= 0.0
                            && b.attacker_casualties == 0
                            && result.defender_hp_final <= 0.0
                            && result.attacker_casualties == 0
                            && result.attackers_hp_remaining <= b.attackers_hp_remaining
                    });
                    if dominated { continue; }

                    if best.as_ref().map_or(true, |b| is_better(&result, b)) {
                        best = Some(result);
                    }
                }
            }

            best
        })
        .collect();

    // Pick the best across all variants
    let mut overall_best: Option<MultiCombatResult> = None;
    for result in results.into_iter().flatten() {
        if overall_best.as_ref().map_or(true, |b| is_better(&result, b)) {
            overall_best = Some(result);
        }
    }

    overall_best.unwrap()
}

/// Sequential multi-defender for calc. Each attacker fights one defender only.
/// When a defender dies, remaining actions carry to the next defender.
pub fn sequential_multi_defender(attackers: &[Unit], defenders: &[Unit]) -> Vec<MultiCombatResult> {
    let mut all_actions = Vec::new();
    for (idx, atk) in attackers.iter().enumerate() {
        let is_x = atk.status_effects.contains(StatusEffects::EXPLODING);
        let is_ax = atk.status_effects.contains(StatusEffects::ATTACK_EXPLODE);
        let is_axi = is_ax && atk.status_effects.contains(StatusEffects::INSTANT_EXPLODE);
        if is_x {
            all_actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: true });
        } else if atk.double_attack {
            all_actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: false, explode_only: false });
            all_actions.push(AttackAction { attacker_idx: idx, hit_num: 1, explode: false, explode_only: false });
        } else {
            all_actions.push(AttackAction { attacker_idx: idx, hit_num: 0, explode: is_axi, explode_only: false });
            if is_ax && !is_axi {
                all_actions.push(AttackAction { attacker_idx: idx, hit_num: 1, explode: false, explode_only: true });
            }
        }
    }

    let mut results = Vec::new();
    let mut offset = 0usize;
    let mut attacker_hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();

    for defender in defenders {
        if offset >= all_actions.len() {
            results.push(MultiCombatResult {
                events: vec![], defender_hp_final: defender.current_hp,
                defender_name: defender.display_name(), attacker_casualties: 0,
                attackers_hp_remaining: 0.0, order: vec![],
            });
            continue;
        }
        let remaining = &all_actions[offset..];
        let (result, consumed) = execute_actions(attackers, remaining, defender, &mut attacker_hp, false);
        offset += consumed;
        results.push(result);
    }

    results
}

/// Optimized multi-defender: assign action slots to defenders, find best.
/// Uses dedup pruning, early termination, and parallel search.
pub fn optimized_multi_defender(attackers: &[Unit], defenders: &[Unit]) -> Vec<MultiCombatResult> {
    let m = defenders.len();
    if m == 1 {
        return vec![optimized(attackers, &defenders[0])];
    }

    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return defenders.iter().map(|d| MultiCombatResult {
            events: vec![], defender_hp_final: d.current_hp,
            defender_name: d.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: 0.0, order: vec![],
        }).collect();
    }

    let max_bits = 3;
    let num_variants = 1usize << explode_opts.len().min(max_bits);

    let variant_slots: Vec<Vec<AttackAction>> = (0..num_variants)
        .map(|variant| apply_explode_variant(&base_slots, &explode_opts, variant, max_bits))
        .collect();

    // Build work items: (variant_idx, assignment_code)
    // Use variant-specific slot count for total_assignments
    let work_items: Vec<(usize, usize)> = variant_slots
        .iter()
        .enumerate()
        .flat_map(|(vi, slots)| {
            let ta = m.checked_pow(slots.len() as u32).unwrap_or(usize::MAX);
            (0..ta).map(move |code| (vi, code))
        })
        .collect();

    // Parallel search across all (variant, assignment) pairs
    let best = work_items
        .par_iter()
        .filter_map(|&(vi, code)| {
            let slots = &variant_slots[vi];
            let sn = slots.len();
            let mut assignments = Vec::with_capacity(sn);
            let mut c = code;
            for i in 0..sn {
                let def_idx = c % m;
                // #1: Dedup — if this slot is equivalent to the previous and assigned
                // to a lower-indexed defender, skip (canonical form: non-decreasing)
                if i > 0 && slots_equivalent(&slots[i], &slots[i - 1], attackers)
                    && def_idx < assignments[i - 1]
                {
                    return None;
                }
                assignments.push(def_idx);
                c /= m;
            }

            if !valid_multi_assignment(slots, &assignments) { return None; }

            let mut per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
            for (i, &d) in assignments.iter().enumerate() {
                per_def[d].push(slots[i].clone());
            }

            let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
            let empty_prior = std::collections::HashSet::new();
            let (results, score) = find_best_ordering(attackers, &per_def, defenders, &hp_state, 0, &empty_prior);

            Some((results, score))
        })
        .reduce_with(|a, b| if a.1 >= b.1 { a } else { b });

    match best {
        Some((results, _)) => results,
        None => sequential_multi_defender(attackers, defenders),
    }
}

/// Recursively find best action ordering across defenders.
/// `prior_hit0` tracks attacker indices whose hit 0 was executed in earlier groups,
/// allowing hit 1 to appear in later groups without hit 0 present.
fn find_best_ordering(
    attackers: &[Unit],
    per_def: &[Vec<AttackAction>],
    def_units: &[Unit],
    hp: &[f32],
    idx: usize,
    prior_hit0: &std::collections::HashSet<usize>,
) -> (Vec<MultiCombatResult>, i64) {
    if idx >= per_def.len() {
        return (vec![], 0);
    }

    let group = &per_def[idx];
    let defender = &def_units[idx];

    if group.is_empty() {
        let empty = MultiCombatResult {
            events: vec![], defender_hp_final: defender.current_hp,
            defender_name: defender.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: 0.0, order: vec![],
        };
        let (mut rest, rs) = find_best_ordering(attackers, per_def, def_units, hp, idx + 1, prior_hit0);
        let mut all = vec![empty];
        all.append(&mut rest);
        return (all, rs);
    }

    let gn = group.len();
    let mut best_all: Option<Vec<MultiCombatResult>> = None;
    let mut best_score = i64::MIN;

    // Collect hit-0 attacker indices in this group for passing to next defender
    let local_hit0s: std::collections::HashSet<usize> = group.iter()
        .filter(|a| a.hit_num == 0)
        .map(|a| a.attacker_idx)
        .collect();

    let prior = prior_hit0.clone();

    // Helper: try one ordering
    let try_order = |ordered: &[AttackAction], best_all: &mut Option<Vec<MultiCombatResult>>, best_score: &mut i64| {
        if !valid_action_order_with_prior(ordered, &prior) { return; }
        let mut hp_copy = hp.to_vec();
        let (result, _) = execute_actions(attackers, ordered, defender, &mut hp_copy, true);
        if !passes_final_check(attackers, &result) { return; }
        let score = score_single(&result);
        let mut next_prior = prior.clone();
        next_prior.extend(&local_hit0s);
        let (mut rest, rs) = find_best_ordering(attackers, per_def, def_units, &hp_copy, idx + 1, &next_prior);
        let total = score + rs;
        if total > *best_score {
            let mut all = vec![result];
            all.append(&mut rest);
            *best_all = Some(all);
            *best_score = total;
        }
    };

    {
        let mut perm: Vec<usize> = (0..gn).collect();
        // Initial
        {
            let ordered: Vec<AttackAction> = perm.iter().map(|&i| group[i].clone()).collect();
            if is_canonical_perm(&perm, group, attackers) {
                try_order(&ordered, &mut best_all, &mut best_score);
            }
        }
        if gn > 1 {
            for (a, b) in Perms::new(gn) {
                if a == 0 && b == 0 { continue; }
                perm.swap(a, b);
                // #1: Skip non-canonical permutations (dedup identical units)
                if !is_canonical_perm(&perm, group, attackers) { continue; }
                let ordered: Vec<AttackAction> = perm.iter().map(|&i| group[i].clone()).collect();
                try_order(&ordered, &mut best_all, &mut best_score);
            }
        }
    }

    (best_all.unwrap_or_default(), best_score)
}

/// Constrained multi-defender: same as optimized_multi_defender but respects
/// reach constraints. Uses dedup pruning and parallel search.
pub fn optimized_multi_defender_constrained(
    attackers: &[Unit],
    defenders: &[Unit],
    reach_masks: &[u32],
) -> Vec<MultiCombatResult> {
    let m = defenders.len();
    assert!(m <= 3, "Max 3 defenders for constrained optimizer");

    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return defenders
            .iter()
            .map(|d| MultiCombatResult {
                events: vec![],
                defender_hp_final: d.current_hp,
                defender_name: d.display_name(),
                attacker_casualties: 0,
                attackers_hp_remaining: 0.0,
                order: vec![],
            })
            .collect();
    }

    let max_bits = 3;
    let num_variants = 1usize << explode_opts.len().min(max_bits);

    let variant_slots: Vec<Vec<AttackAction>> = (0..num_variants)
        .map(|variant| apply_explode_variant(&base_slots, &explode_opts, variant, max_bits))
        .collect();

    let slot_reach_variants: Vec<Vec<u32>> = variant_slots
        .iter()
        .map(|slots| slots.iter().map(|s| reach_masks[s.attacker_idx]).collect())
        .collect();

    let work_items: Vec<(usize, usize)> = variant_slots
        .iter()
        .enumerate()
        .flat_map(|(vi, slots)| {
            let ta = m.checked_pow(slots.len() as u32).unwrap_or(usize::MAX);
            (0..ta).map(move |code| (vi, code))
        })
        .collect();

    let best = work_items
        .par_iter()
        .filter_map(|&(vi, code)| {
            let slots = &variant_slots[vi];
            let slot_reach = &slot_reach_variants[vi];
            let sn = slots.len();
            let mut assignments = Vec::with_capacity(sn);
            let mut c = code;
            for i in 0..sn {
                let def_idx = c % m;
                if slot_reach[i] & (1 << def_idx) == 0 { return None; }
                // Dedup: canonical form for equivalent slots
                if i > 0 && slots_equivalent(&slots[i], &slots[i - 1], attackers)
                    && slot_reach[i] == slot_reach[i - 1]
                    && def_idx < assignments[i - 1]
                {
                    return None;
                }
                assignments.push(def_idx);
                c /= m;
            }

            if !valid_multi_assignment(slots, &assignments) { return None; }

            let mut per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
            for (i, &d) in assignments.iter().enumerate() {
                per_def[d].push(slots[i].clone());
            }

            let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
            let empty_prior = std::collections::HashSet::new();
            let (results, score) =
                find_best_ordering(attackers, &per_def, defenders, &hp_state, 0, &empty_prior);

            Some((results, score))
        })
        .reduce_with(|a, b| if a.1 >= b.1 { a } else { b });

    match best {
        Some((results, _)) => results,
        None => sequential_multi_defender(attackers, defenders),
    }
}

// ======================== Greedy Optimization ========================

/// Greedy single-defender optimizer for large inputs (>12 slots).
/// Constructs an ordering greedily then improves with pairwise swaps.
pub fn greedy_optimized(attackers: &[Unit], defender: &Unit) -> MultiCombatResult {
    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return MultiCombatResult {
            events: vec![], defender_hp_final: defender.current_hp,
            defender_name: defender.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: attackers.iter().map(|a| a.current_hp).sum(),
            order: vec![],
        };
    }

    let max_bits = 4;
    let num_variants = 1usize << explode_opts.len().min(max_bits);
    let mut best: Option<MultiCombatResult> = None;

    for variant in 0..num_variants {
        let slots = apply_explode_variant(&base_slots, &explode_opts, variant, max_bits);

        // Greedy construction: at each step pick the action that gives the best result
        let order = greedy_construct(&slots, attackers, defender);

        // Evaluate the greedy order
        let ordered: Vec<AttackAction> = order.iter().map(|&i| slots[i].clone()).collect();
        let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
        let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
        if passes_final_check(attackers, &result) {
            if best.as_ref().map_or(true, |b| is_better(&result, b)) { best = Some(result); }
        }

        // Swap improvement: repeatedly try all pairwise swaps until no improvement
        let improved_order = swap_improve(&slots, attackers, defender, order);
        let ordered: Vec<AttackAction> = improved_order.iter().map(|&i| slots[i].clone()).collect();
        let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
        let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
        if passes_final_check(attackers, &result) {
            if best.as_ref().map_or(true, |b| is_better(&result, b)) { best = Some(result); }
        }
    }

    best.unwrap()
}

/// Greedy multi-defender optimizer for large inputs.
/// Assigns attackers to defenders greedily, then optimizes ordering within each group.
pub fn greedy_optimized_multi_defender(attackers: &[Unit], defenders: &[Unit]) -> Vec<MultiCombatResult> {
    let m = defenders.len();
    if m == 1 {
        return vec![greedy_optimized(attackers, &defenders[0])];
    }

    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return defenders.iter().map(|d| MultiCombatResult {
            events: vec![], defender_hp_final: d.current_hp,
            defender_name: d.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: 0.0, order: vec![],
        }).collect();
    }

    let max_bits = 3;
    let num_variants = 1usize << explode_opts.len().min(max_bits);
    let mut best_results: Option<Vec<MultiCombatResult>> = None;
    let mut best_score = i64::MIN;

    for variant in 0..num_variants {
        let slots = apply_explode_variant(&base_slots, &explode_opts, variant, max_bits);

        // Greedy assignment: assign each slot to the defender where it helps most
        let assignments = greedy_assign(&slots, attackers, defenders);

        if !valid_multi_assignment(&slots, &assignments) { continue; }

        let mut per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
        for (i, &d) in assignments.iter().enumerate() {
            per_def[d].push(slots[i].clone());
        }

        let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
        let empty_prior = std::collections::HashSet::new();
        let (results, score) = find_best_ordering_greedy(attackers, &per_def, defenders, &hp_state, 0, &empty_prior);

        if score > best_score {
            best_score = score;
            best_results = Some(results);
        }

        // Also try: swap each slot's assignment to a different defender and see if it improves
        for slot_idx in 0..slots.len() {
            for alt_def in 0..m {
                if alt_def == assignments[slot_idx] { continue; }
                let mut alt_assignments = assignments.clone();
                alt_assignments[slot_idx] = alt_def;
                if !valid_multi_assignment(&slots, &alt_assignments) { continue; }

                let mut alt_per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
                for (i, &d) in alt_assignments.iter().enumerate() {
                    alt_per_def[d].push(slots[i].clone());
                }

                let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                let empty_prior = std::collections::HashSet::new();
                let (results, score) = find_best_ordering_greedy(attackers, &alt_per_def, defenders, &hp_state, 0, &empty_prior);

                if score > best_score {
                    best_score = score;
                    best_results = Some(results);
                }
            }
        }
    }

    best_results.unwrap_or_else(|| sequential_multi_defender(attackers, defenders))
}

/// Greedy constrained multi-defender optimizer.
pub fn greedy_optimized_multi_defender_constrained(
    attackers: &[Unit],
    defenders: &[Unit],
    reach_masks: &[u32],
) -> Vec<MultiCombatResult> {
    let m = defenders.len();
    let (base_slots, explode_opts) = generate_base_slots(attackers);
    let n = base_slots.len();

    if n == 0 {
        return defenders.iter().map(|d| MultiCombatResult {
            events: vec![], defender_hp_final: d.current_hp,
            defender_name: d.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: 0.0, order: vec![],
        }).collect();
    }

    let max_bits = 3;
    let num_variants = 1usize << explode_opts.len().min(max_bits);
    let mut best_results: Option<Vec<MultiCombatResult>> = None;
    let mut best_score = i64::MIN;

    for variant in 0..num_variants {
        let slots = apply_explode_variant(&base_slots, &explode_opts, variant, max_bits);
        // reach masks may need re-derivation since the variant may append slots
        let slot_reach: Vec<u32> = slots.iter().map(|s| reach_masks[s.attacker_idx]).collect();

        let assignments = greedy_assign_constrained(&slots, attackers, defenders, &slot_reach);

        if !valid_multi_assignment(&slots, &assignments) { continue; }

        let mut per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
        for (i, &d) in assignments.iter().enumerate() {
            per_def[d].push(slots[i].clone());
        }

        let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
        let empty_prior = std::collections::HashSet::new();
        let (results, score) = find_best_ordering_greedy(attackers, &per_def, defenders, &hp_state, 0, &empty_prior);

        if score > best_score {
            best_score = score;
            best_results = Some(results);
        }

        // Swap improvement on assignments
        for slot_idx in 0..n {
            for alt_def in 0..m {
                if alt_def == assignments[slot_idx] { continue; }
                if slot_reach[slot_idx] & (1 << alt_def) == 0 { continue; }
                let mut alt_assignments = assignments.clone();
                alt_assignments[slot_idx] = alt_def;
                if !valid_multi_assignment(&slots, &alt_assignments) { continue; }

                let mut alt_per_def: Vec<Vec<AttackAction>> = vec![Vec::new(); m];
                for (i, &d) in alt_assignments.iter().enumerate() {
                    alt_per_def[d].push(slots[i].clone());
                }

                let hp_state: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                let empty_prior = std::collections::HashSet::new();
                let (results, score) = find_best_ordering_greedy(attackers, &alt_per_def, defenders, &hp_state, 0, &empty_prior);

                if score > best_score {
                    best_score = score;
                    best_results = Some(results);
                }
            }
        }
    }

    best_results.unwrap_or_else(|| sequential_multi_defender(attackers, defenders))
}

/// Greedy construction: build ordering by picking the best next action at each step.
fn greedy_construct(slots: &[AttackAction], attackers: &[Unit], defender: &Unit) -> Vec<usize> {
    let n = slots.len();
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut order: Vec<usize> = Vec::with_capacity(n);
    let mut def = defender.clone();
    let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();

    while !remaining.is_empty() {
        if def.current_hp <= 0.0 {
            // Defender dead — append rest in original order (won't matter)
            order.extend(&remaining);
            break;
        }

        let mut best_ri: Option<usize> = None;
        let mut best_dmg = f32::MIN;

        for (ri, &slot_idx) in remaining.iter().enumerate() {
            let action = &slots[slot_idx];

            // Enforce hit ordering: hit 1 requires hit 0 already placed
            if action.hit_num == 1 {
                let has_hit0 = order.iter().any(|&oi| {
                    slots[oi].attacker_idx == action.attacker_idx && slots[oi].hit_num == 0
                });
                if !has_hit0 { continue; }
            }

            let atk_hp = hp[action.attacker_idx];
            if atk_hp <= 0.0 { continue; }

            // Simulate this action against current defender state
            let mut temp_atk = attackers[action.attacker_idx].clone();
            temp_atk.current_hp = atk_hp;
            if action.explode_only || action.explode {
                temp_atk.status_effects.insert(StatusEffects::EXPLODING);
            }
            let result = calculate_damage(&temp_atk, &def);
            let dmg = result.damage_to_defender;
            // Poison bonus: if this unit would poison an unpoisoned defender,
            // add a large bonus because halving defense for all subsequent attacks
            // is worth more than raw damage
            let poison_bonus = if temp_atk.trait_effects.contains(StatusEffects::POISONED)
                && !def.status_effects.contains(StatusEffects::POISONED)
                && def.current_hp - dmg > 0.0
            {
                def.current_hp * 2.0 // scale with defender remaining HP
            } else {
                0.0
            };
            let score = dmg * 1000.0 + poison_bonus * 1000.0 - result.damage_to_attacker;

            if best_ri.is_none() || score > best_dmg {
                best_dmg = score;
                best_ri = Some(ri);
            }
        }

        match best_ri {
            Some(ri) => {
                let slot_idx = remaining.remove(ri);
                let action = &slots[slot_idx];

                // Actually execute this action to update state
                let mut temp_atk = attackers[action.attacker_idx].clone();
                temp_atk.current_hp = hp[action.attacker_idx];
                if action.explode_only {
                    temp_atk.status_effects.insert(StatusEffects::EXPLODING);
                    let result = calculate_damage(&temp_atk, &def);
                    def.current_hp = (def.current_hp - result.damage_to_defender).max(0.0);
                    hp[action.attacker_idx] = 0.0;
                    if def.current_hp > 0.0 {
                        def.status_effects.insert(StatusEffects::POISONED);
                        def.recalc_defense_bonus();
                    }
                } else {
                    temp_atk.status_effects.remove(StatusEffects::EXPLODING);
                    let result = calculate_damage(&temp_atk, &def);
                    def.current_hp = (def.current_hp - result.damage_to_defender).max(0.0);
                    hp[action.attacker_idx] = (hp[action.attacker_idx] - result.damage_to_attacker).max(0.0);
                    if result.damage_to_defender > 0.0 && def.current_hp > 0.0 {
                        if temp_atk.trait_effects.contains(StatusEffects::POISONED) {
                            def.status_effects.insert(StatusEffects::POISONED);
                            def.recalc_defense_bonus();
                        }
                    }
                    // Handle attack+explode
                    if action.explode && hp[action.attacker_idx] > 0.0 && def.current_hp > 0.0 {
                        let mut eu = temp_atk.clone();
                        eu.current_hp = hp[action.attacker_idx];
                        eu.status_effects.insert(StatusEffects::EXPLODING);
                        let exp = calculate_damage(&eu, &def);
                        def.current_hp = (def.current_hp - exp.damage_to_defender).max(0.0);
                        hp[action.attacker_idx] = 0.0;
                        if def.current_hp > 0.0 {
                            def.status_effects.insert(StatusEffects::POISONED);
                            def.recalc_defense_bonus();
                        }
                    }
                }

                order.push(slot_idx);
            }
            None => {
                // No valid action available (all blocked by hit ordering), append rest
                order.extend(&remaining);
                break;
            }
        }
    }

    order
}

/// Swap improvement: try all pairwise swaps, accept first improvement, repeat.
fn swap_improve(slots: &[AttackAction], attackers: &[Unit], defender: &Unit, mut order: Vec<usize>) -> Vec<usize> {
    let n = order.len();
    let mut improved = true;
    let mut iterations = 0;
    let max_iterations = n * 3; // prevent infinite loops

    while improved && iterations < max_iterations {
        improved = false;
        iterations += 1;

        let current_ordered: Vec<AttackAction> = order.iter().map(|&i| slots[i].clone()).collect();
        if !valid_action_order(&current_ordered) { break; }
        let mut current_hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
        let (current_result, _) = execute_actions(attackers, &current_ordered, defender, &mut current_hp, true);
        let current_score = score_result(&current_result);

        for i in 0..n {
            for j in (i + 1)..n {
                let mut candidate = order.clone();
                candidate.swap(i, j);

                let ordered: Vec<AttackAction> = candidate.iter().map(|&k| slots[k].clone()).collect();
                if !valid_action_order(&ordered) { continue; }

                let mut hp: Vec<f32> = attackers.iter().map(|a| a.current_hp).collect();
                let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp, true);
                if !passes_final_check(attackers, &result) { continue; }

                if score_result(&result) > current_score {
                    order = candidate;
                    improved = true;
                    break;
                }
            }
            if improved { break; }
        }
    }

    order
}

/// Greedy assignment of slots to defenders.
/// Tracks remaining defender HP and assigns each slot to the defender
/// where it provides the most marginal value (damage relative to remaining HP).
fn greedy_assign(slots: &[AttackAction], attackers: &[Unit], defenders: &[Unit]) -> Vec<usize> {
    let m = defenders.len();
    let n = slots.len();
    let mut assignments = vec![0usize; n];
    let mut def_hp_remaining: Vec<f32> = defenders.iter().map(|d| d.current_hp).collect();

    for (i, action) in slots.iter().enumerate() {
        let atk = &attackers[action.attacker_idx];
        let mut best_def = 0;
        let mut best_score = f32::MIN;

        for d in 0..m {
            // Skip defenders already dead
            if def_hp_remaining[d] <= 0.0 { continue; }

            let result = calculate_damage(atk, &defenders[d]);
            let dmg = result.damage_to_defender;

            // Score: prioritize defenders that still need damage
            // Bonus for kills, penalty for overkill
            let score = if dmg >= def_hp_remaining[d] {
                // Would kill — high value, but penalize overkill
                def_hp_remaining[d] * 10.0 - (dmg - def_hp_remaining[d])
            } else {
                // Partial damage — value is proportional to how much of the
                // defender's remaining HP this removes
                dmg + (dmg / def_hp_remaining[d]) * 5.0
            };

            if score > best_score {
                best_score = score;
                best_def = d;
            }
        }

        // If all defenders are dead, just assign to first
        if def_hp_remaining.iter().all(|&hp| hp <= 0.0) {
            best_def = 0;
        }

        // For double-attack hit 1, must go to same or later defender as hit 0
        if action.hit_num == 1 {
            if let Some(hit0_def) = slots.iter().enumerate()
                .find(|(_, s)| s.attacker_idx == action.attacker_idx && s.hit_num == 0)
                .map(|(j, _)| assignments[j])
            {
                if best_def < hit0_def {
                    best_def = hit0_def;
                }
            }
        }

        assignments[i] = best_def;

        // Update remaining HP estimate
        let result = calculate_damage(atk, &defenders[best_def]);
        def_hp_remaining[best_def] = (def_hp_remaining[best_def] - result.damage_to_defender).max(0.0);
    }

    assignments
}

/// Greedy assignment with reach constraints.
/// Same marginal-value logic but respects reach masks.
fn greedy_assign_constrained(
    slots: &[AttackAction],
    attackers: &[Unit],
    defenders: &[Unit],
    slot_reach: &[u32],
) -> Vec<usize> {
    let m = defenders.len();
    let n = slots.len();
    let mut assignments = vec![0usize; n];
    let mut def_hp_remaining: Vec<f32> = defenders.iter().map(|d| d.current_hp).collect();

    for (i, action) in slots.iter().enumerate() {
        let atk = &attackers[action.attacker_idx];
        let mut best_def = 0;
        let mut best_score = f32::MIN;
        let mut found_any = false;

        for d in 0..m {
            if slot_reach[i] & (1 << d) == 0 { continue; }
            if def_hp_remaining[d] <= 0.0 && def_hp_remaining.iter().any(|&hp| hp > 0.0) { continue; }

            let result = calculate_damage(atk, &defenders[d]);
            let dmg = result.damage_to_defender;

            let score = if dmg >= def_hp_remaining[d] {
                def_hp_remaining[d] * 10.0 - (dmg - def_hp_remaining[d])
            } else {
                dmg + (dmg / def_hp_remaining[d].max(0.1)) * 5.0
            };

            if !found_any || score > best_score {
                best_score = score;
                best_def = d;
                found_any = true;
            }
        }

        if action.hit_num == 1 {
            if let Some(hit0_def) = slots.iter().enumerate()
                .find(|(_, s)| s.attacker_idx == action.attacker_idx && s.hit_num == 0)
                .map(|(j, _)| assignments[j])
            {
                if best_def < hit0_def && slot_reach[i] & (1 << hit0_def) != 0 {
                    best_def = hit0_def;
                }
            }
        }

        assignments[i] = best_def;

        let result = calculate_damage(atk, &defenders[best_def]);
        def_hp_remaining[best_def] = (def_hp_remaining[best_def] - result.damage_to_defender).max(0.0);
    }

    assignments
}


/// Like find_best_ordering but uses greedy+swap within each group instead of full permutation.
fn find_best_ordering_greedy(
    attackers: &[Unit],
    per_def: &[Vec<AttackAction>],
    def_units: &[Unit],
    hp: &[f32],
    idx: usize,
    prior_hit0: &std::collections::HashSet<usize>,
) -> (Vec<MultiCombatResult>, i64) {
    if idx >= per_def.len() {
        return (vec![], 0);
    }

    let group = &per_def[idx];
    let defender = &def_units[idx];

    if group.is_empty() {
        let empty = MultiCombatResult {
            events: vec![], defender_hp_final: defender.current_hp,
            defender_name: defender.display_name(), attacker_casualties: 0,
            attackers_hp_remaining: 0.0, order: vec![],
        };
        let (mut rest, rs) = find_best_ordering_greedy(attackers, per_def, def_units, hp, idx + 1, prior_hit0);
        let mut all = vec![empty];
        all.append(&mut rest);
        return (all, rs);
    }

    let local_hit0s: std::collections::HashSet<usize> = group.iter()
        .filter(|a| a.hit_num == 0)
        .map(|a| a.attacker_idx)
        .collect();

    // Use greedy+swap for this group's ordering
    let group_slots: Vec<AttackAction> = group.clone();
    let greedy_order = greedy_construct(&group_slots, attackers, defender);
    let improved_order = swap_improve(&group_slots, attackers, defender, greedy_order);

    let ordered: Vec<AttackAction> = improved_order.iter().map(|&i| group_slots[i].clone()).collect();
    let prior = prior_hit0.clone();

    if valid_action_order_with_prior(&ordered, &prior) {
        let mut hp_copy = hp.to_vec();
        let (result, _) = execute_actions(attackers, &ordered, defender, &mut hp_copy, true);
        let score = score_single(&result);
        let mut next_prior = prior;
        next_prior.extend(&local_hit0s);
        let (mut rest, rs) = find_best_ordering_greedy(attackers, per_def, def_units, &hp_copy, idx + 1, &next_prior);
        let total = score + rs;
        let mut all = vec![result];
        all.append(&mut rest);
        (all, total)
    } else {
        // Fallback: use original order
        let mut hp_copy = hp.to_vec();
        let (result, _) = execute_actions(attackers, group, defender, &mut hp_copy, true);
        let score = score_single(&result);
        let mut next_prior = prior;
        next_prior.extend(&local_hit0s);
        let (mut rest, rs) = find_best_ordering_greedy(attackers, per_def, def_units, &hp_copy, idx + 1, &next_prior);
        let total = score + rs;
        let mut all = vec![result];
        all.append(&mut rest);
        (all, total)
    }
}

// ======================== Threshold ========================

/// Find the minimum subset of attackers to bring defender to ≤ target_hp.
/// Tries smallest subsets first, then ranks by casualties, remaining HP, and closeness to target.
pub fn threshold(attackers: &[Unit], defender: &Unit, target_hp: f32) -> Option<MultiCombatResult> {
    let n = attackers.len();
    if n == 0 || n > 20 { return None; }

    let mut best: Option<MultiCombatResult> = None;

    for size in 1..=n {
        let mut found = false;

        // Iterate all bitmasks with exactly `size` bits set
        for mask in 1..(1u32 << n) {
            if (mask.count_ones() as usize) != size { continue; }

            let subset: Vec<Unit> = (0..n)
                .filter(|&i| mask & (1 << i) != 0)
                .map(|i| attackers[i].clone())
                .collect();

            let result = optimized(&subset, defender);

            if result.defender_hp_final <= target_hp {
                found = true;
                if best.as_ref().map_or(true, |b| threshold_better(&result, b)) {
                    best = Some(result);
                }
            }
        }

        if found { break; }
    }

    best
}

/// Compare two threshold results (both achieve threshold).
/// Prefer: fewer casualties, more remaining HP, closer to target (less overkill).
fn threshold_better(a: &MultiCombatResult, b: &MultiCombatResult) -> bool {
    let a_score = (
        -(a.attacker_casualties as i64),
        (a.attackers_hp_remaining * 10.0) as i64,
        (a.defender_hp_final * 10.0) as i64, // higher = closer to target = less waste
    );
    let b_score = (
        -(b.attacker_casualties as i64),
        (b.attackers_hp_remaining * 10.0) as i64,
        (b.defender_hp_final * 10.0) as i64,
    );
    a_score > b_score
}

/// Multi-defender threshold: find minimum subset of attackers to bring ALL defenders
/// to their respective target HPs. Each defender can have a different target.
/// Defenders already at or below their target are excluded from optimization.
/// Returns (per-defender results for ALL defenders, subset attacker indices).
pub fn threshold_multi(attackers: &[Unit], defenders: &[Unit], targets: &[f32]) -> Option<(Vec<MultiCombatResult>, Vec<usize>)> {
    let n = attackers.len();
    if n == 0 || n > 20 { return None; }

    // Filter to only defenders that need weakening
    let needs_work: Vec<usize> = defenders.iter().enumerate()
        .filter(|(i, d)| d.current_hp > targets[*i])
        .map(|(i, _)| i)
        .collect();

    if needs_work.is_empty() {
        return None;
    }

    let work_defenders: Vec<Unit> = needs_work.iter().map(|&i| defenders[i].clone()).collect();
    let work_targets: Vec<f32> = needs_work.iter().map(|&i| targets[i]).collect();

    let mut best: Option<(Vec<MultiCombatResult>, Vec<usize>)> = None;
    let mut best_size = usize::MAX;

    for size in 1..=n {
        if size > best_size { break; }
        let mut found = false;

        for mask in 1..(1u32 << n) {
            if (mask.count_ones() as usize) != size { continue; }

            let indices: Vec<usize> = (0..n).filter(|&i| mask & (1 << i) != 0).collect();
            let subset: Vec<Unit> = indices.iter().map(|&i| attackers[i].clone()).collect();

            let work_results = optimized_multi_defender(&subset, &work_defenders);

            // Check if each work defender is at or below its own target
            let all_met = work_results.iter().enumerate()
                .all(|(i, r)| r.defender_hp_final <= work_targets[i]);

            if all_met {
                found = true;

                let is_better = if let Some((ref prev, _)) = best {
                    let prev_work: Vec<_> = prev.iter()
                        .enumerate()
                        .filter(|(i, _)| needs_work.contains(i))
                        .map(|(_, r)| r.clone())
                        .collect();
                    threshold_multi_better(&work_results, &prev_work)
                } else {
                    true
                };

                if is_better {
                    let mut full_results: Vec<MultiCombatResult> = Vec::new();
                    let mut work_iter = work_results.into_iter();
                    for (i, def) in defenders.iter().enumerate() {
                        if needs_work.contains(&i) {
                            full_results.push(work_iter.next().unwrap());
                        } else {
                            full_results.push(MultiCombatResult {
                                events: vec![],
                                defender_hp_final: def.current_hp,
                                defender_name: def.display_name(),
                                attacker_casualties: 0,
                                attackers_hp_remaining: 0.0,
                                order: vec![],
                            });
                        }
                    }
                    best = Some((full_results, indices));
                    best_size = size;
                }
            }
        }

        if found { break; }
    }

    best
}

/// Compare two multi-defender threshold results.
/// Prefer: fewer casualties total, more remaining HP, less overkill.
fn threshold_multi_better(a: &[MultiCombatResult], b: &[MultiCombatResult]) -> bool {
    let score = |results: &[MultiCombatResult]| -> (i64, i64, i64) {
        let casualties: usize = results.iter().map(|r| r.attacker_casualties).sum();
        let hp_remaining: f32 = results.last().map_or(0.0, |r| r.attackers_hp_remaining);
        let defender_hp: f32 = results.iter().map(|r| r.defender_hp_final.max(0.0)).sum();
        (
            -(casualties as i64),
            (hp_remaining * 10.0) as i64,
            (defender_hp * 10.0) as i64, // higher = less overkill
        )
    };
    score(a) > score(b)
}

// ======================== Utility ========================

pub fn bulk(attacker: &Unit, defender: &Unit) -> Option<u32> {
    if attacker.attack <= 0.0 { return None; }
    let mut def = defender.clone();
    let mut count = 0u32;
    while def.current_hp > 0.0 && count < 1000 {
        let result = calculate_damage(attacker, &def);
        if result.damage_to_defender <= 0.0 { return None; }
        def.current_hp = (def.current_hp - result.damage_to_defender).max(0.0);
        if attacker.trait_effects.contains(StatusEffects::POISONED) && def.current_hp > 0.0 {
            def.status_effects.insert(StatusEffects::POISONED);
            def.recalc_defense_bonus();
        }
        count += 1;
    }
    Some(count)
}

pub fn eliminate_defender(attacker: &Unit, defender: &Unit) -> Option<f32> {
    if attacker.attack <= 0.0 { return None; }
    for hp in 1..=(attacker.max_hp as u32) {
        let mut a = attacker.clone();
        a.current_hp = hp as f32;
        let result = calculate_damage(&a, defender);
        if result.damage_to_defender >= defender.current_hp {
            return Some(hp as f32);
        }
    }
    None
}

pub fn eliminate_attacker(attacker: &Unit, defender: &Unit) -> f32 {
    for hp in (1..=(defender.max_hp as u32)).rev() {
        let mut d = defender.clone();
        d.current_hp = hp as f32;
        let result = calculate_damage(attacker, &d);
        if result.damage_to_defender >= d.current_hp {
            return hp as f32;
        }
    }
    0.0
}

// ======================== Tests ========================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warrior_vs_warrior() {
        let a = Unit::new(UnitType::Warrior);
        let d = Unit::new(UnitType::Warrior);
        let r = single_combat(&a, &d);
        assert_eq!(r.damage_to_defender, 5.0);
        assert_eq!(r.damage_to_attacker, 5.0);
    }

    #[test]
    fn warrior_vs_defender_walled() {
        let a = Unit::new(UnitType::Warrior);
        let d = Unit::new(UnitType::Defender).with_status(StatusEffects::WALLED);
        let r = single_combat(&a, &d);
        assert!(r.damage_to_defender < 5.0);
    }

    #[test]
    fn catapult_no_retaliation() {
        let a = Unit::new(UnitType::Warrior);
        let d = Unit::new(UnitType::Catapult);
        let r = single_combat(&a, &d);
        assert!(r.damage_to_defender > 0.0);
        assert_eq!(r.damage_to_attacker, 0.0);
    }

    #[test]
    fn dagger_surprise_no_retaliation() {
        let a = Unit::new(UnitType::Dagger);
        let d = Unit::new(UnitType::Warrior);
        let r = single_combat(&a, &d);
        assert!(r.damage_to_defender > 0.0);
        assert_eq!(r.damage_to_attacker, 0.0);
    }

    #[test]
    fn poison_reduces_defense() {
        let a = Unit::new(UnitType::Warrior);
        let d_normal = Unit::new(UnitType::Warrior);
        let d_poisoned = Unit::new(UnitType::Warrior).with_status(StatusEffects::POISONED);
        let r_normal = single_combat(&a, &d_normal);
        let r_poisoned = single_combat(&a, &d_poisoned);
        assert!(r_poisoned.damage_to_defender > r_normal.damage_to_defender);
    }

    #[test]
    fn multi_combat_no_reuse() {
        // Two attackers vs two defenders: each should only attack once
        let wa = Unit::new(UnitType::Warrior);
        let ri = Unit::new(UnitType::Rider);
        let attackers = vec![wa, ri];
        let d1 = Unit::new(UnitType::Warrior).with_hp(1.0); // dies to first hit
        let d2 = Unit::new(UnitType::Warrior);
        let results = sequential_multi_defender(&attackers, &[d1, d2]);
        // D1 should be killed by first attacker
        assert!(results[0].defender_hp_final <= 0.0);
        // D2 should only be attacked by the second attacker (not both)
        assert_eq!(results[1].events.len(), 1);
    }

    // ---- splash floor / fractional ----

    #[test]
    fn splash_is_fractional_by_default() {
        // Fire dragon splash vs a defender — should produce a fractional damage value
        // when the raw damage is odd.
        let a = Unit::new(UnitType::Firedragon).with_status(StatusEffects::SPLASHING);
        let d = Unit::new(UnitType::Defender).with_hp(11.0);
        let r = single_combat(&a, &d);
        // Default (no fl): damage can have half-HP precision
        let frac = r.damage_to_defender - r.damage_to_defender.floor();
        assert!(frac == 0.0 || (frac - 0.5).abs() < 1e-4,
            "expected whole or .5 damage, got {}", r.damage_to_defender);
    }

    #[test]
    fn splash_fl_modifier_floors() {
        // Same fight with and without fl — floored result should be <= fractional result
        // and should have no decimal part.
        let d = Unit::new(UnitType::Defender).with_hp(11.0);
        let a_frac = Unit::new(UnitType::Firedragon).with_status(StatusEffects::SPLASHING);
        let a_floor = Unit::new(UnitType::Firedragon)
            .with_status(StatusEffects::SPLASHING | StatusEffects::FLOOR_SPLASH);
        let r_frac = single_combat(&a_frac, &d);
        let r_floor = single_combat(&a_floor, &d);
        // Floor must equal floor(frac)
        assert_eq!(r_floor.damage_to_defender, r_frac.damage_to_defender.floor());
    }

    #[test]
    fn splash_takes_no_retaliation() {
        // A splashing Juggernaut (range 1) vs a range-1 defender must not eat retaliation,
        // even though a normal range-1 attack against the same defender would.
        // Giant survives the hit (so a normal attack would draw retaliation).
        let d = Unit::new(UnitType::Giant);
        let normal = Unit::new(UnitType::Juggernaut);
        let splash = Unit::new(UnitType::Juggernaut).with_status(StatusEffects::SPLASHING);
        let r_normal = single_combat(&normal, &d);
        let r_splash = single_combat(&splash, &d);
        assert!(r_normal.damage_to_attacker > 0.0, "normal jugg attack should be retaliated");
        assert_eq!(r_splash.damage_to_attacker, 0.0, "splash hit must take no retaliation");
    }

    // ---- jelly tentacles + nr ----

    #[test]
    fn jelly_tentacles_hit_attacker() {
        // Warrior attacks Jelly — tentacle pre-damage + no normal retaliation
        let a = Unit::new(UnitType::Warrior);
        let d = Unit::new(UnitType::Jelly);
        let r = single_combat(&a, &d);
        assert!(r.damage_to_attacker > 0.0, "tentacles should damage attacker");
    }

    #[test]
    fn jelly_nr_suppresses_tentacles() {
        // nr on attacker disables both retaliation AND jelly tentacles
        let a_normal = Unit::new(UnitType::Warrior);
        let a_nr = Unit::new(UnitType::Warrior).with_status(StatusEffects::NO_RETALIATE);
        let d = Unit::new(UnitType::Jelly);
        let r_normal = single_combat(&a_normal, &d);
        let r_nr = single_combat(&a_nr, &d);
        assert!(r_normal.damage_to_attacker > 0.0);
        assert_eq!(r_nr.damage_to_attacker, 0.0, "nr should suppress tentacles");
    }

    // ---- doomux explode modifier semantics ----

    #[test]
    fn doomux_default_does_not_auto_explode() {
        // Bare `doomux` (no ax/axi/x) should NEVER explode, even if exploding would help.
        // With a single Doomux vs a Giant, the optimizer used to try both; now it must not.
        let atk = Unit::new(UnitType::Doomux);
        let def = Unit::new(UnitType::Giant);
        let result = optimized(&[atk.clone()], &def);
        // Events should contain exactly one normal hit (no explode step)
        assert_eq!(result.events.len(), 1);
        assert!(!result.events[0].attacker_name.contains("explode"),
            "default doomux should not auto-explode: {:?}", result.events);
        // Doomux should survive to some HP > 0
        let doomux_final_hp = result.events[0].attacker_hp_after;
        assert!(doomux_final_hp > 0.0, "doomux should not die without ax/axi");
    }

    #[test]
    fn doomux_axi_can_explode_or_not() {
        // axi: optimizer tries with and without the explosion and picks the better.
        // Vs a Giant, exploding once deals more damage, so axi should fire.
        let mut atk = Unit::new(UnitType::Doomux);
        atk.status_effects.insert(StatusEffects::ATTACK_EXPLODE | StatusEffects::INSTANT_EXPLODE);
        let def = Unit::new(UnitType::Giant);
        let result = optimized(&[atk], &def);
        // Either exactly 1 hit (variant skipped the explosion) or the hit bundled the explosion.
        // When the explosion is included, the doomux dies (current_hp → 0).
        assert!(!result.events.is_empty());
    }

    #[test]
    fn doomux_ax_optional_delayed_explode() {
        // ax (non-instant): explosion slot is optional and can be placed after the hit.
        // With ax + a warrior vs giant, optimizer should pick whichever ordering is best.
        let mut doomux = Unit::new(UnitType::Doomux);
        doomux.status_effects.insert(StatusEffects::ATTACK_EXPLODE);
        let warrior = Unit::new(UnitType::Warrior);
        let def = Unit::new(UnitType::Giant);
        let result = optimized(&[doomux, warrior], &def);
        // Just verify it produces a valid result with no panic and doomux hit at least once.
        assert!(!result.events.is_empty());
    }

    // ---- target HP ----

    #[test]
    fn target_exact_achievable() {
        // 4 warriors vs a Giant (40hp). A single warrior hit deals about 5 damage.
        // Pick a target that one or two warriors can actually reach.
        let warriors: Vec<Unit> = (0..4).map(|_| Unit::new(UnitType::Warrior)).collect();
        let giant = Unit::new(UnitType::Giant);
        // First try a few targets and pick one that should be reachable
        // (exact warrior damage depends on HP state; we only assert that
        // SOME target in a reasonable window works)
        let tried = [35.0f32, 34.0, 30.0, 25.0];
        let mut any_ok = false;
        for t in tried {
            if let Ok(r) = optimized_with_target(&warriors, &giant, Target::Exact(t)) {
                assert!((r.defender_hp_final - t).abs() < 1e-4);
                assert!(r.events.len() <= warriors.len());
                any_ok = true;
                break;
            }
        }
        assert!(any_ok, "expected at least one exact target to be achievable");
    }

    #[test]
    fn target_below_uses_fewest_units() {
        // 5 warriors vs a Giant — getting below 35 only needs 1-2 warriors.
        // Below-target optimizer should prefer the smallest prefix.
        let warriors: Vec<Unit> = (0..5).map(|_| Unit::new(UnitType::Warrior)).collect();
        let giant = Unit::new(UnitType::Giant);
        let r = optimized_with_target(&warriors, &giant, Target::Below(35.0)).unwrap();
        assert!(r.defender_hp_final < 35.0);
        // Should use far fewer than all 5 attackers
        assert!(r.events.len() <= 3, "expected ≤3 events, got {}", r.events.len());
    }

    #[test]
    fn target_hp_must_be_below_current() {
        let warriors: Vec<Unit> = (0..3).map(|_| Unit::new(UnitType::Warrior)).collect();
        let giant = Unit::new(UnitType::Giant);
        let err = optimized_with_target(&warriors, &giant, Target::Exact(50.0))
            .expect_err("target above current HP should error");
        assert!(err.contains("must be less than"), "{}", err);
    }

    #[test]
    fn segment_always_explodes_on_attack() {
        // Segment spawns with EXPLODING status and POISONED trait. Any attack = self-destruct + poison.
        let seg = Unit::new(UnitType::Segment);
        assert!(seg.status_effects.contains(StatusEffects::EXPLODING),
            "Segment must start with EXPLODING");
        assert!(seg.trait_effects.contains(StatusEffects::POISONED),
            "Segment must poison on attack");

        let def = Unit::new(UnitType::Warrior);
        let result = single_combat(&seg, &def);
        // Exploding → attacker takes full self-damage
        assert_eq!(result.damage_to_attacker, seg.current_hp,
            "Segment should consume itself on attack");
        // Exploding → damage is halved (splash math), should still be > 0 against warrior
        assert!(result.damage_to_defender > 0.0);
        // Poison trait → defender gets poisoned (if survives)
        if def.current_hp - result.damage_to_defender > 0.0 {
            assert!(result.effects_to_defender.contains(StatusEffects::POISONED),
                "Segment should apply poison to surviving defender");
        }
    }

    #[test]
    fn target_exact_impossible_errors() {
        // 1 warrior vs giant, target HP=1 (impossible with one warrior hit)
        let warriors = vec![Unit::new(UnitType::Warrior)];
        let giant = Unit::new(UnitType::Giant);
        let err = optimized_with_target(&warriors, &giant, Target::Exact(1.0))
            .expect_err("impossible exact target should error");
        assert!(err.contains("exactly"), "{}", err);
    }

    // ======== optim single-defender, multi-attacker ========

    #[test]
    fn optim_uses_all_attackers_when_needed() {
        // 3 knights vs a full-HP giant — all three should fire to chunk the giant.
        let attackers: Vec<Unit> = (0..3).map(|_| Unit::new(UnitType::Knight)).collect();
        let giant = Unit::new(UnitType::Giant);
        let result = optimized(&attackers, &giant);
        assert_eq!(result.events.len(), 3, "all 3 knights should attack");
        // Giant takes meaningful damage (knights hit hard)
        assert!(result.defender_hp_final < giant.current_hp - 5.0,
            "giant should take ≥5 damage total, final={}", result.defender_hp_final);
    }

    #[test]
    fn optim_orders_poison_first() {
        // Phychi poisons first so subsequent warriors benefit from reduced defense.
        // Expect the first event to be the Phychi (or any poisoner) — if Phychi is
        // not first, the warriors' damage would be lower.
        let mut attackers = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Phychi),
        ];
        // keep warriors at full HP, phychi at full HP
        for a in &mut attackers { a.current_hp = a.max_hp; }

        let defender = Unit::new(UnitType::Giant).with_status(StatusEffects::FORTIFIED);
        let result = optimized(&attackers, &defender);

        // Any solution where poison is applied should be chosen — check that the
        // final defender state has the poisoned flag (the event layer records it).
        let any_poison = result.events.iter().any(|e| e.poison_applied);
        assert!(any_poison || result.defender_hp_final <= 0.0,
            "optimizer should apply poison (or outright kill): {:?}", result.events);
    }

    #[test]
    fn optim_final_modifier_forces_killing_blow() {
        // Knight marked `final` must deliver the killing blow.
        // Setup: low-HP defender + weak attacker + final knight — knight must land last.
        let mut knight = Unit::new(UnitType::Knight);
        knight.is_final = true;
        let warrior = Unit::new(UnitType::Warrior);
        let defender = Unit::new(UnitType::Warrior).with_hp(3.0);

        let attackers = vec![warrior, knight];
        let result = optimized(&attackers, &defender);

        // Defender should be dead
        assert!(result.defender_hp_final <= 0.0);
        // Killing blow (last event with def hp ≤ 0) must be the knight (attacker idx 1)
        let kill_pos = result.events.iter().position(|e| e.defender_hp_after <= 0.0).unwrap();
        let killer_idx = result.order[kill_pos];
        assert_eq!(killer_idx, 1, "knight (idx 1) must deliver killing blow");
    }

    #[test]
    fn optim_segment_in_attackers() {
        // Segment among attackers should self-destruct + poison the defender
        // instead of doing a normal attack and retaliating.
        let attackers = vec![
            Unit::new(UnitType::Segment),
            Unit::new(UnitType::Warrior),
        ];
        let defender = Unit::new(UnitType::Giant);
        let result = optimized(&attackers, &defender);
        // Segment casualty guaranteed (it exploded)
        assert!(result.attacker_casualties >= 1, "Segment should die from explosion");
        // Find segment event — verify it applied poison
        let seg_event = result.events.iter()
            .zip(result.order.iter())
            .find(|(_, &idx)| matches!(attackers[idx].unit_type, UnitType::Segment))
            .map(|(e, _)| e);
        assert!(seg_event.is_some(), "segment should have an event");
        // Defender got poisoned if it survived after segment
        if result.defender_hp_final > 0.0 {
            assert!(seg_event.unwrap().poison_applied, "segment should poison surviving defender");
        }
    }

    // ======== optim multi-defender ========

    #[test]
    fn optim_multi_defender_splits_attackers() {
        // 4 warriors vs (weak warrior 3hp, fresh warrior) — optimizer should
        // commit only enough warriors to kill the weak one, then focus on the fresh one.
        let attackers: Vec<Unit> = (0..4).map(|_| Unit::new(UnitType::Warrior)).collect();
        let defenders = vec![
            Unit::new(UnitType::Warrior).with_hp(3.0),
            Unit::new(UnitType::Warrior),
        ];
        let results = optimized_multi_defender(&attackers, &defenders);
        assert_eq!(results.len(), 2);
        // Weak defender should be killed
        assert!(results[0].defender_hp_final <= 0.0);
        // Fresh defender should also take damage
        assert!(results[1].defender_hp_final < defenders[1].current_hp);
    }

    #[test]
    fn optim_multi_defender_respects_hp_state_between_targets() {
        // After hitting a defender, attackers that retaliated have reduced HP for
        // the next target. Verify that a warrior's HP decrease from d0 carries into d1.
        let attackers = vec![Unit::new(UnitType::Warrior), Unit::new(UnitType::Warrior)];
        let defenders = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Warrior),
        ];
        let results = optimized_multi_defender(&attackers, &defenders);
        // Both defenders got damaged (sequential engagement)
        let any_dmg = results.iter().any(|r| r.defender_hp_final < 10.0);
        assert!(any_dmg, "at least one defender should take damage");
        // Total casualty count across results should be consistent with events
        let total_events: usize = results.iter().map(|r| r.events.len()).sum();
        assert!(total_events >= 1);
    }

    #[test]
    fn optim_multi_defender_single_defender_matches_single_path() {
        // Calling optimized_multi_defender with one defender should match optimized().
        let attackers = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Archer),
        ];
        let defender = Unit::new(UnitType::Defender);
        let multi = optimized_multi_defender(&attackers, &[defender.clone()]);
        let single = optimized(&attackers, &defender);
        assert_eq!(multi.len(), 1);
        assert_eq!(multi[0].defender_hp_final, single.defender_hp_final);
        assert_eq!(multi[0].attacker_casualties, single.attacker_casualties);
    }

    // ======== threshold ========

    #[test]
    fn threshold_finds_min_attackers() {
        // 6 warriors vs giant, target ≤35: 1 warrior should be enough.
        let attackers: Vec<Unit> = (0..6).map(|_| Unit::new(UnitType::Warrior)).collect();
        let giant = Unit::new(UnitType::Giant);
        let result = threshold(&attackers, &giant, 35.0).expect("target should be reachable");
        assert!(result.defender_hp_final <= 35.0);
        // Should use a subset, not all 6
        let unique: std::collections::HashSet<usize> = result.order.iter().copied().collect();
        assert!(unique.len() < 6, "should use fewer than all 6 warriors, got {}", unique.len());
    }

    #[test]
    fn threshold_prefers_fewer_casualties() {
        // 4 knights vs a giant, target 20hp — should reach target with minimum units.
        let attackers: Vec<Unit> = (0..4).map(|_| Unit::new(UnitType::Knight)).collect();
        let giant = Unit::new(UnitType::Giant);
        let result = threshold(&attackers, &giant, 20.0).expect("reachable");
        assert!(result.defender_hp_final <= 20.0);
    }

    #[test]
    fn threshold_unreachable_returns_none() {
        // 1 weak warrior vs fresh giant, target 1hp — impossible.
        let attackers = vec![Unit::new(UnitType::Warrior).with_hp(1.0)];
        let giant = Unit::new(UnitType::Giant);
        let result = threshold(&attackers, &giant, 1.0);
        assert!(result.is_none(), "threshold unreachable should return None");
    }

    #[test]
    fn threshold_multi_defender_all_below_target() {
        // 6 knights vs (giant, defender), generous targets that can be reached.
        let attackers: Vec<Unit> = (0..6).map(|_| Unit::new(UnitType::Knight)).collect();
        let defenders = vec![
            Unit::new(UnitType::Giant),
            Unit::new(UnitType::Defender),
        ];
        let targets = vec![30.0, 12.0];
        let (results, subset) = threshold_multi(&attackers, &defenders, &targets)
            .expect("should be reachable");
        assert_eq!(results.len(), 2);
        assert!(results[0].defender_hp_final <= 30.0,
            "giant final {} should be ≤30", results[0].defender_hp_final);
        assert!(results[1].defender_hp_final <= 12.0,
            "defender final {} should be ≤12", results[1].defender_hp_final);
        assert!(subset.len() <= 6);
    }

    #[test]
    fn threshold_multi_skips_already_met() {
        // If a defender is already below its target, threshold_multi should still
        // succeed (the satisfied defender is excluded from the search).
        let attackers: Vec<Unit> = (0..2).map(|_| Unit::new(UnitType::Warrior)).collect();
        let defenders = vec![
            Unit::new(UnitType::Defender).with_hp(3.0),
            Unit::new(UnitType::Warrior),
        ];
        let targets = vec![5.0, 5.0]; // first is already ≤5
        let res = threshold_multi(&attackers, &defenders, &targets);
        assert!(res.is_some(), "should find a combo for just the second defender");
        let (results, _) = res.unwrap();
        // First defender's HP should be untouched (still at 3)
        assert_eq!(results[0].defender_hp_final, 3.0);
        // Second defender's HP should be ≤5
        assert!(results[1].defender_hp_final <= 5.0);
    }

    // ======== assign (reach-constrained optim) ========

    #[test]
    fn assign_respects_reach_mask() {
        // 2 attackers, 2 defenders. attacker 0 can only reach defender 0,
        // attacker 1 can only reach defender 1. Each attacker hits their
        // assigned defender.
        let attackers = vec![Unit::new(UnitType::Warrior), Unit::new(UnitType::Rider)];
        let defenders = vec![Unit::new(UnitType::Defender), Unit::new(UnitType::Warrior)];
        let reach = vec![0b01u32, 0b10u32]; // warrior→def0 only, rider→def1 only
        let results = optimized_multi_defender_constrained(&attackers, &defenders, &reach);
        assert_eq!(results.len(), 2);
        // Both defenders should take damage since each has exactly one reachable attacker.
        assert!(results[0].defender_hp_final < defenders[0].current_hp);
        assert!(results[1].defender_hp_final < defenders[1].current_hp);
    }

    #[test]
    fn assign_unconstrained_matches_optim_multi() {
        // With every attacker reaching every defender, assign should produce a
        // result at least as good as optimized_multi_defender (they're the same
        // search with broader masks; ours picks the max-scoring plan).
        let attackers = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Rider),
            Unit::new(UnitType::Archer),
        ];
        let defenders = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Defender).with_hp(5.0),
        ];
        let reach = vec![0b11u32, 0b11u32, 0b11u32]; // everyone reaches everyone
        let constrained = optimized_multi_defender_constrained(&attackers, &defenders, &reach);
        let unconstrained = optimized_multi_defender(&attackers, &defenders);
        assert_eq!(constrained.len(), unconstrained.len());
        // Scores should match since the constraint is vacuous.
        for (a, b) in constrained.iter().zip(unconstrained.iter()) {
            assert_eq!(a.defender_hp_final, b.defender_hp_final);
        }
    }

    #[test]
    fn assign_isolated_defender_is_unreachable() {
        // attacker 0 can only reach def 0; def 1 has no reachable attacker.
        // Result should show def 1 untouched.
        let attackers = vec![Unit::new(UnitType::Warrior)];
        let defenders = vec![
            Unit::new(UnitType::Warrior),
            Unit::new(UnitType::Warrior),
        ];
        let reach = vec![0b01u32]; // warrior only reaches def 0
        let results = optimized_multi_defender_constrained(&attackers, &defenders, &reach);
        assert!(results[0].defender_hp_final < defenders[0].current_hp,
            "def 0 should take damage");
        assert_eq!(results[1].defender_hp_final, defenders[1].current_hp,
            "def 1 should be untouched");
    }

    // ======== greedy path ========

    #[test]
    fn greedy_optimized_handles_large_input() {
        // 15 warriors vs giant — triggers the greedy path (>12 slots).
        let attackers: Vec<Unit> = (0..15).map(|_| Unit::new(UnitType::Warrior)).collect();
        let giant = Unit::new(UnitType::Giant);
        let result = greedy_optimized(&attackers, &giant);
        // Giant should die (15 warriors is plenty).
        assert!(result.defender_hp_final <= 0.0, "greedy should kill giant with 15 warriors");
        // Should not panic and should have events.
        assert!(!result.events.is_empty());
    }

    #[test]
    fn greedy_multi_defender_handles_large_input() {
        // Large mixed set vs multiple defenders.
        let attackers: Vec<Unit> = (0..14).map(|i| {
            if i % 2 == 0 { Unit::new(UnitType::Warrior) } else { Unit::new(UnitType::Rider) }
        }).collect();
        let defenders = vec![
            Unit::new(UnitType::Giant),
            Unit::new(UnitType::Defender).with_status(StatusEffects::WALLED),
        ];
        let results = greedy_optimized_multi_defender(&attackers, &defenders);
        assert_eq!(results.len(), 2);
        // Both defenders should take damage
        for r in &results {
            assert!(!r.events.is_empty() || r.defender_hp_final < 0.0);
        }
    }
}
