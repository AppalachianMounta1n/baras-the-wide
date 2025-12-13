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
    let event = CombatEvent {
        line_number,
        timestamp: ts,
        source_entity,
        target_entity,
        action,
        effect,
        ..Default::default()
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

    let type_id = parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]);
    let class_name = segment[end_brackets[0] + 2..brackets[1] - 1]
        .trim()
        .to_string();
    let class_id = parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]);
    let discipline_name = segment[slash_pos + 1..brackets[2] - 1].trim().to_string();
    let discipline_id = parse_i64!(&segment[brackets[2] + 1..end_brackets[2]]);

    Some(Effect::DisciplineChanged {
        type_id,
        class_name,
        class_id,
        discipline_name,
        discipline_id,
    })
}

fn parse_area_entered(segment: &str) -> Option<Effect> {
    let bytes = segment.as_bytes();
    let brackets: Vec<usize> = memchr_iter(b'{', bytes).collect();
    let end_brackets: Vec<usize> = memchr_iter(b'}', bytes).collect();

    if brackets.len() < 2 || end_brackets.len() < 2 {
        return Some(Effect::Empty);
    }

    let type_id = parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]);
    let area_name = segment[end_brackets[0] + 2..brackets[1] - 1]
        .trim()
        .to_string();
    let area_id = parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]);

    // Difficulty is optional - check if there's a third bracket pair
    let (difficulty, difficulty_id) = if brackets.len() >= 3 && end_brackets.len() >= 3 {
        let diff_name = segment[end_brackets[1] + 1..brackets[2] - 1]
            .trim()
            .to_string();
        let diff_id = parse_i64!(&segment[brackets[2] + 1..end_brackets[2]]);
        (Some(diff_name), Some(diff_id))
    } else {
        (None, None)
    };

    Some(Effect::AreaEntered {
        type_id,
        area_name,
        area_id,
        difficulty,
        difficulty_id,
    })
}
fn parse_standard_effect(segment: &str) -> Option<Effect> {
    let bytes = segment.as_bytes();
    let brackets: Vec<usize> = memchr_iter(b'{', bytes).collect();
    let end_brackets: Vec<usize> = memchr_iter(b'}', bytes).collect();

    if brackets.len() < 2 {
        return Some(Effect::Empty);
    }

    Some(Effect::Standard {
        type_name: segment[..brackets[0]].trim().to_string(),
        type_id: parse_i64!(&segment[brackets[0] + 1..end_brackets[0]]),
        name: segment[end_brackets[0] + 2..brackets[1] - 1]
            .trim()
            .to_string(),
        id: parse_i64!(&segment[brackets[1] + 1..end_brackets[1]]),
    })
}
