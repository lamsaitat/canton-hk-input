#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacePolicy {
    /// Windows built-in Pinyin IME behavior: Space commits the highlighted candidate.
    CommitSelected,
    /// Classic gcin behavior: Space advances to the next page of 10 candidates.
    PageNext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidKeyPolicy {
    /// Keep current pre-edit buffer intact and ignore invalid extension key.
    Reject,
    /// Auto-commit candidate #1 and start a new syllable with the incoming key.
    AutoCommitAndStartNew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunctuationMode {
    /// Open candidate popup for punctuation keys (`,`, `.`, `"`, `[`, etc.).
    CandidateList,
    /// Immediately commit the primary Chinese punctuation (e.g. `,` -> `，`).
    DirectCommitPrimary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineConfig {
    pub page_size: usize,
    pub space_policy: SpacePolicy,
    pub invalid_key_policy: InvalidKeyPolicy,
    pub punctuation_mode: PunctuationMode,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            page_size: 10,
            space_policy: SpacePolicy::CommitSelected,
            invalid_key_policy: InvalidKeyPolicy::Reject,
            punctuation_mode: PunctuationMode::CandidateList,
        }
    }
}
