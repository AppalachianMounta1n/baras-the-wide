use crate::event_models::*;
use memchr::memchr;
use memchr::memchr_iter;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::option::Option;
use std::path::Path;

macro_rules! parse_i64 {
    ($s:expr) => {
        $s.parse::<i64>().unwrap_or_default()
    };
}
macro_rules! parse_i32 {
    ($s:expr) => {
        $s.parse::<i32>().unwrap_or_default()
    };
}
pub fn parse_log_file<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CombatEvent>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let bytes = mmap.as_ref();

    // Find all line boundaries
    let mut line_ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for end in memchr_iter(b'\n', bytes) {
        if end > start {
            line_ranges.push((start, end));
        }
        start = end + 1;
    }
    if start < bytes.len() {
        line_ranges.push((start, bytes.len()));
    }

    let events: Vec<CombatEvent> = line_ranges
        .par_iter()
        .enumerate()
        .filter_map(|(idx, &(start, end))| {
            let line = unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) };
            parse_line(idx + 1, line)
        })
        .collect();

    Ok(events)
}

fn parse_line(line_number: usize, _line: &str) -> Option<CombatEvent> {
    let (_remaining, ts) = parse_timestamp(_line)?;
    let (_remaining, source_entity) = parse_entity(_remaining)?;
    let (_remaining, target_entity) = parse_entity(_remaining)?;

    let (_remaining, action) = parse_action(_remaining)?;

    let target_entity = if target_entity.entity_type == EntityType::SelfReference {
        source_entity.clone()
    } else {
        target_entity
    };
    let (_remaining, effect) = parse_effect(_remaining)?;
    let details = parse_details(_remaining, &effect.name)?;

    let event = CombatEvent {
        line_number,
        timestamp: ts,
        source_entity,
        target_entity,
        action,
        effect,
        details,
    };

    Some(event)
}

pub fn parse_timestamp(input: &str) -> Option<(&str, Timestamp)> {
    let b = input.as_bytes();
    if b.len() < 14 || b[0] != b'[' || b[3] != b':' || b[6] != b':' || b[9] != b'.' || b[13] != b']'
    {
        return None;
    }

    let hour = (b[1] - b'0') * 10 + (b[2] - b'0');
    let minute = (b[4] - b'0') * 10 + (b[5] - b'0');
    let second = (b[7] - b'0') * 10 + (b[8] - b'0');
    let millis = (b[10] - b'0') as u16 * 100 + (b[11] - b'0') as u16 * 10 + (b[12] - b'0') as u16;

    Some((
        &input[14..],
        Timestamp {
            hour,
            minute,
            second,
            millis,
        },
    ))
}

// [Dread Master Bestia {3273941900591104}:5320000112163|(137.28,-120.98,-8.85,81.28)|(0/19129210)]
// [@Galen Ayder#690129185314118|(-4700.43,-4750.48,710.03,-0.71)|(1/414851)]
// [@Jerran Zeva#689501114780828/Raina Temple {493328533553152}:87481369009487|(4749.87,4694.53,710.05,0.00)|(288866/288866)]

pub fn parse_entity(input: &str) -> Option<(&str, Entity)> {
    let bytes = input.as_bytes();
    let segment_start_pos = memchr(b'[', bytes)?;
    let segment_end_pos = memchr(b']', bytes)?;
    let self_target_pos = memchr(b'=', bytes);
    if segment_end_pos <= 2 {
        return Some((
            &input[segment_end_pos + 1..],
            Entity {
                ..Default::default()
            },
        ));
    }

    if self_target_pos.is_some_and(|x| x == 2) {
        return Some((
            &input[segment_end_pos + 1..],
            Entity {
                entity_type: EntityType::SelfReference,
                ..Default::default()
            },
        ));
    }

    let pipe_pos: Vec<usize> = memchr_iter(b'|', bytes).collect();
    let name_segment = &input[segment_start_pos + 1..pipe_pos[0]];
    let _ = &input[pipe_pos[0] + 1..pipe_pos[1]]; // coordinates ignore for now not used
    let health_segment = &input[pipe_pos[1]..segment_end_pos];

    let (name, class_id, log_id, entity_type) = parse_entity_name_id(name_segment)?;
    let health = parse_entity_health(health_segment)?;

    Some((
        &input[segment_end_pos + 1..],
        Entity {
            name: name.to_string(),
            class_id,
            log_id,
            entity_type,
            health,
        },
    ))
}

