use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cin_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("cantonhk.cin");

    println!("cargo:rerun-if-changed={}", cin_path.display());

    let file = File::open(&cin_path).expect("Failed to open cantonhk.cin");
    let reader = BufReader::new(file);

    let mut in_chardef = false;
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen_pairs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut alpha_prefixes: BTreeSet<String> = BTreeSet::new();

    for line in reader.lines() {
        let line = line.expect("Failed to read line from cantonhk.cin");
        let trimmed = line.trim();
        if trimmed == "%chardef begin" {
            in_chardef = true;
            continue;
        }
        if trimmed == "%chardef end" {
            in_chardef = false;
            continue;
        }
        if !in_chardef || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let key = match parts.next() {
            Some(k) if !k.is_empty() => k,
            _ => continue,
        };
        let raw_val = match parts.next() {
            Some(v) => v.trim(),
            None => continue,
        };

        // Strip Apple corporate PUA variation selector U+F87D
        let cleaned: String = raw_val.chars().filter(|&c| c != '\u{F87D}').collect();
        if cleaned.is_empty() {
            continue;
        }

        // Skip entries that consist solely of Private Use Area codepoints (U+E000..=U+F8FF)
        if cleaned.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) {
            continue;
        }

        let key_str = key.to_string();
        if !seen_pairs.insert((key_str.clone(), cleaned.clone())) {
            continue;
        }

        if key_str.bytes().all(|b| b.is_ascii_lowercase()) {
            for len in 1..=key_str.len() {
                alpha_prefixes.insert(key_str[..len].to_string());
            }
        }

        entries.entry(key_str).or_default().push(cleaned);
    }

    // Build flat UTF-16 pool + candidate slice table + key table
    let mut utf16_pool: Vec<u16> = Vec::new();
    let mut cand_slices: Vec<(u32, u16)> = Vec::new(); // (utf16_offset, utf16_len)
    let mut key_records: Vec<(String, u32, u16, bool)> = Vec::new();

    let alpha_prefix_set: HashSet<&String> = alpha_prefixes.iter().collect();

    for (key, cands) in &entries {
        let cand_start = cand_slices.len() as u32;
        let cand_count = cands.len() as u16;
        for c in cands {
            let offset = utf16_pool.len() as u32;
            let encoded: Vec<u16> = c.encode_utf16().collect();
            let len = encoded.len() as u16;
            utf16_pool.extend_from_slice(&encoded);
            cand_slices.push((offset, len));
        }
        let is_prefix = alpha_prefix_set.contains(key);
        key_records.push((key.clone(), cand_start, cand_count, is_prefix));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("lexicon_data.rs");
    let mut out = BufWriter::new(File::create(dest).unwrap());

    writeln!(out, "// Auto-generated from cantonhk.cin by build.rs").unwrap();
    writeln!(out, "pub const UTF16_POOL: &[u16] = &[").unwrap();
    for chunk in utf16_pool.chunks(24) {
        write!(out, "    ").unwrap();
        for &u in chunk {
            write!(out, "0x{:04X}, ", u).unwrap();
        }
        writeln!(out).unwrap();
    }
    writeln!(out, "];").unwrap();

    writeln!(out, "pub const CAND_SLICES: &[(u32, u16)] = &[").unwrap();
    for &(off, len) in &cand_slices {
        writeln!(out, "    ({}, {}),", off, len).unwrap();
    }
    writeln!(out, "];").unwrap();

    writeln!(
        out,
        "pub const KEY_RECORDS: &[(&str, u32, u16)] = &["
    )
    .unwrap();
    for (k, start, count, _) in &key_records {
        writeln!(out, "    ({:?}, {}, {}),", k, start, count).unwrap();
    }
    writeln!(out, "];").unwrap();

    writeln!(out, "pub const ALPHA_PREFIXES: &[&str] = &[").unwrap();
    for p in &alpha_prefixes {
        writeln!(out, "    {:?},", p).unwrap();
    }
    writeln!(out, "];").unwrap();
}
