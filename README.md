# Polytopia Calculator

A Discord bot for [The Battle of Polytopia](https://polytopia.io) that does combat math the way the actual game does it — and then uses that math to solve planning problems you'd otherwise work out on paper.

Add the bot to your server here: https://discord.com/oauth2/authorize?client_id=1486369442836385972

```
/optim wa, ar, ri, kn vs gi, de w
```

The bot figures out which attacker hits which defender, in what order, to kill the most, lose the least, and spend the fewest units doing it.

---

## Why this bot

Most Polytopia calculators approximate the combat formula. This one uses the game's integer math, rounding rule and defense-bonus semantics, so the numbers should match what you see in game. If they disagree, the game is what we trust.

On top of that correct base, the bot ships a set of planners that go beyond simple 1v1 math:

- **Exact optimal attack order** — searches every valid permutation of your attackers (up to 10) and returns the provably best one. Dedup pruning skips redundant orderings of identical units; dominated branches are cut early.
- **Multi-defender planning** — when you have two or three defenders, it partitions your attackers across targets *and* orders them within each target simultaneously. Parallelized with [rayon](https://docs.rs/rayon).
- **Range-constrained assignment** — tag each attacker with which defenders it can physically reach, and the solver respects those reach masks. Covers the common real-game case where not every unit is in range of every target.
- **Target HP mode** — ask for a *specific* final HP instead of maximum damage. Useful for setting up knight chains, next-turn kills, or leaving a defender at exactly the right HP for a follow-up.
- **Final modifier (`f`)** — force a named unit to deliver the killing blow. XP farming on knights, riders, or whichever unit you want promoted.
- **Minimum-attacker threshold** — the inverse search: what's the smallest subset of attackers that brings the defender to ≤ N HP? Includes a `kn` shortcut that automatically targets knight one-shot range.
- **Siege survival check** — two-phase planner: clear blockers, then hit a survive target with whatever's left. Tries every split.
- **Greedy fallback for 11+ attackers** — for inputs too large to search exhaustively, the bot constructs a greedy ordering (poison-first) and improves it with pairwise swaps. Exact solves that exceed 60s fall back automatically.

If none of that's useful, `/calc` is still a faster, more accurate 1v1 simulator than most alternatives, with a 2000-case snapshot suite regression-tested against the actual game.

---

## Commands

| Command | What it does |
|---|---|
| `/calc` | Simulate a fight in a user-specified order. Supports multi-attacker, multi-defender. |
| `/optim` | Best attack order. Single or multi-defender. Supports target HP and the `f` modifier. |
| `/assign` | `/optim` with reach constraints — tag which attackers can hit which defenders. |
| `/threshold` | Minimum attackers to bring defender(s) to ≤ target HP. Supports per-unit `kn` mode. |
| `/survive` | Siege planner: clear blockers, then damage/kill the survive target. |
| `/bulk` | How many identical hits are needed to kill a target. |
| `/elim` | HP thresholds for one-shot elimination. |
| `/stats` | Unit stats lookup and side-by-side comparison. |
| `/help` | Per-command help, including modifiers and naval syntax. |

All commands also work with a `#` prefix: `#c wa vs de d` is the same as `/calc warrior vs defender d`.

---

## Syntax at a glance

```
/calc <attackers> vs <defenders>
```

**Units** — full names or abbreviations (`warrior`/`wa`, `defender`/`de`, `rider`/`ri`, `knight`/`kn`, `phychi`/`ph`, `doomux`/`do`, etc.). `/stats list` prints the full catalog.

**HP** — a bare number after the unit: `warrior 7` is a warrior at 7 HP. If the number exceeds base max but is within the veteran range, the unit is auto-promoted.

**Modifiers** — space-separated tags after the unit name:

| Tag | Effect |
|---|---|
| `v` | Veteran (+5 max HP) |
| `d` | Fortified (1.5x defense) |
| `w` | Walled (4x defense — needs Fortify ability) |
| `p` | Poisoned (defense replaced with 0.7x — matches the game, not common approximations) |
| `b` | Swarm marker (cosmetic — no attack bonus per current game) |
| `x` | Exploding (self-destruct only) |
| `ax` | Attack, then optionally explode (optimizer decides) |
| `axi` | Attack, then instantly explode (bundled) |
| `s` | Splash hit (half damage, fractional) |
| `fl` | Floor splash to integer (next-version semantics) |
| `r` | Force retaliation |
| `nr` | No retaliation (also suppresses Jelly tentacles) |
| `f` | Must deliver the killing blow (`/optim` only) |
| `fzn` | Frozen — can't retaliate |
| `1h` | Single hit (disables double-attack, Phychi/etc.) |

**Naval** — add a ship tag after the unit: `wa sc` is a warrior embarked on a scoutship. Ship stats override attack/defense/range; the embarked unit keeps its HP and tribe abilities.

| Tag | Ship |
|---|---|
| `sc` | Scoutship |
| `bo` | Bombership |
| `rm` | Rammership |
| `rf` | Raft |

**Target HP** — append to the defender in `/optim`: `gi t34` (exactly 34), `gi t<35` (below 35, fewest units).

---

## Examples

```
/calc warrior vs defender d
/calc wa 7, ri vs gi
/calc wa, ri vs gi, de w          ← sequential multi-defender

/optim wa, ar, ri vs gi            ← best order, single target
/optim wa, ar, ri, kn vs gi, de w  ← multi-defender: partition + order
/optim wa, wa, wa vs gi t<35       ← bring giant below 35 HP, fewest units
/optim wa, kn f vs wa              ← knight must land the killing blow

/assign ri ab, ri a, ar b vs wa, de d
  ← rider#1 reaches both, rider#2 only A, archer only B

/threshold kn wa, ri vs gi         ← min attackers for knight one-shot range
/threshold 5 wa, ri vs de d, wa    ← multi-defender: both ≤5 HP

/survive ri, ri, ri vs ri, ri d, wa
  ← kill 2 riders, then attack the warrior with what's left

/bulk ar vs gi w                   ← archers needed to kill a walled giant
/elim kn vs sw                     ← min knight HP to one-shot a swordsman
/stats warrior vs mantis           ← side-by-side comparison
```

---

## Supported units

All base-game units plus every tribe-specific unit:

- **Imperius / base** — Warrior, Rider, Knight, Defender, Archer, Catapult, Swordsman, Giant, Mind Bender, Dagger, Cloak, Pirate
- **Aquarion** — Amphibian, Tridention, Crab, Jelly, Shark, Siren, Aquapult, Mermaid variants
- **Polaris** — Mooni, Battlesled, Icefortress, Icearcher, Gaami
- **Cymanti** — Phychi, Kiton, Exida, Centipede, Segment, Raychi, Shaman, Doomux, Hexapod
- **∑∫ỹriȱŋ / Elyrion** — Polytaur, Dragonegg, Babydragon, Firedragon
- **Nature / other** — Ciru, Mantis, Bugegg, Moth, Larva, Bunny, Island, Boomchi, Juggernaut

All naval forms (Scoutship, Bombership, Rammership, Transportship) are supported both as standalone units and as carriers.

---

## Self-hosting

Requires a recent stable Rust toolchain and a Discord bot token. See [SETUP_GUIDE.md](SETUP_GUIDE.md) for creating the bot application and inviting it.

```bash
cargo build --release
DISCORD_TOKEN=<your-bot-token> ./target/release/polytopia-bot
```

Slash commands are registered globally on startup, so they can take a little while to appear in Discord.

An optional `config.json` in the working directory configures the admin-only `/deathmsg` command:

```json
{
  "admin_user_id": "<your-discord-user-id>",
  "death_messages": []
}
```

Without it the bot runs normally; `/deathmsg` is simply unavailable and kills show as `[killed]`.

Unit stats live in `unit_data.yaml` and are processed by `build.rs` at build time.

## Tests

```bash
cargo test
```

`tests/game_formula_tests.rs` checks the combat math against reference formulas.

## Legal

BattleCalc is an unofficial community project, not affiliated with Midjiwan AB or The Battle of Polytopia. The hosted instance is covered by the [Terms of Service](TERMS_OF_SERVICE.md) and [Privacy Policy](PRIVACY_POLICY.md).