pub fn parse_entity_health(input: &str) -> Option<(i32, i32)> {
    let bytes = input.as_bytes();
    let health_start_pos = memchr(b'(', bytes);
    let health_delim_pos = memchr(b'/', bytes);
    let health_end_pos = memchr(b')', bytes);

    let current_health = parse_i32!(&input[health_start_pos? + 1..health_delim_pos?]);
    let health_end_pos = parse_i32!(&input[health_delim_pos? + 1..health_end_pos?]);

    Some((current_health, health_end_pos))
}

pub fn parse_entity_name_id(input: &str) -> Option<(&str, i64, i64, EntityType)> {
    let bytes = input.as_bytes();

    let end_brack_pos = memchr(b'}', bytes);
    let start_brack_pos = memchr(b'{', bytes);
    let name_delim_pos = memchr(b'#', bytes);
    let companion_delim_pos = memchr(b'/', bytes);

    // Parse Player and Player Companion
    if name_delim_pos.is_some() {
        let player_name = &input[1..name_delim_pos?];

        if companion_delim_pos.is_none() {
            let player_id = parse_i64!(&input[name_delim_pos? + 1..]);

            return Some((player_name, 0, player_id, EntityType::Player));
        } else {
            let companion_name = &input[companion_delim_pos? + 1..start_brack_pos? - 1];
            let companion_char_id = parse_i64!(&input[start_brack_pos? + 1..end_brack_pos?]);
            let companion_log_id = parse_i64!(&&input[end_brack_pos? + 2..]);

            return Some((
                companion_name,
                companion_char_id,
                companion_log_id,
                EntityType::Companion,
            ));
        }
    }

    // if no '#' detected parse NPC
    let npc_name = input[..start_brack_pos?].trim();
    let npc_char_id = parse_i64!(&input[start_brack_pos? + 1..end_brack_pos?]);
    let npc_log_id = parse_i64!(&input[end_brack_pos? + 2..]);

    Some((npc_name, npc_char_id, npc_log_id, EntityType::Npc))
}

pub fn parse_action(input: &str) -> Option<(&str, Action)> {
    let bytes = input.as_bytes();

    let segment_start_pos = memchr(b'[', bytes)?;
    let segment_end_pos = memchr(b']', bytes)?;
    let end_brack_pos = memchr(b'}', bytes);
    let start_brack_pos = memchr(b'{', bytes);
    if segment_end_pos <= 2 {
        return Some((
            &input[segment_end_pos + 1..],
            Action {
                ..Default::default()
            },
        ));
    }

    let action_name = input[segment_start_pos + 1..start_brack_pos?]
        .trim()
        .to_string();
    let action_id = parse_i64!(input[start_brack_pos? + 1..end_brack_pos?]);

    Some((
        &input[segment_end_pos + 1..],
        Action {
            name: action_name,
            action_id,
        },
    ))
}

pub fn parse_effect(input: &str) -> Option<(&str, Effect)> {
    let bytes = input.as_bytes();
    let segment_start_pos = memchr(b'[', bytes)?;
    let segment_end_pos = memchr(b']', bytes)?;
    let segment = &input[segment_start_pos + 1..segment_end_pos];

    let effect = if segment.starts_with("DisciplineChanged") {
        parse_discipline_changed(segment)?
    } else if segment.starts_with("AreaEntered") {
        parse_area_entered(segment)?
    } else {
        parse_standard_effect(segment)?
    };

    Some((&input[segment_end_pos + 1..], effect))
}

fn parse_discipline_changed(segment: &str) -> Option<Effect> {
    let bytes = segment.as_bytes();

    let brackets: Vec<usize> = memchr_iter(b'{', bytes).collect();
    let end_brackets: Vec<usize> = memchr_iter(b'}', bytes).collect();
    let slash_pos = memchr(b'/', bytes)?;

    let type_name = segment[..brackets[0]].trim().to_string();
    let type_id = parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]);
    let name = segment[end_brackets[0] + 2..brackets[1] - 1]
        .trim()
        .to_string();
    let id = parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]);
    let discipline_name = segment[slash_pos + 1..brackets[2] - 1].trim().to_string();
    let discipline_id = parse_i64!(&segment[brackets[2] + 1..end_brackets[2]]);

    Some(Effect {
        type_id,
        type_name,
        name,
        id,
        discipline_name: Some(discipline_name),
        discipline_id: Some(discipline_id),
        ..Default::default()
    })
}

