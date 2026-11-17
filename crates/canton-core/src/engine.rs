use crate::config::{EngineConfig, InvalidKeyPolicy, PunctuationMode, SpacePolicy};
use crate::lexicon::Lexicon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Backspace,
    Space,
    Enter,
    Escape,
    PageDown,
    PageUp,
    Down,
    Up,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateItem {
    pub text: String,
    pub suffix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePage {
    pub preedit: String,
    pub items: Vec<CandidateItem>,
    pub selected_in_page: usize,
    pub page_index: usize,
    pub total_pages: usize,
    pub total_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineAction {
    /// Key was not consumed; pass through to the host application.
    PassThrough,
    /// Key was consumed; update pre-edit and candidate UI.
    UpdateUi(CandidatePage),
    /// Commit `text` to the host application and clear composition/candidate UI.
    Commit(String),
    /// Commit `committed` first, then immediately open a new composition (`next_page`).
    CommitAndUpdate {
        committed: String,
        next_page: CandidatePage,
    },
    /// Cancel composition and hide candidate UI without committing text.
    Cancel,
    /// Key rejected (e.g., invalid syllable extension); keep current state intact.
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposeKind {
    Idle,
    Alpha,
    Punctuation,
}

pub struct Engine {
    config: EngineConfig,
    chinese_mode: bool,
    full_width_mode: bool,
    kind: ComposeKind,
    buffer: String,
    page_index: usize,
    selected_in_page: usize,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            chinese_mode: true,
            full_width_mode: false,
            kind: ComposeKind::Idle,
            buffer: String::new(),
            page_index: 0,
            selected_in_page: 0,
        }
    }

    pub fn config(&self) -> EngineConfig {
        self.config
    }

    pub fn set_config(&mut self, config: EngineConfig) {
        self.config = config;
    }

    pub fn is_chinese_mode(&self) -> bool {
        self.chinese_mode
    }

    pub fn set_chinese_mode(&mut self, enabled: bool) {
        if self.chinese_mode != enabled {
            self.chinese_mode = enabled;
            self.reset();
        }
    }

    pub fn toggle_chinese_mode(&mut self) -> bool {
        self.set_chinese_mode(!self.chinese_mode);
        self.chinese_mode
    }

    pub fn is_full_width(&self) -> bool {
        self.full_width_mode
    }

    pub fn set_full_width(&mut self, enabled: bool) {
        self.full_width_mode = enabled;
    }

    pub fn is_composing(&self) -> bool {
        self.kind != ComposeKind::Idle
    }

    pub fn preedit(&self) -> &str {
        &self.buffer
    }

    pub fn reset(&mut self) {
        self.kind = ComposeKind::Idle;
        self.buffer.clear();
        self.page_index = 0;
        self.selected_in_page = 0;
    }

    pub fn current_page(&self) -> Option<CandidatePage> {
        if self.kind == ComposeKind::Idle {
            return None;
        }
        let all = self.all_candidates();
        let total = all.len();
        let page_size = self.config.page_size.max(1);
        let total_pages = if total == 0 {
            1
        } else {
            (total + page_size - 1) / page_size
        };
        let page_index = self.page_index.min(total_pages - 1);
        let start = page_index * page_size;
        let end = (start + page_size).min(total);
        let items: Vec<CandidateItem> = all[start..end]
            .iter()
            .map(|(text, suffix)| CandidateItem {
                text: text.clone(),
                suffix: suffix.clone(),
            })
            .collect();
        let selected_in_page = if items.is_empty() {
            0
        } else {
            self.selected_in_page.min(items.len() - 1)
        };

        Some(CandidatePage {
            preedit: self.buffer.clone(),
            items,
            selected_in_page,
            page_index,
            total_pages,
            total_candidates: total,
        })
    }

    fn all_candidates(&self) -> Vec<(String, String)> {
        match self.kind {
            ComposeKind::Idle => Vec::new(),
            ComposeKind::Alpha => Lexicon::collect_candidates_with_predictions(&self.buffer, 5),
            ComposeKind::Punctuation => Lexicon::get_all_candidates(&self.buffer)
                .into_iter()
                .map(|s| (s, String::new()))
                .collect(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EngineAction {
        if !self.chinese_mode {
            return EngineAction::PassThrough;
        }

        match self.kind {
            ComposeKind::Idle => self.handle_idle(key),
            ComposeKind::Alpha => self.handle_alpha_composing(key),
            ComposeKind::Punctuation => self.handle_punct_composing(key),
        }
    }

    fn handle_idle(&mut self, key: KeyEvent) -> EngineAction {
        match key {
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                let s = ch.to_string();
                if Lexicon::is_alpha_prefix(&s) {
                    self.kind = ComposeKind::Alpha;
                    self.buffer = s;
                    self.page_index = 0;
                    self.selected_in_page = 0;
                    EngineAction::UpdateUi(self.current_page().unwrap())
                } else {
                    EngineAction::Reject
                }
            }
            KeyEvent::Char(ch) if !ch.is_ascii_digit() && !ch.is_ascii_uppercase() => {
                let s = ch.to_string();
                let count = Lexicon::lookup_count(&s);
                if count > 0 {
                    match self.config.punctuation_mode {
                        PunctuationMode::DirectCommitPrimary => {
                            let primary = Lexicon::get_candidate_string(&s, 0).unwrap();
                            EngineAction::Commit(primary)
                        }
                        PunctuationMode::CandidateList => {
                            if count == 1 {
                                let only = Lexicon::get_candidate_string(&s, 0).unwrap();
                                EngineAction::Commit(only)
                            } else {
                                self.kind = ComposeKind::Punctuation;
                                self.buffer = s;
                                self.page_index = 0;
                                self.selected_in_page = 0;
                                EngineAction::UpdateUi(self.current_page().unwrap())
                            }
                        }
                    }
                } else {
                    EngineAction::PassThrough
                }
            }
            _ => EngineAction::PassThrough,
        }
    }

    fn handle_alpha_composing(&mut self, key: KeyEvent) -> EngineAction {
        match key {
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                let mut next = self.buffer.clone();
                next.push(ch);
                if Lexicon::is_alpha_prefix(&next) {
                    self.buffer = next;
                    self.page_index = 0;
                    self.selected_in_page = 0;
                    EngineAction::UpdateUi(self.current_page().unwrap())
                } else {
                    match self.config.invalid_key_policy {
                        InvalidKeyPolicy::Reject => EngineAction::Reject,
                        InvalidKeyPolicy::AutoCommitAndStartNew => {
                            if let Some(first) = Lexicon::get_candidate_string(&self.buffer, 0) {
                                let single = ch.to_string();
                                if Lexicon::is_alpha_prefix(&single) {
                                    self.buffer = single;
                                    self.page_index = 0;
                                    self.selected_in_page = 0;
                                    let next_page = self.current_page().unwrap();
                                    EngineAction::CommitAndUpdate {
                                        committed: first,
                                        next_page,
                                    }
                                } else {
                                    self.reset();
                                    EngineAction::Commit(first)
                                }
                            } else {
                                EngineAction::Reject
                            }
                        }
                    }
                }
            }
            KeyEvent::Char(ch) if ch.is_ascii_digit() => {
                let slot = selkey_to_slot(ch);
                self.commit_slot_in_page(slot)
            }
            KeyEvent::Char('=') => self.next_page(),
            KeyEvent::Char('-') => self.prev_page(),
            KeyEvent::Backspace => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    self.reset();
                    EngineAction::Cancel
                } else {
                    self.page_index = 0;
                    self.selected_in_page = 0;
                    EngineAction::UpdateUi(self.current_page().unwrap())
                }
            }
            KeyEvent::Escape => {
                self.reset();
                EngineAction::Cancel
            }
            KeyEvent::Enter => {
                let raw = self.buffer.clone();
                self.reset();
                EngineAction::Commit(raw)
            }
            KeyEvent::Space => match self.config.space_policy {
                SpacePolicy::CommitSelected => self.commit_slot_in_page(self.selected_in_page),
                SpacePolicy::PageNext => self.next_page(),
            },
            KeyEvent::PageDown | KeyEvent::Right => self.next_page(),
            KeyEvent::PageUp | KeyEvent::Left => self.prev_page(),
            KeyEvent::Down => self.move_selection(1),
            KeyEvent::Up => self.move_selection(-1),
            _ => EngineAction::Reject,
        }
    }

    fn handle_punct_composing(&mut self, key: KeyEvent) -> EngineAction {
        match key {
            KeyEvent::Char(ch) if ch.is_ascii_digit() => {
                let slot = selkey_to_slot(ch);
                self.commit_slot_in_page(slot)
            }
            KeyEvent::Space => self.commit_slot_in_page(self.selected_in_page),
            KeyEvent::Enter => {
                let raw = self.buffer.clone();
                self.reset();
                EngineAction::Commit(raw)
            }
            KeyEvent::Escape | KeyEvent::Backspace => {
                self.reset();
                EngineAction::Cancel
            }
            KeyEvent::Down | KeyEvent::Right | KeyEvent::PageDown => self.move_selection(1),
            KeyEvent::Up | KeyEvent::Left | KeyEvent::PageUp => self.move_selection(-1),
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                // Auto-commit primary punctuation and start typing the alphabetic syllable
                let primary = Lexicon::get_candidate_string(&self.buffer, 0)
                    .unwrap_or_else(|| self.buffer.clone());
                let s = ch.to_string();
                if Lexicon::is_alpha_prefix(&s) {
                    self.kind = ComposeKind::Alpha;
                    self.buffer = s;
                    self.page_index = 0;
                    self.selected_in_page = 0;
                    EngineAction::CommitAndUpdate {
                        committed: primary,
                        next_page: self.current_page().unwrap(),
                    }
                } else {
                    self.reset();
                    EngineAction::Commit(primary)
                }
            }
            _ => EngineAction::Reject,
        }
    }

    fn commit_slot_in_page(&mut self, slot: usize) -> EngineAction {
        let global_idx = self.page_index * self.config.page_size + slot;
        let all = self.all_candidates();
        if let Some((cand, _suffix)) = all.get(global_idx) {
            let committed = cand.clone();
            self.reset();
            EngineAction::Commit(committed)
        } else {
            EngineAction::Reject
        }
    }

    fn next_page(&mut self) -> EngineAction {
        let total = self.all_candidates().len();
        if total == 0 {
            return EngineAction::Reject;
        }
        let page_size = self.config.page_size.max(1);
        let total_pages = (total + page_size - 1) / page_size;
        self.page_index = (self.page_index + 1) % total_pages;
        self.selected_in_page = 0;
        EngineAction::UpdateUi(self.current_page().unwrap())
    }

    fn prev_page(&mut self) -> EngineAction {
        let total = self.all_candidates().len();
        if total == 0 {
            return EngineAction::Reject;
        }
        let page_size = self.config.page_size.max(1);
        let total_pages = (total + page_size - 1) / page_size;
        self.page_index = (self.page_index + total_pages - 1) % total_pages;
        self.selected_in_page = 0;
        EngineAction::UpdateUi(self.current_page().unwrap())
    }

    fn move_selection(&mut self, delta: isize) -> EngineAction {
        let Some(page) = self.current_page() else {
            return EngineAction::Reject;
        };
        if page.items.is_empty() {
            return EngineAction::Reject;
        }
        let len = page.items.len() as isize;
        let next = (page.selected_in_page as isize + delta).rem_euclid(len) as usize;
        self.selected_in_page = next;
        EngineAction::UpdateUi(self.current_page().unwrap())
    }
}

