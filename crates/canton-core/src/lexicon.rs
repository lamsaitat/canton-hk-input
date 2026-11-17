mod generated {
    include!(concat!(env!("OUT_DIR"), "/lexicon_data.rs"));
}

/// Zero-copy, static lexicon compiled from `cantonhk.cin`.
pub struct Lexicon;

impl Lexicon {
    /// Returns `true` if `prefix` is a valid lowercase alphabetic key or proper prefix of one.
    pub fn is_alpha_prefix(prefix: &str) -> bool {
        generated::ALPHA_PREFIXES.binary_search(&prefix).is_ok()
    }

    /// Looks up exact candidate count for `key`.
    pub fn lookup_count(key: &str) -> usize {
        match generated::KEY_RECORDS.binary_search_by_key(&key, |r| r.0) {
            Ok(idx) => generated::KEY_RECORDS[idx].2 as usize,
            Err(_) => 0,
        }
    }

    /// Returns a candidate at `index` as a `String`.
    pub fn get_candidate_string(key: &str, index: usize) -> Option<String> {
        let u16_slice = Self::get_candidate_utf16(key, index)?;
        Some(String::from_utf16_lossy(u16_slice))
    }

    /// Returns a candidate at `index` as a zero-copy `&'static [u16]` slice (UTF-16LE for Win32/TSF).
    pub fn get_candidate_utf16(key: &str, index: usize) -> Option<&'static [u16]> {
        let rec_idx = generated::KEY_RECORDS
            .binary_search_by_key(&key, |r| r.0)
            .ok()?;
        let (_, start, count) = generated::KEY_RECORDS[rec_idx];
        if index >= count as usize {
            return None;
        }
        let (off, len) = generated::CAND_SLICES[start as usize + index];
        let start_u16 = off as usize;
        let end_u16 = start_u16 + len as usize;
        Some(&generated::UTF16_POOL[start_u16..end_u16])
    }

    /// Returns all candidates for `key` as `Vec<String>`.
    pub fn get_all_candidates(key: &str) -> Vec<String> {
        let count = Self::lookup_count(key);
        (0..count)
            .filter_map(|i| Self::get_candidate_string(key, i))
            .collect()
    }

    /// Returns `(text, suffix)` pairs for `prefix`:
    /// exact matches first (`suffix == ""`), followed by top candidates from longer keys
    /// starting with `prefix` (ordered by shortest suffix length first).
    pub fn collect_candidates_with_predictions(
        prefix: &str,
        max_per_extension: usize,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let exact_count = Self::lookup_count(prefix);
        for i in 0..exact_count {
            if let Some(s) = Self::get_candidate_string(prefix, i) {
                results.push((s, String::new()));
            }
        }

        // Only expand alphabetic prefixes for predictive completions
        if prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_lowercase()) {
            return results;
        }

        let start_idx = generated::KEY_RECORDS.partition_point(|r| r.0 < prefix);
        let mut extensions: Vec<(&'static str, u16)> = Vec::new();
        for &(k, _start, count) in &generated::KEY_RECORDS[start_idx..] {
            if !k.starts_with(prefix) {
                break;
            }
            if k.len() > prefix.len() && k.bytes().all(|b| b.is_ascii_lowercase()) {
                extensions.push((k, count));
            }
        }

        // Shortest suffix first (1-char completions before 2-char, etc.), then key order
        extensions.sort_by_key(|&(k, _)| (k.len(), k));

        for (ext_key, count) in extensions {
            let suffix = ext_key[prefix.len()..].to_string();
            let take_n = (count as usize).min(max_per_extension);
            for i in 0..take_n {
                if let Some(s) = Self::get_candidate_string(ext_key, i) {
                    results.push((s, suffix.clone()));
                }
            }
        }

        results
    }

    /// Total number of unique keys in the compiled lexicon.
    pub fn key_count() -> usize {
        generated::KEY_RECORDS.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexicon_counts_and_sanitization() {
        assert_eq!(Lexicon::key_count(), 783);
        // `ji` has 160 candidates, starting with 之
        assert_eq!(Lexicon::lookup_count("ji"), 160);
        assert_eq!(Lexicon::get_candidate_string("ji", 0).as_deref(), Some("之"));
        // `yung` starts with 容, 用, 佣
        assert_eq!(Lexicon::get_candidate_string("yung", 0).as_deref(), Some("容"));
        assert_eq!(Lexicon::get_candidate_string("yung", 1).as_deref(), Some("用"));
        // Line 50-53 `,` deduplicated after stripping U+F87D: should only have `，` and `、`
        assert_eq!(Lexicon::lookup_count(","), 2);
        assert_eq!(Lexicon::get_candidate_string(",", 0).as_deref(), Some("，"));
        assert_eq!(Lexicon::get_candidate_string(",", 1).as_deref(), Some("、"));
        // Prefix checks
        assert!(Lexicon::is_alpha_prefix("j"));
        assert!(Lexicon::is_alpha_prefix("cheun"));
        assert!(!Lexicon::is_alpha_prefix("jz"));
    }
}
