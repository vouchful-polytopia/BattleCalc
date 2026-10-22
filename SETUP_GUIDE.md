# Polytopia Calculator Bot — Setup Guide

## Step 1: Install Rust

Open a terminal (PowerShell or Command Prompt) and paste:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If that doesn't work on Windows, download the installer from:
**https://rustup.rs** — click the big download button, run the `.exe`.

When it asks, just press Enter for the default installation.

**Close and reopen your terminal** after installing so `cargo` is available.

Verify it worked:
```
cargo --version
```
You should see something like `cargo 1.XX.0`.

---

## Step 2: Create a Discord Bot

1. Go to **https://discord.com/developers/applications**
2. Log in with your Discord account
3. Click **"New Application"** (top right)
4. Name it whatever you want (e.g. "PolyCalc")
5. Click **"Create"**

### Get the Bot Token

6. In the left sidebar, click **"Bot"**
7. Click **"Reset Token"** and confirm
8. **Copy the token** — save it somewhere safe (you'll need it in Step 4)
   - You will NEVER be able to see this token again. If you lose it, you'll need to reset it.

### Enable Message Content Intent

9. Scroll down on the Bot page
10. Under **"Privileged Gateway Intents"**, turn ON **"Message Content Intent"**
11. Click **"Save Changes"**

This is required for the `#` prefix commands to work.

### Invite the Bot to Your Server

12. In the left sidebar, click **"OAuth2"**
13. Click **"URL Generator"**
14. Under **Scopes**, check: `bot`, `applications.commands`
15. Under **Bot Permissions**, check:
    - Send Messages
    - Embed Links
    - Read Message History
    - Use Slash Commands
16. Copy the **Generated URL** at the bottom
17. Paste it in your browser
18. Select your server and click **"Authorize"**

---

## Step 3: Build the Bot

Open a terminal and navigate to the bot folder:

```
cd BattleCalc
```

Build it:

```
cargo build --release
```

First build takes 2-5 minutes (it downloads and compiles dependencies). Subsequent builds are fast.

If you get errors about missing C compiler:
- Install **Visual Studio Build Tools** from https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Select "Desktop development with C++" workload
- Restart terminal and try again

---

## Step 4: Run the Bot

Set the token and run:

**PowerShell:**
```powershell
$env:DISCORD_TOKEN = "paste-your-token-here"
.\target\release\polytopia-bot.exe
```

**Command Prompt:**
```cmd
set DISCORD_TOKEN=paste-your-token-here
.\target\release\polytopia-bot.exe
```

**Git Bash:**
```bash
DISCORD_TOKEN="paste-your-token-here" ./target/release/polytopia-bot.exe
```

You should see: `Bot is online! Prefix: #`

The bot will register slash commands globally (takes up to an hour to appear in all servers, but prefix commands work immediately).

---

## Step 5: Use It

### Prefix Commands (work immediately)

```
#c warrior vs defender d
#o warrior, archer, rider vs giant
#o warrior, archer, rider vs giant, defender w
#b warrior vs giant w
#e knight vs swordsman
#s mantis
#s warrior vs mantis
#h
```

### Slash Commands (may take up to 1 hour to appear)

```
/calc warrior vs defender d
/optim warrior, archer, rider vs giant, defender w
/bulk warrior vs giant w
/elim knight vs swordsman
/stats mantis
/help
```

---

## Quick Syntax Reference

### Units
Use full names or abbreviations:
`warrior` or `wa`, `rider` or `ri`, `knight` or `kn`, `defender` or `de`,
`archer` or `ar`, `catapult` or `ca`, `giant` or `gi`, `swordsman` or `sw`,
`mantis` or `ma`, `hexapod` or `he`, `kiton` or `ki`, `phychi` or `ph`,
`doomux` or `do`, `centipede` or `ce`, `boomchi` or `bm`, `island` or `li`

### Modifiers (after unit name)
- `7` — HP is 7
- `v` — veteran (+5 max HP)
- `d` — defense bonus (1.5x)
- `w` — walled (4x, only units with Fortify)
- `psn` — poisoned (-30% defense)
- `b` — boosted (+0.5 attack)
- `x` — exploding (half damage, self-destruct)
- `sp` — splash (half damage AoE)

### Naval Units (after unit name)
- `sc` — scoutship
- `bo` — bombership
- `rm` — rammership
- `rf` — raft

### Separators
- `,` separates units
- `vs` separates attackers from defenders
- Multiple defenders after `vs` are fought sequentially

### Examples
```
#c wa vs de d               — warrior attacks protected defender
#c wa 7, ri vs gi           — damaged warrior + rider vs giant
#o wa, ar, ri vs gi, de w   — optimize: fight giant, then walled defender
#c wa sc vs de sc            — warrior scoutship vs defender scoutship
#b ar vs gi w                — how many archers to kill walled giant
#s mantis                    — show mantis stats
#s warrior vs mantis         — compare warrior and mantis
```

---

## Troubleshooting

**"Set DISCORD_TOKEN env var"** — You didn't set the token. See Step 4.

**Bot is online but doesn't respond to `#c`** — Make sure Message Content Intent is ON (Step 2, item 10).

**Slash commands don't appear** — They take up to 1 hour to register globally. Prefix commands (`#c`) work immediately.

**"Unknown unit"** — Use `#s list` to see all valid unit names and abbreviations.

**Bot goes offline when you close the terminal** — The bot runs as long as the terminal is open. To run it in the background, use `start /b` (Windows) or a process manager.

---

## Updating Unit Stats

Edit `unit_data.yaml` and rebuild:
```
cargo build --release
```
The stats are baked in at compile time — no runtime file loading needed.
