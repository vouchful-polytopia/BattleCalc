//! Runs the competitor's Jest snapshot test suite against our bot's combat engine.
//!
//! Reads `tests/snapshot_data/*.test.js.snap`, parses each entry, executes the
//! `_cmd` through our parser + `multi_combat`, and compares the resulting
//! attacker[0].afterhp / defender.afterhp against the snapshot's expected values.
//!
//! Expected to have some mismatches — our bot's semantics intentionally differ
//! from the competitor in a few places (poison override vs multiplicative, and
//! a few edge cases where the competitor disagrees with in-game results). Output
//! groups failures by "shape" so we can triage quickly.
//!
//! Usage:
//!     cargo run --release --bin snapshot_runner
//!     cargo run --release --bin snapshot_runner -- --show 5 --pattern 'p '

use polytopia_bot::combat::{self, MultiCombatResult};
use polytopia_bot::commands::parse_battle_input;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct SnapEntry {
    cmd: String,
    expected_attacker: f32,
    expected_defender: f32,
}

fn parse_snap_file(content: &str) -> Vec<SnapEntry> {
    let mut entries = Vec::new();
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("exports[`") {
            continue;
        }
        // Parse the next 5-6 lines for _cmd, attacker, defender
        let mut cmd = String::new();
        let mut atk: Option<f32> = None;
        let mut def: Option<f32> = None;
        for inner in lines.by_ref() {
            let t = inner.trim();
            if t == "`;" || t.starts_with("`;") {
                break;
            }
            if let Some(rest) = t.strip_prefix("\"_cmd\":") {
                let rest = rest.trim_start().trim_start_matches('"');
                let end = rest.find('"').unwrap_or(rest.len());
                cmd = rest[..end].to_string();
            } else if let Some(rest) = t.strip_prefix("\"attacker\":") {
                let v = rest.trim().trim_end_matches(',').trim();
                atk = v.parse().ok();
            } else if let Some(rest) = t.strip_prefix("\"defender\":") {
                let v = rest.trim().trim_end_matches(',').trim();
                def = v.parse().ok();
            }
        }
        if !cmd.is_empty() {
            if let (Some(a), Some(d)) = (atk, def) {
                entries.push(SnapEntry {
                    cmd,
                    expected_attacker: a,
                    expected_defender: d,
                });
            }
        }
    }
    entries
}

#[derive(Debug, Clone)]
struct CaseResult {
    cmd: String,
    expected_attacker: f32,
    expected_defender: f32,
    actual_attacker: f32,
    actual_defender: f32,
    status: CaseStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum CaseStatus {
    Pass,
    ParseError(String),
    Differ,
}

fn run_case(entry: &SnapEntry) -> CaseResult {
    let mut result = CaseResult {
        cmd: entry.cmd.clone(),
        expected_attacker: entry.expected_attacker,
        expected_defender: entry.expected_defender,
        actual_attacker: f32::NAN,
        actual_defender: f32::NAN,
        status: CaseStatus::Pass,
    };

    let (attackers, defenders) = match parse_battle_input(&entry.cmd) {
        Ok(r) => r,
        Err(e) => {
            result.status = CaseStatus::ParseError(e);
            return result;
        }
    };
    if attackers.is_empty() || defenders.is_empty() {
        result.status = CaseStatus::ParseError("no attackers/defenders after parse".into());
        return result;
    }
    if defenders.len() > 1 {
        // Competitor's snapshots are single-defender only
        result.status = CaseStatus::ParseError("unexpected multi-defender in snapshot".into());
        return result;
    }

    // Default order: attackers in sequence, then defender
    let order: Vec<usize> = (0..attackers.len()).collect();
    let res: MultiCombatResult = combat::multi_combat(&attackers, &defenders[0], &order);

    // First attacker's final HP: latest event referencing attacker idx 0, or its
    // starting HP if no event exists (shouldn't happen for 1v1 snapshots).
    let atk0_final = res
        .events
        .iter()
        .rev()
        .zip(res.order.iter().rev())
        .find(|(_, &idx)| idx == 0)
        .map(|(e, _)| e.attacker_hp_after)
        .unwrap_or(attackers[0].current_hp)
        .max(0.0);
    let def_final = res.defender_hp_final.max(0.0);

    result.actual_attacker = atk0_final;
    result.actual_defender = def_final;

    let eq = |a: f32, b: f32| (a - b).abs() < 0.05;
    if eq(atk0_final, entry.expected_attacker) && eq(def_final, entry.expected_defender) {
        result.status = CaseStatus::Pass;
    } else {
        result.status = CaseStatus::Differ;
    }

    result
}

/// Hash-like signature of what DIFFERS about a failing case: which side mismatches
/// and by how much (rounded to 0.5 buckets). Used to group similar failures.
fn failure_shape(cr: &CaseResult) -> String {
    let atk_diff = cr.actual_attacker - cr.expected_attacker;
    let def_diff = cr.actual_defender - cr.expected_defender;
    let sig_atk = (atk_diff * 2.0).round() / 2.0;
    let sig_def = (def_diff * 2.0).round() / 2.0;
    let has_p = cr.cmd.split_whitespace().any(|t| t == "p");
    let has_w = cr.cmd.split_whitespace().any(|t| t == "w");
    let has_d = cr.cmd.split_whitespace().any(|t| t == "d");
    let has_s = cr.cmd.split_whitespace().any(|t| t == "s");
    let has_b = cr.cmd.split_whitespace().any(|t| t == "b");
    let has_v = cr.cmd.split_whitespace().any(|t| t == "v");
    format!(
        "atk{:+.1} def{:+.1} p={} w={} d={} s={} b={} v={}",
        sig_atk, sig_def, has_p as u8, has_w as u8, has_d as u8, has_s as u8, has_b as u8, has_v as u8
    )
}

/// Known-divergent categories. Return the category name + a human explanation.
/// "Investigate" means the failure isn't explained by a known semantic divergence.
fn categorize_failure(cr: &CaseResult) -> &'static str {
    let tokens: Vec<&str> = cr.cmd.split_whitespace().collect();
    let has_p = tokens.iter().any(|t| *t == "p");
    let has_w_and_p = has_p && tokens.iter().any(|t| *t == "w");
    let has_b = tokens.iter().any(|t| *t == "b");
    let has_s = tokens.iter().any(|t| *t == "s");