fn parse_area_entered(segment: &str) -> Option<Effect> {
    let bytes = segment.as_bytes();
    let brackets: Vec<usize> = memchr_iter(b'{', bytes).collect();
    let end_brackets: Vec<usize> = memchr_iter(b'}', bytes).collect();

    if brackets.len() < 2 || end_brackets.len() < 2 {
        return Some(Effect {
            ..Default::default()
        });
    }

    let type_id = parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]);
    let type_name = segment[..end_brackets[0]].trim().to_string();
    let area_name = segment[end_brackets[0] + 2..brackets[1] - 1]
        .trim()
        .to_string();
    let area_id = parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]);

    // Difficulty is optional - check if there's a third bracket pair
    let (difficulty_name, difficulty_id) = if brackets.len() >= 3 && end_brackets.len() >= 3 {
        let diff_name = segment[end_brackets[1] + 1..brackets[2] - 1]
            .trim()
            .to_string();
        let diff_id = parse_i64!(&segment[brackets[2] + 1..end_brackets[2]]);
        (Some(diff_name), Some(diff_id))
    } else {
        (None, None)
    };

    Some(Effect {
        type_id,
        type_name,
        name: area_name,
        id: area_id,
        difficulty_name,
        difficulty_id,
        ..Default::default()
    })
}
fn parse_standard_effect(segment: &str) -> Option<Effect> {
    let bytes = segment.as_bytes();
    let brackets: Vec<usize> = memchr_iter(b'{', bytes).collect();
    let end_brackets: Vec<usize> = memchr_iter(b'}', bytes).collect();

    if brackets.len() < 2 {
        return Some(Effect {
            ..Default::default()
        });
    }

    Some(Effect {
        type_name: segment[..brackets[0]].trim().to_string(),
        type_id: parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]),
        name: segment[end_brackets[0] + 2..brackets[1] - 1]
            .trim()
            .to_string(),
        id: parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]),
        ..Default::default()
    })
}

fn parse_details(segment: &str, effect_name: &str) -> Option<Details> {
    if effect_name == "Damage" {
        return parse_dmg_details(segment);
    } else if effect_name == "Heal" {
        return parse_heal_details(segment);
    } else if (effect_name == "ModifyCharges" || effect_name == "ApplyEffect")
        && memchr(b'(', segment.as_bytes()).is_some()
    {
        return parse_charges(segment);
    }

    Some(Details {
        ..Default::default()
    })
}

