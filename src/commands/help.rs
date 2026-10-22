use poise::CreateReply;
use serenity::all::CreateEmbed;

type Context<'a> = poise::Context<'a, crate::Data, crate::Error>;

/// Show help and examples for all commands
#[poise::command(slash_command, aliases("h"))]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Command name for detailed help"]
    command: Option<String>,
) -> Result<(), crate::Error> {
    let embed = match command.as_deref() {
        Some("c") | Some("calc") => help_calc(),
        Some("o") | Some("optim") => help_optim(),
        Some("t") | Some("threshold") => help_threshold(),
        Some("b") | Some("bulk") => help_bulk(),
        Some("e") | Some("elim") => help_elim(),
        Some("s") | Some("stats") => help_stats(),
        Some("sv") | Some("survive") => help_survive(),
        Some("as") | Some("assign") | Some("plan") => help_assign(),
        Some("greedy") | Some("exact") | Some("accuracy") => help_greedy(),
        Some("modifiers") | Some("mods") => help_modifiers(),
        Some("naval") | Some("ships") => help_naval(),
        _ => help_overview(),
    };

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

fn help_overview() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("Polytopia Calculator")
        .description("Use `/` slash commands. Separate attackers from defenders with **vs**.")
        .field(
            "Commands",
            "\
**/calc** — Simulate combat
**/optim** — Optimal attack order
**/threshold** — Min attackers to reach target HP
**/bulk** — Repeated hit count
**/elim** — One-shot thresholds
**/stats** — Unit stats / compare
**/survive** — Siege survival check
**/assign** — Optimal order with range constraints
**/help** — This help",
            false,
        )
        .field(
            "Quick Examples",
            "\
`/calc warrior vs defender d`
`/calc warrior, rider 5 vs giant`
`/optim wa, ar, ri vs gi`
`/optim wa, ar, ri vs gi, de w`  — multi-defender
`/bulk wa vs gi w`
`/elim kn vs sw`
`/stats mantis`
`/stats warrior vs mantis`  — compare",
            false,
        )
        .field(
            "More Help",
            "`/help survive` — Siege survival check\n\
`/help assign` — Range-constrained optimization\n\
`/help greedy` — Greedy vs exact solving explained\n\
`/help modifiers` — All unit modifiers (veteran, wall, poison, etc.)\n\
`/help naval` — Ship/boat syntax\n\
`/help calc` / `/help optim` / etc. — Detailed command help",
            false,
        )
}

fn help_calc() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/calc — Calculate Fight")
        .description("Simulate a combat encounter.")
        .field(
            "Syntax",
            "\
`/calc <attackers> vs <defenders>`
`/calc <units>` (last unit = defender)",
            false,
        )
        .field(
            "Examples",
            "\
`/calc warrior vs defender d` — warrior attacks protected defender
`/calc wa, ri vs de d` — warrior then rider attack
`/calc wa 7, ri 5 vs gi` — damaged attackers vs giant
`/calc wa, ri vs gi, de w` — multi-defender: fight giant first, then walled defender",
            false,
        )
}

fn help_optim() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/optim — Optimize Attack Order")
        .description("Returns the best order to use multiple attackers according to these priorities:")
        .field(
            "Priorities",
            "\
1. Kill the defender
2. Maximize effective damage dealt
3. Minimize attacker casualties
4. Maximize remaining attacker HP
5. Use fewest unique attackers
6. Use fewest total actions
7. Apply poison (tiebreaker)",
            false,
        )
        .field(
            "Single Defender",
            "`/optim wa, ar, ri vs gi` — best order to fight a giant",
            false,
        )
        .field(
            "Multiple Defenders",
            "\
`/optim wa, ar, ri, kn vs gi, de w`
Assigns attackers to defenders and finds the best ordering across all targets.",
            false,
        )
        .field(
            "Final Modifier",
            "`/optim wa, ar, ri f vs gi` — rider must deliver the killing blow",
            false,
        )
        .field(
            "Target HP",
            "\
Add `t<N>` (exact) or `t<<N>` (below) to the defender to aim for a specific end HP:
`/optim wa, wa, wa, wa vs gi t34` — leave the Giant at exactly 34 HP (errors if impossible)
`/optim wa, wa, wa vs gi t<35` — get the Giant below 35 HP with fewest units
Target HP is single-defender only and capped at 10 attackers.",
            false,
        )
        .field("Limits", "Max 20 attackers. Over 10 uses a greedy approximation (target HP is always exact, capped at 10).", false)
}