    // Competitor still applies +0.5 attack for boost; we dropped it
    // to match the actual game (swarm = movement only).
    if has_b {
        return "expected: boost no longer +0.5 attack (game change)";
    }
    // Walled + poisoned: we use 0.7 override, competitor uses base*0.5 = 2.0.
    if has_w_and_p {
        return "expected: poisoned+walled uses 0.7 override (game truncation)";
    }
    // Splash fractional difference (0.5 vs integer) is an expected drift
    // because we default to fractional half and competitor may still
    // be in integer-halved territory for some tests.
    if has_s {
        let def_diff = cr.actual_defender - cr.expected_defender;
        if (def_diff.abs() - 0.5).abs() < 0.01 {
            return "expected: splash fractional 0.5 drift";
        }
    }
    // Bare poison (not walled) difference: 0.7 vs 0.5 mult produces a
    // systematic 1-2 HP drift.
    if has_p {
        return "expected: poisoned 0.7 override vs 0.5 multiply";
    }
    // ca (range 3) attacking dr (range 2): our engine lets the catapult stay
    // out of reach, so no retaliation. Competitor treats range as a boolean
    // so retaliation still fires. Our numeric-range model is game-accurate.
    if cr.cmd.starts_with("ca ") || cr.cmd.starts_with("ca,") {
        // attacker is catapult; find defender code after comma
        if let Some(defside) = cr.cmd.split_once(',').map(|(_, r)| r.trim()) {
            if defside.starts_with("dr ") || defside.starts_with("dr,") || defside == "dr" {
                return "expected: ca vs dr uses numeric range retaliation";
            }
        }
    }
    "INVESTIGATE"
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut show_per_group: usize = 3;
    let mut pattern_filter: Option<String> = None;
    let mut single_file: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--show" => {
                i += 1;
                show_per_group = args[i].parse().unwrap_or(3);
            }
            "--pattern" => {
                i += 1;
                pattern_filter = Some(args[i].clone());
            }
            "--file" => {
                i += 1;
                single_file = Some(args[i].clone());
            }
            _ => {}
        }
        i += 1;
    }

    // Find the snapshot dir — relative to the CWD (expected: polytopia_bot/)
    let snap_dir = PathBuf::from("tests/snapshot_data");
    if !snap_dir.is_dir() {
        eprintln!("Error: expected tests/snapshot_data/ in cwd. Run from polytopia_bot/.");
        std::process::exit(2);
    }

    // Gather all .snap files
    let mut files: Vec<PathBuf> = fs::read_dir(&snap_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("snap"))
        .collect();
    files.sort();

    if let Some(f) = &single_file {
        files.retain(|p| p.file_name().and_then(|s| s.to_str()).map_or(false, |n| n.contains(f)));
    }

    let mut total_entries = 0usize;
    let mut total_pass = 0usize;
    let mut total_differ = 0usize;
    let mut total_parse_err = 0usize;
    let mut shape_groups: BTreeMap<String, Vec<CaseResult>> = BTreeMap::new();
    let mut parse_errs: Vec<CaseResult> = Vec::new();
    let mut category_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut investigate_cases: Vec<CaseResult> = Vec::new();

    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().to_string();
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skip {}: {}", name, e);
                continue;
            }
        };
        let entries = parse_snap_file(&content);
        let filtered: Vec<&SnapEntry> = if let Some(p) = &pattern_filter {
            entries.iter().filter(|e| e.cmd.contains(p.as_str())).collect()
        } else {
            entries.iter().collect()
        };

        let results: Vec<CaseResult> = filtered.par_iter().map(|e| run_case(e)).collect();

        let file_entries = results.len();
        let mut file_pass = 0usize;
        let mut file_differ = 0usize;
        let mut file_err = 0usize;
        for r in results {
            match r.status {
                CaseStatus::Pass => file_pass += 1,
                CaseStatus::ParseError(_) => {
                    file_err += 1;
                    if parse_errs.len() < 20 { parse_errs.push(r); }
                }
                CaseStatus::Differ => {
                    file_differ += 1;
                    let category = categorize_failure(&r);
                    *category_counts.entry(category).or_insert(0) += 1;
                    if category == "INVESTIGATE" {
                        investigate_cases.push(r.clone());
                    }
                    let shape = failure_shape(&r);
                    shape_groups.entry(shape).or_default().push(r);
                }
            }
        }
        total_entries += file_entries;
        total_pass += file_pass;
        total_differ += file_differ;
        total_parse_err += file_err;
        println!(
            "{:>28}  {:>6} cases, {:>6} pass, {:>5} differ, {:>4} parse_err  ({:.2}%)",
            name, file_entries, file_pass, file_differ, file_err,
            100.0 * file_pass as f64 / file_entries.max(1) as f64
        );
    }

    println!("\n================ Totals ================");
    println!(
        "Total: {}  pass: {} ({:.2}%)  differ: {}  parse_err: {}",
        total_entries,
        total_pass,
        100.0 * total_pass as f64 / total_entries.max(1) as f64,
        total_differ,
        total_parse_err,
    );

    if !parse_errs.is_empty() {
        println!("\n---- Parse errors (first {}): ----", parse_errs.len());
        for e in &parse_errs {
            if let CaseStatus::ParseError(m) = &e.status {
                println!("  `{}` → {}", e.cmd, m);
            }
        }
    }

    // Bucketed root-cause summary
    if !category_counts.is_empty() {
        println!("\n---- Failure categories ----");
        let mut cats: Vec<(&&'static str, &usize)> = category_counts.iter().collect();
        cats.sort_by_key(|(_, &n)| std::cmp::Reverse(n));
        let mut known = 0usize;
        let mut investigate = 0usize;
        for (name, count) in &cats {
            let marker = if **name == "INVESTIGATE" { "⚠" } else { "✓" };
            println!("  {} {:6}  {}", marker, count, name);
            if **name == "INVESTIGATE" { investigate += **count; } else { known += **count; }
        }
        println!("\n  Known-divergent (explained): {}", known);
        println!("  Needs investigation:         {}", investigate);
    }

    // Group INVESTIGATE cases by (attacker_code, defender_code) pattern to
    // reveal distinct unexplained root causes.
    if !investigate_cases.is_empty() {
        println!(
            "\n---- INVESTIGATE: unexplained failures (total {}) ----",
            investigate_cases.len(),
        );
        let mut by_pair: BTreeMap<String, Vec<CaseResult>> = BTreeMap::new();
        for c in &investigate_cases {
            // Extract first token of each side as the unit code
            let (left, right) = match c.cmd.split_once(',') {
                Some((l, r)) => (l.trim(), r.trim()),
                None => (c.cmd.as_str(), ""),
            };
            let lcode = left.split_whitespace().next().unwrap_or("?");
            let rcode = right.split_whitespace().next().unwrap_or("?");
            by_pair
                .entry(format!("{} vs {}", lcode, rcode))
                .or_default()
                .push(c.clone());
        }
        let mut pairs: Vec<(&String, &Vec<CaseResult>)> = by_pair.iter().collect();
        pairs.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (pair, cases) in &pairs {
            println!("  [{}]  {} cases", pair, cases.len());
            for c in cases.iter().take(show_per_group) {
                println!(
                    "      `{}`: got (atk {}, def {}), expected (atk {}, def {})",
                    c.cmd, c.actual_attacker, c.actual_defender, c.expected_attacker, c.expected_defender,
                );
            }
        }
    }

    if !shape_groups.is_empty() {
        println!(
            "\n---- Top shape groups ({} groups, {} total): ----",
            shape_groups.len(),
            total_differ,
        );
        let mut groups: Vec<(&String, &Vec<CaseResult>)> = shape_groups.iter().collect();
        groups.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (shape, cases) in groups.iter().take(15) {
            println!("  [{}]  {} cases", shape, cases.len());
            for c in cases.iter().take(show_per_group) {
                println!(
                    "      `{}`: got (atk {}, def {}), expected (atk {}, def {})",
                    c.cmd,
                    c.actual_attacker,
                    c.actual_defender,
                    c.expected_attacker,
                    c.expected_defender,
                );
            }
        }
    }

    if total_differ == 0 && total_parse_err == 0 {
        println!("\n✓ 100% parity with competitor snapshots.");
    } else {
        println!("\n{} total mismatches to triage.", total_differ + total_parse_err);
    }
}
