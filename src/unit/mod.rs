use arrayvec::ArrayVec;
use strum::IntoStaticStr;

mod generated;
pub use generated::{UNIT_NAMES, UNIT_TYPE_DATA};

const MAX_UNITS: usize = 12;

bitflags::bitflags! {
    #[derive(PartialEq, Eq, Hash, Debug, Copy, Clone)]
    pub struct StatusEffects: u32 {
        const BOOSTED          = 0b0000_0000_0000_0000_0001;
        const CONVERTED        = 0b0000_0000_0000_0000_0010;
        const EXPLODING        = 0b0000_0000_0000_0000_0100;
        const FORTIFIED        = 0b0000_0000_0000_0000_1000;
        const FROZEN           = 0b0000_0000_0000_0001_0000;
        const POISONED         = 0b0000_0000_0000_0010_0000;
        const SPLASHING        = 0b0000_0000_0000_0100_0000;
        const VETERAN          = 0b0000_0001_0000_0000_0000;
        const WALLED           = 0b0000_0010_0000_0000_0000;
        const ATTACK_EXPLODE   = 0b0000_0100_0000_0000_0000;
        const FORCE_RETALIATE  = 0b0000_1000_0000_0000_0000;
        const NO_RETALIATE     = 0b0001_0000_0000_0000_0000;
        const NO_EXPLODE       = 0b0010_0000_0000_0000_0000;
        const INSTANT_EXPLODE  = 0b0100_0000_0000_0000_0000;
        const FLOOR_SPLASH     = 0b1000_0000_0000_0000_0000;
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, IntoStaticStr)]
pub enum UnitType {
    Warrior,
    Rider,
    Knight,
    Defender,
    Catapult,
    Archer,
    Mindbender,
    Swordsman,
    Giant,
    Bunny,
    Polytaur,
    Dragonegg,
    Babydragon,
    Firedragon,
    Amphibian,
    Tridention,
    Mooni,
    Battlesled,
    Icefortress,
    Icearcher,
    Crab,
    Gaami,
    Hexapod,
    Doomux,
    Phychi,
    Kiton,
    Exida,
    Centipede,
    Segment,
    Raychi,
    Shaman,
    Dagger,
    Cloak,
    CloakBoat,
    Pirate,
    Bombership,
    Scoutship,
    Transportship,
    Rammership,
    Juggernaut,
    Mermaidwarrior,
    Mermaidarcher,
    Mermaidswordsman,
    Mermaiddefender,
    Mermaidcloak,
    Mermaiddagger,
    Jelly,
    Shark,
    Siren,
    Aquapult,
    Boomchi,
    Island,
    Ciru,
    Mantis,
    Bugegg,
    Moth,
    Larva,
}

#[derive(Debug, Clone)]
pub struct Unit {
    pub unit_type: UnitType,
    pub attack: f32,
    pub defense: f32,
    pub current_hp: f32,
    pub max_hp: f32,
    pub range: u8,
    pub retaliates: bool,
    pub surprise: bool,
    pub can_fortify: bool,
    pub can_explode: bool,
    pub can_splash: bool,
    pub promotable: bool,
    pub double_attack: bool,
    pub is_final: bool,
    pub label: Option<String>,
    pub naval_type: Option<&'static str>,
    pub status_effects: StatusEffects,
    pub trait_effects: StatusEffects,
    pub defense_bonus: f32,
}

impl Unit {
    pub fn new(unit_type: UnitType) -> Self {
        UNIT_TYPE_DATA.iter()
            .find(|u| u.unit_type == unit_type)
            .expect("Unit type not found in generated data")
            .clone()
    }

    pub fn with_hp(mut self, hp: f32) -> Self {
        self.current_hp = hp;
        self
    }

    pub fn with_status(mut self, effects: StatusEffects) -> Self {
        self.apply_status(effects);
        self
    }

    pub fn apply_status(&mut self, effects: StatusEffects) {
        if effects.contains(StatusEffects::VETERAN) && !self.status_effects.contains(StatusEffects::VETERAN) {
            self.max_hp += 5.0;
            self.current_hp += 5.0;
        }
        // Swarm (formerly Boost) no longer gives attack bonus — movement only
        // BOOSTED status is kept for display but has no combat effect
        self.status_effects.insert(effects);
        self.recalc_defense_bonus();
    }

    pub fn recalc_defense_bonus(&mut self) {
        // Base bonus from terrain/wall
        let base = if self.status_effects.contains(StatusEffects::WALLED) {
            if self.can_fortify { 4.0 } else { 1.5 }
        } else if self.status_effects.contains(StatusEffects::FORTIFIED) {
            1.5
        } else {
            1.0
        };
        // Poison is a 50% multiplier on the defense bonus. The resulting float
        // (e.g. fortified 1.5*0.5 = 0.75) is truncated to integer via `as i64`
        // in combat.rs, matching the game's integer math (15/2 = 7 → 0.7).
        self.defense_bonus = if self.status_effects.contains(StatusEffects::POISONED) {
            base * 0.5
        } else {
            base
        };
    }