fn selkey_to_slot(ch: char) -> usize {
    match ch {
        '1'..='9' => (ch as usize) - ('1' as usize),
        '0' => 9,
        _ => usize::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cantonese_typing_and_space_commit() {
        let mut engine = Engine::default();
        assert!(matches!(
            engine.handle_key(KeyEvent::Char('n')),
            EngineAction::UpdateUi(_)
        ));
        assert!(matches!(
            engine.handle_key(KeyEvent::Char('g')),
            EngineAction::UpdateUi(_)
        ));
        assert!(matches!(
            engine.handle_key(KeyEvent::Char('o')),
            EngineAction::UpdateUi(_)
        ));
        assert_eq!(engine.preedit(), "ngo");

        // Space commits candidate #1 (`傲` for `ngo`)
        match engine.handle_key(KeyEvent::Space) {
            EngineAction::Commit(s) => assert_eq!(s, "傲"),
            other => panic!("Expected Commit, got {:?}", other),
        }
        assert!(!engine.is_composing());
    }

    #[test]
    fn test_invalid_key_is_rejected() {
        let mut engine = Engine::default();
        engine.handle_key(KeyEvent::Char('n'));
        engine.handle_key(KeyEvent::Char('g'));
        engine.handle_key(KeyEvent::Char('o'));
        // `ngoz` is not a valid prefix; default policy is Reject
        assert_eq!(engine.handle_key(KeyEvent::Char('z')), EngineAction::Reject);
        assert_eq!(engine.preedit(), "ngo");
    }

    #[test]
    fn test_paging_and_number_selection() {
        let mut engine = Engine::default();
        engine.handle_key(KeyEvent::Char('j'));
        engine.handle_key(KeyEvent::Char('i'));
        // Page 2 via PageDown
        let page2 = match engine.handle_key(KeyEvent::PageDown) {
            EngineAction::UpdateUi(p) => p,
            other => panic!("Expected UpdateUi, got {:?}", other),
        };
        assert_eq!(page2.page_index, 1);
        assert!(page2.total_pages >= 16);
        assert_eq!(page2.items.len(), 10);

        // Select slot '1' on page 2 -> global index 10
        let expected = Lexicon::get_candidate_string("ji", 10).unwrap();
        match engine.handle_key(KeyEvent::Char('1')) {
            EngineAction::Commit(c) => assert_eq!(c, expected),
            other => panic!("Expected Commit, got {:?}", other),
        }
    }

    #[test]
    fn test_predictive_prefix_candidates() {
        let mut engine = Engine::default();
        engine.handle_key(KeyEvent::Char('l'));
        let page = match engine.handle_key(KeyEvent::Char('o')) {
            EngineAction::UpdateUi(p) => p,
            other => panic!("Expected UpdateUi, got {:?}", other),
        };
        // `lo` has predictive completions like `k 樂` (from `lok`)
        let all = Lexicon::collect_candidates_with_predictions("lo", 5);
        assert!(all.iter().any(|(text, suffix)| text == "樂" && suffix == "k"));
        assert!(!page.items.is_empty());
    }

    #[test]
    fn test_backspace_and_escape() {
        let mut engine = Engine::default();
        engine.handle_key(KeyEvent::Char('j'));
        engine.handle_key(KeyEvent::Char('i'));
        assert!(matches!(
            engine.handle_key(KeyEvent::Backspace),
            EngineAction::UpdateUi(p) if p.preedit == "j"
        ));
        assert_eq!(engine.handle_key(KeyEvent::Backspace), EngineAction::Cancel);
        assert!(!engine.is_composing());
    }

    #[test]
    fn test_idle_digits_pass_through() {
        let mut engine = Engine::default();
        assert_eq!(
            engine.handle_key(KeyEvent::Char('5')),
            EngineAction::PassThrough
        );
    }

    #[test]
    fn test_punctuation_composing_and_auto_commit() {
        let mut engine = Engine::default();
        // `,` has 2 candidates (`，`, `、`)
        let page = match engine.handle_key(KeyEvent::Char(',')) {
            EngineAction::UpdateUi(p) => p,
            other => panic!("Expected UpdateUi, got {:?}", other),
        };
        let texts: Vec<&str> = page.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, vec!["，", "、"]);
        // Typing `h` after `,` auto-commits `，` and starts `h`
        match engine.handle_key(KeyEvent::Char('h')) {
            EngineAction::CommitAndUpdate {
                committed,
                next_page,
            } => {
                assert_eq!(committed, "，");
                assert_eq!(next_page.preedit, "h");
            }
            other => panic!("Expected CommitAndUpdate, got {:?}", other),
        }
    }
}
