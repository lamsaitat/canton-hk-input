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
