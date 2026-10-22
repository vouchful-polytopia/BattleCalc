use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct RawUnitData {
    attack: f32,
    defense: f32,
    hp: f32,
    range: u8,
    #[allow(unused)]
    cost: u8,
    retaliates: bool,
    surprise: bool,
    can_fortify: bool,
    can_explode: bool,
    can_splash: bool,
    promotable: bool,
    double_attack: bool,
    trait_effects: Option<String>,
    #[serde(default)]
    initial_status: Option<String>,
}

fn to_pascal(s: &str) -> String {
    let mut result = String::new();
    let mut cap_next = true;
    for c in s.chars() {
        if c == '_' {
            cap_next = true;
        } else if cap_next {
            result.push(c.to_ascii_uppercase());
            cap_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn main() {
    println!("cargo:rerun-if-changed=unit_data.yaml");
    let yaml = std::fs::read_to_string("unit_data.yaml").unwrap();
    let units: BTreeMap<String, RawUnitData> = serde_yaml::from_str(&yaml).unwrap();

    let mut out = String::new();
    out.push_str("// AUTO-GENERATED — DO NOT EDIT\n");
    out.push_str("use crate::unit::{StatusEffects, Unit, UnitType};\n\n");

    // Unit type enum variants
    out.push_str("pub const UNIT_NAMES: &[(&str, UnitType)] = &[\n");
    for name in units.keys() {
        let pascal = to_pascal(name);
        let lower = name.to_lowercase();
        out.push_str(&format!("    (\"{lower}\", UnitType::{pascal}),\n"));
    }
    out.push_str("];\n\n");

    // Unit data array
    out.push_str("pub const UNIT_TYPE_DATA: &[Unit] = &[\n");
    for (name, d) in &units {
        let pascal = to_pascal(name);
        let trait_effects = match d.trait_effects.as_deref() {
            Some("POISONED") => "StatusEffects::POISONED",
            Some("FROZEN") => "StatusEffects::FROZEN",
            Some("CONVERTED") => "StatusEffects::CONVERTED",
            _ => "StatusEffects::empty()",
        };
        let initial_status = match d.initial_status.as_deref() {
            Some("EXPLODING") => "StatusEffects::EXPLODING",
            Some("POISONED") => "StatusEffects::POISONED",
            Some("FROZEN") => "StatusEffects::FROZEN",
            Some("SPLASHING") => "StatusEffects::SPLASHING",
            _ => "StatusEffects::empty()",
        };
        out.push_str(&format!(
            "    Unit {{ unit_type: UnitType::{pascal}, attack: {a:.1}, defense: {d:.1}, current_hp: {hp:.1}, max_hp: {hp:.1}, range: {r}, retaliates: {ret}, surprise: {sur}, can_fortify: {fort}, can_explode: {exp}, can_splash: {spl}, promotable: {prom}, double_attack: {da}, is_final: false, label: None, naval_type: None, status_effects: {is}, trait_effects: {te}, defense_bonus: 1.0 }},\n",
            a = d.attack,
            d = d.defense,
            hp = d.hp,
            r = d.range,
            ret = d.retaliates,
            sur = d.surprise,
            fort = d.can_fortify,
            exp = d.can_explode,
            spl = d.can_splash,
            prom = d.promotable,
            da = d.double_attack,
            te = trait_effects,
            is = initial_status,
        ));
    }
    out.push_str("];\n");

    std::fs::write("./src/unit/generated.rs", out).expect("Failed to write generated.rs");
}