fn help_threshold() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/threshold — Threshold")
        .description("Find the minimum attackers to bring defender(s) to a target HP or below. Great for planning knight chains. Supports multiple defenders with per-unit `kn` thresholds.")
        .field(
            "Syntax",
            "\
`/threshold <target_hp> <attackers> vs <defender(s)>`
`/threshold kn <attackers> vs <defender(s)>` — knight one-shot threshold (per-unit)",
            false,
        )
        .field(
            "Examples",
            "\
`/threshold 15 wa, ar, ri vs gi` — min attackers to bring Giant ≤15hp
`/threshold kn wa, ri vs gi` — bring Giant to Knight one-shot range
`/threshold 5 wa, ri vs de d` — bring defended Defender ≤5hp
`/threshold 10 wa, wa, ar vs gi, de w` — multi-defender: both ≤10hp
`/threshold kn ri, ri vs de, wa d` — per-unit knight thresholds (different for each)",
            false,
        )
        .field(
            "How it works",
            "Tries every subset of attackers, smallest first. Returns the combo that reaches the target with fewest units, fewest casualties, and least overkill. Supports multiple defenders.",
            false,
        )
}

fn help_bulk() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/bulk — Hits to Kill")
        .description("Count repeated attacks needed to eliminate a target.")
        .field(
            "Examples",
            "\
`/bulk warrior vs giant w` — how many warriors to kill a walled giant
`/bulk ar vs de d` — archers vs protected defender",
            false,
        )
}

fn help_elim() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/elim — Elimination Threshold")
        .description("Find HP thresholds for one-shot eliminations.")
        .field(
            "Examples",
            "\
`/elim knight vs swordsman` — min knight HP to one-shot a swordsman
`/elim kn vs sw d` — vs a protected swordsman",
            false,
        )
}

fn help_stats() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/stats — Unit Stats")
        .description("Look up a unit's stats or compare two units.")
        .field(
            "Examples",
            "\
`/stats mantis` — show mantis stats
`/stats list` — list all units
`/stats warrior vs mantis` — side-by-side comparison",
            false,
        )
}

fn help_survive() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/survive — Siege Survival Check")
        .description("Check if attackers can clear blockers and kill/reach the survive target. The **last defender** is the survive target.")
        .field(
            "Syntax",
            "`/survive <attackers> vs <blockers>, <survive target>`",
            false,
        )
        .field(
            "Examples",
            "\
`/survive ri, ri, ri vs ri, ri d, wa` — 3 riders vs 2 blocker riders, can they reach the warrior?
`/survive wa, wa, ar vs sw, kn, de w` — clear 2 blockers, can they kill walled defender?
`/survive ri 4, ri 6, ri vs ri 5, ri d, wa` — damaged riders vs blockers + warrior",
            false,
        )
        .field(
            "How it works",
            "\
**Phase 1:** Finds the best split of attackers to kill all blockers.
**Phase 2:** Unused attackers (not spent on blockers) attack the survive target.
Each unit can only attack once. Tries all possible splits to maximize damage to the target.",
            false,
        )
}

fn help_assign() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("/assign — Range-Constrained Optimization")
        .description("Like `/optim` but you tag each attacker with which defenders it can reach. Use when not all attackers are in range of all defenders.")
        .field(
            "Syntax",
            "\
`/assign <attackers with tags> vs <defenders>`
Defenders are labeled **A**, **B**, **C** by position (max 3).
Add lowercase letters after each attacker to indicate which defenders it can reach.",
            false,
        )
        .field(
            "Examples",
            "\
`/assign ri ab, ri a, ar b, wa ab vs warrior, defender d`
→ Rider #1 can reach both, Rider #2 only A (warrior), Archer only B (defender), Warrior both.

`/assign wa ab, ri a, ar bc vs gi, de w, sw`
→ 3 defenders (A=giant, B=defender, C=swordsman). Warrior hits A/B, Rider only A, Archer B/C.",
            false,
        )
        .field(
            "Notes",
            "\
• If an attacker has no tag, it can reach all defenders
• Max 3 defenders, max 12 attackers
• Same scoring priorities as `/optim`",
            false,
        )
}

