use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    generate_off_gcd_set(&out_dir);
    generate_attack_types_map(&out_dir);

    println!("cargo:rerun-if-changed=data/off_gcd.json");
    println!("cargo:rerun-if-changed=data/attack_types.csv");
}

fn generate_off_gcd_set(out_dir: &str) {
    let json = fs::read_to_string("data/off_gcd.json").expect("failed to read off_gcd.json");

    // Simple JSON object parse: extract all numeric keys
    let mut ids: Vec<i64> = json
        .split('"')
        .enumerate()
        .filter_map(|(i, s)| if i % 2 == 1 { s.parse::<i64>().ok() } else { None })
        .collect();
    ids.sort_unstable();
    ids.dedup();

    let path = Path::new(out_dir).join("off_gcd_abilities.rs");
    let mut file = BufWriter::new(fs::File::create(&path).unwrap());

    let mut builder = phf_codegen::Set::new();
    for id in &ids {
        builder.entry(*id);
    }

    writeln!(file, "pub static OFF_GCD_ABILITIES: phf::Set<i64> = {};", builder.build()).unwrap();
}

fn generate_attack_types_map(out_dir: &str) {
    let csv = fs::read_to_string("data/attack_types.csv").expect("failed to read attack_types.csv");

    // BTreeMap for deterministic output (sorted by key)
    let mut entries = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 2 {
            continue;
        }
        let id: i64 = match fields[0].trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let attack_type = fields[1].trim();
        if matches!(attack_type, "" | "None" | "God") {
            continue;
        }
        entries.entry(id).or_insert(attack_type.to_string());
    }

    let path = Path::new(out_dir).join("attack_types.rs");
    let mut file = BufWriter::new(fs::File::create(&path).unwrap());

    let mut builder = phf_codegen::Map::new();
    let quoted: Vec<_> = entries.iter().map(|(id, at)| (*id, format!("\"{}\"", at))).collect();
    for (id, at) in &quoted {
        builder.entry(*id, at);
    }

    writeln!(file, "pub static ATTACK_TYPES: phf::Map<i64, &'static str> = {};", builder.build())
        .unwrap();
}