fn parse_dmg_details(segment: &str) -> Option<Details> {
    let bytes = segment.as_bytes();

    // Find main delimiters
    let paren_start = memchr(b'(', bytes)?;
    let paren_end = rfind_matching_paren(bytes, paren_start)?;
    let angle_start = memchr(b'<', bytes);
    let angle_end = memchr(b'>', bytes);

    let inner = &segment[paren_start + 1..paren_end];
    let inner_bytes = inner.as_bytes();

    // Parse threat from <value>
    let threat = angle_start
        .zip(angle_end)
        .and_then(|(s, e)| segment[s + 1..e].parse::<f32>().ok())
        .unwrap_or_default();

    // Handle edge case: (0 -) - nullified damage from reflect
    if inner.trim() == "0 -" {
        return Some(Details {
            dmg_amount: 0,
            is_reflect: true,
            threat,
            ..Default::default()
        });
    }

    // Check for crit marker
    let is_crit = memchr(b'*', inner_bytes).is_some();

    // Check for reflected marker
    let is_reflect = inner.contains("reflected");

    // Check for avoidance (-miss, -dodge, -parry, -immune, -resist, -deflect, -shield)
    // But not the bare "-" from (0 -)
    let avoid_pos = inner.find(" -").map(|p| p + 1); // find " -" to avoid matching other dashes
    let avoid_type = avoid_pos.and_then(|pos| {
        let after_dash = &inner[pos + 1..];
        // Skip if it's just "-" alone
        if after_dash.is_empty() {
            return None;
        }
        let end = after_dash
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(after_dash.len());
        let avoid = &after_dash[..end];
        if avoid.is_empty() {
            None
        } else {
            Some(avoid.to_string())
        }
    });

    // Parse amount (first number)
    let amount_end = inner
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(inner.len());
    let dmg_amount = parse_i32!(&inner[..amount_end]);

    // Parse effective damage after ~
    let effective_pos = memchr(b'~', inner_bytes);
    let dmg_effective = effective_pos
        .map(|pos| {
            let start = pos + 1;
            let end = inner[start..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|e| start + e)
                .unwrap_or(inner.len());
            parse_i32!(&inner[start..end])
        })
        .unwrap_or(dmg_amount);
    // Find damage type and ID (first { } pair in inner, but not "reflected" or "absorbed")
    let brace_start = memchr(b'{', inner_bytes);
    let brace_end = memchr(b'}', inner_bytes);

    let (dmg_type, dmg_type_id) = if let (Some(bs), Some(be)) = (brace_start, brace_end) {
        // Find type name before the brace - scan backwards for a word
        let type_start = inner[..bs]
            .rfind(|c: char| c.is_whitespace())
            .map(|p| p + 1)
            .unwrap_or(0);
        let dmg_type = inner[type_start..bs].trim().to_string();
        let dmg_type_id = parse_i64!(&inner[bs + 1..be]);
        (dmg_type, dmg_type_id)
    } else {
        (String::new(), 0)
    };

    // Parse absorbed amount from nested (X absorbed {id})
    let dmg_absorbed = if let Some(absorbed_pos) = inner.find("absorbed") {
        let before_absorbed = &inner[..absorbed_pos];
        if let Some(nested_paren) = before_absorbed.rfind('(') {
            let absorbed_str = &inner[nested_paren + 1..absorbed_pos].trim();
            Some(parse_i32!(absorbed_str))
        } else {
            None
        }
    } else {
        None
    };

    Some(Details {
        dmg_amount,
        is_crit,
        is_reflect,
        dmg_effective,
        dmg_type,
        dmg_type_id,
        avoid_type,
        dmg_absorbed,
        threat,
        ..Default::default()
    })
}
/// Find matching closing paren, handling nested parens
fn rfind_matching_paren(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_heal_details(segment: &str) -> Option<Details> {
    let bytes = segment.as_bytes();

    // Find main delimiters
    let paren_start = memchr(b'(', bytes)?;
    let paren_end = memchr(b')', bytes)?;
    let angle_start = memchr(b'<', bytes);
    let angle_end = memchr(b'>', bytes);

    let inner = &segment[paren_start + 1..paren_end];
    let inner_bytes = inner.as_bytes();

    // Parse threat from <value> - only present if effective heal occurred
    let threat = angle_start
        .zip(angle_end)
        .and_then(|(s, e)| segment[s + 1..e].parse::<f32>().ok())
        .unwrap_or_default();

    // Check for crit marker
    let is_crit = memchr(b'*', inner_bytes).is_some();

    // Parse heal amount (first number)
    let amount_end = inner
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(inner.len());
    let heal_amount = parse_i32!(&inner[..amount_end]);

    // Parse effective heal after ~, default to heal_amount if not present
    let effective_pos = memchr(b'~', inner_bytes);
    let heal_effective = effective_pos
        .map(|pos| {
            let start = pos + 1;
            let end = inner[start..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|e| start + e)
                .unwrap_or(inner.len());
            parse_i32!(&inner[start..end])
        })
        .unwrap_or(heal_amount);

    Some(Details {
        heal_amount,
        heal_effective,
        is_crit,
        threat,
        ..Default::default()
    })
}
fn parse_charges(segment: &str) -> Option<Details> {
    let bytes = segment.as_bytes();

    let paren_start = memchr(b'(', bytes)?;
    let paren_end = memchr(b')', bytes)?;
    let brace_start = memchr(b'{', bytes)?;
    let brace_end = memchr(b'}', bytes)?;

    // Parse count: number before "charges"
    let inner = &segment[paren_start + 1..paren_end];
    let count_end = inner
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(inner.len());
    let charges = parse_i32!(&inner[..count_end]);

    // Parse ability ID
    let ability_id = parse_i64!(&segment[brace_start + 1..brace_end]);

    Some(Details {
        charges,
        ability_id,
        ..Default::default()
    })
}