fn help_greedy() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("Greedy vs Exact Solving")
        .description("When you use `/optim`, `/assign`, or `/survive`, the bot searches for the best attack order. The method depends on how many units you enter.")
        .field(
            "Exact Solve",
            "\
The bot tries **every possible ordering** and picks the best one. This is guaranteed to find the true optimal result, but the number of orderings grows extremely fast (factorial).

• Exact up to **10 attackers**
• Over 10 uses greedy approximation",
            false,
        )
        .field(
            "Greedy Solve",
            "\
For larger inputs, the bot uses a **greedy approximation**:
1. Builds an ordering by picking the best next action at each step
2. Prioritizes **poisoning unpoisoned defenders** — halving their defense bonus benefits all subsequent attacks
3. Improves the result by trying pairwise swaps until no swap helps

This is fast but may miss the true optimal if a locally worse move leads to a globally better outcome. In practice it's usually correct or very close.",
            false,
        )
        .field(
            "Timeout Fallback",
            "If an exact solve takes over 60 seconds, the bot automatically falls back to greedy and shows the result with a warning.",
            false,
        )
        .field(
            "When does it matter?",
            "For most real game scenarios the greedy result matches the exact result. Differences are most likely with many diverse unit types at varied HP levels. If accuracy is critical, try to keep inputs to 10 or fewer attackers.",
            false,
        )
}

fn help_modifiers() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("Unit Modifiers")
        .description("Add after the unit name, separated by spaces.")
        .field(
            "Modifiers",
            "\
**Number** — Current HP: `warrior 7`
**v** — Veteran (+5 max HP): `warrior v`
**d** — Defense bonus (1.5x): `defender d`
**w** — Walled (4x, needs Fortify): `defender w`
**p** — Poisoned (truncates defense): `giant p`
**b** — Swarm (movement only): `warrior b`
**x** — Exploding only (self-destruct, no attack): `doomux x`
**ax** — Attack then explode — optimizer decides whether and when to detonate: `doomux ax`
**axi** — Attack then *instant* explode — explosion locked immediately after the hit: `doomux axi`
**s** — Splash (half AoE, fractional by default): `firedragon s`
**fl** — Floor splash damage (match the upcoming Polytopia integer behavior): `firedragon s fl`
**r** — Force retaliation: `catapult r`
**nr** — No retaliation (also suppresses Jelly tentacles): `defender nr`
**f** — Final (must deliver kill, /optim only): `rider f`
**fzn** — Frozen (can't retaliate): `warrior fzn`
**1h** — One hit only (multi-attack → single): `phychi 1h`",
            false,
        )
        .field(
            "Combining",
            "`warrior v 12` — veteran warrior at 12 HP\n\
`defender w v` — veteran walled defender\n\
`giant p 30` — poisoned giant at 30 HP\n\
`doomux ax 15` — Doomux at 15hp, attack then optional delayed explode",
            false,
        )
}

fn help_naval() -> CreateEmbed {
    CreateEmbed::new()
        .color(0x2b7a78)
        .title("Naval Units")
        .description("Add a ship type after the unit name. The unit keeps its HP but uses the ship's combat stats.")
        .field(
            "Ship Types",
            "\
**sc** — Scout (2 atk, 1 def, range 2)
**bo** — Bomber (3 atk, 2 def, range 3, splash, no retaliation)
**rm** — Rammer (3 atk, 3 def, range 1)
**rf** — Raft (0 atk, 1 def, no retaliation)",
            false,
        )
        .field(
            "Examples",
            "\
`/calc wa sc vs de sc` — warrior scout vs defender scout
`/optim wa bo, wa rm, wa sc vs gi` — naval fleet vs giant
`/calc de rf vs wa` — defender on a raft (bad idea)",
            false,
        )
}