    pub fn display_name(&self) -> String {
        let base: &'static str = self.unit_type.into();
        let mut parts = Vec::new();
        if self.status_effects.contains(StatusEffects::VETERAN) {
            parts.push("Vet".to_string());
        }
        if self.status_effects.contains(StatusEffects::BOOSTED) {
            parts.push("Boosted".to_string());
        }
        parts.push(base.to_string());
        if let Some(ref lbl) = self.label {
            parts.push(format!("#{}", lbl));
        }
        if let Some(ship) = self.naval_type {
            parts.push(ship.to_string());
        }
        let mut name = parts.join(" ");
        if self.status_effects.contains(StatusEffects::POISONED) {
            name.push_str(" (psn)");
        }
        if self.status_effects.contains(StatusEffects::FROZEN) {
            name.push_str(" (fzn)");
        }
        if self.status_effects.contains(StatusEffects::EXPLODING) {
            name.push_str(" 💥");
        }
        if self.status_effects.contains(StatusEffects::SPLASHING) {
            name.push_str(" 💦");
        }
        let bonus = if self.status_effects.contains(StatusEffects::WALLED) {
            " [W]"
        } else if self.status_effects.contains(StatusEffects::FORTIFIED) {
            " [D]"
        } else {
            ""
        };
        name.push_str(bonus);
        name
    }
}

#[derive(Debug, Clone)]
pub struct Units(pub ArrayVec<Unit, MAX_UNITS>);

impl Units {
    pub fn new() -> Self {
        Self(ArrayVec::new())
    }

    pub fn push(&mut self, unit: Unit) {
        self.0.push(unit);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Unit> {
        self.0.iter()
    }
}

impl std::ops::Index<usize> for Units {
    type Output = Unit;
    fn index(&self, i: usize) -> &Unit {
        &self.0[i]
    }
}

impl std::ops::IndexMut<usize> for Units {
    fn index_mut(&mut self, i: usize) -> &mut Unit {
        &mut self.0[i]
    }
}

impl<const N: usize> From<[Unit; N]> for Units {
    fn from(arr: [Unit; N]) -> Self {
        let mut u = Units::new();
        u.0.extend(arr);
        u
    }
}

pub fn find_unit(name: &str) -> Option<UnitType> {
    let lower = name.to_lowercase();
    // Exact match first
    for &(n, t) in UNIT_NAMES {
        if n == lower {
            return Some(t);
        }
    }
    // Alias table
    let alias = match lower.as_str() {
        "wa" | "war" => Some("warrior"),
        "ri" | "rid" => Some("rider"),
        "kn" | "kni" => Some("knight"),
        "de" | "def" => Some("defender"),
        "ca" | "cat" => Some("catapult"),
        "ar" | "arc" => Some("archer"),
        "mb" | "mind" | "mindbender" => Some("mindbender"),
        "sw" | "swo" | "sword" => Some("swordsman"),
        "gi" | "gia" => Some("giant"),
        "po" | "pol" | "poly" => Some("polytaur"),
        "tr" | "tri" | "trid" => Some("tridention"),
        "am" | "amp" | "amph" => Some("amphibian"),
        "mo" | "moo" => Some("mooni"),
        "bs" | "sled" => Some("battlesled"),
        "if" | "icef" => Some("icefortress"),
        "ia" | "icea" => Some("icearcher"),
        "cr" | "cra" => Some("crab"),
        "ga" | "gaa" => Some("gaami"),
        "he" | "hex" => Some("hexapod"),
        "do" | "doo" | "doom" => Some("doomux"),
        "ph" | "phy" => Some("phychi"),
        "ki" | "kit" => Some("kiton"),
        "ex" | "exi" => Some("exida"),
        "ce" | "cen" | "cent" => Some("centipede"),
        "se" | "seg" => Some("segment"),
        "ra" | "ray" => Some("raychi"),
        "sh" | "sha" => Some("shaman"),
        "da" | "dag" => Some("dagger"),
        "cl" | "clo" => Some("cloak"),
        "pi" | "pir" => Some("pirate"),
        "bo" | "bomb" => Some("bombership"),
        "scsh" | "scoutship" => Some("scoutship"),
        "rm" | "ram" | "rammer" => Some("rammership"),
        "ju" | "jug" => Some("juggernaut"),
        "je" | "jel" => Some("jelly"),
        "sk" | "sha2" => Some("shark"),
        "si" | "sir" => Some("siren"),
        "aq" | "aqua" => Some("aquapult"),
        "bm" | "boom" => Some("boomchi"),
        "li" | "isl" => Some("island"),
        "ci" | "cir" => Some("ciru"),
        "ma" | "man" | "mant" => Some("mantis"),
        "be" | "bug" | "bugegg" => Some("bugegg"),
        "mt" | "mot" => Some("moth"),
        "la" | "lar" => Some("larva"),
        "dr" | "fire" | "firedrag" => Some("firedragon"),
        "bd" | "baby" | "babydrag" => Some("babydragon"),
        "egg" | "degg" => Some("dragonegg"),
        "mw" | "merwar" => Some("mermaidwarrior"),
        "marc" | "merarc" => Some("mermaidarcher"),
        "msw" | "merswo" => Some("mermaidswordsman"),
        "md" | "merdef" => Some("mermaiddefender"),
        "mcl" | "merclo" => Some("mermaidcloak"),
        "mda" | "merdag" => Some("mermaiddagger"),
        "pf" | "puf" | "puff" => Some("phychi"), // puffer is old name
        "rf" | "raft" | "trans" => Some("transportship"),
        _ => None,
    };
    alias.and_then(|a| {
        UNIT_NAMES.iter().find(|&&(n, _)| n == a).map(|&(_, t)| t)
    })
}
