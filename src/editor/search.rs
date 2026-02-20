//! Search and replace functionality for the hex editor

use std::collections::HashSet;

/// Search mode - either hex pattern or ASCII string
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SearchMode {
    /// Hex pattern search (e.g., "FF D8 FF" or "FF ?? FF" with wildcards)
    #[default]
    Hex,
    /// ASCII string search
    Ascii,
}

/// Parsed search pattern element
#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    /// Exact byte match
    Byte(u8),
    /// Wildcard (matches any byte)
    Wildcard,
}

/// A message from a search/replace operation — either an error or informational
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMessage {
    /// A real error (parse failure, protection violation, etc.)
    Error(String),
    /// An informational message (e.g. "Replaced 1 of 2, 1 skipped")
    Info(String),
}

/// Search state and results
#[derive(Default)]
pub struct SearchState {
    /// The current search query string
    pub query: String,
    /// Search mode (hex or ASCII)
    pub mode: SearchMode,
    /// Case-sensitive search (only applies to ASCII mode)
    pub case_sensitive: bool,
    /// All match positions (byte offsets)
    pub matches: Vec<usize>,
    /// Pre-computed set of all offsets within any match (for O(1) highlight lookup)
    highlighted_offsets: HashSet<usize>,
    /// Current match index (for Next/Previous navigation)
    pub current_match: Option<usize>,
    /// Whether the search dialog is visible
    pub dialog_open: bool,
    /// Replace text
    pub replace_with: String,
    /// Last search/replace message (error or informational)
    pub message: Option<SearchMessage>,
    /// Cached pattern length (computed in execute_search/clear_results)
    cached_pattern_len: usize,
    /// Whether the dialog was just opened (for auto-focus on first frame)
    pub just_opened: bool,
    /// Query that produced the current matches (for stale detection)
    last_searched_query: String,
    /// Mode that produced the current matches
    last_searched_mode: SearchMode,
    /// Case sensitivity that produced the current matches
    last_searched_case_sensitive: bool,
    /// Editor generation when search was last executed
    searched_at_generation: u64,
}

impl SearchState {
    /// Open the search dialog
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
        self.just_opened = true;
    }

    /// Close the search dialog and clear results/highlights
    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
        self.clear_results();
    }

    /// Check if an offset is within a match (considering pattern length)
    /// Uses pre-computed HashSet for O(1) lookup
    pub fn is_within_match(&self, offset: usize) -> bool {
        self.highlighted_offsets.contains(&offset)
    }

    /// Rebuild the highlighted offsets set from current matches
    fn rebuild_highlighted_offsets(&mut self, pattern_len: usize) {
        self.highlighted_offsets.clear();
        for &match_start in &self.matches {
            for offset in match_start..(match_start + pattern_len) {
                self.highlighted_offsets.insert(offset);
            }
        }
    }

    /// Get the cached pattern length (set by execute_search/clear_results)
    pub fn pattern_length(&self) -> usize {
        self.cached_pattern_len
    }

    /// Check if the query/mode/case settings have changed since the last search
    pub fn query_changed_since_search(&self) -> bool {
        self.query != self.last_searched_query
            || self.mode != self.last_searched_mode
            || self.case_sensitive != self.last_searched_case_sensitive
    }

    /// Check if match results may be stale due to buffer edits since the search
    pub fn matches_may_be_stale(&self, current_generation: u64) -> bool {
        !self.matches.is_empty() && self.searched_at_generation != current_generation
    }

    /// Record the editor generation at search time
    pub fn set_searched_generation(&mut self, generation: u64) {
        self.searched_at_generation = generation;
    }

    /// Move to the next match
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        match self.current_match {
            None => self.current_match = Some(0),
            Some(i) => {
                self.current_match = Some((i + 1) % self.matches.len());
            }
        }
    }

    /// Move to the previous match
    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        match self.current_match {
            None => self.current_match = Some(self.matches.len() - 1),
            Some(0) => self.current_match = Some(self.matches.len() - 1),
            Some(i) => self.current_match = Some(i - 1),
        }
    }

    /// Get the offset of the current match
    pub fn current_match_offset(&self) -> Option<usize> {
        self.current_match
            .and_then(|i| self.matches.get(i).copied())
    }

    /// Clear search results
    pub fn clear_results(&mut self) {
        self.matches.clear();
        self.highlighted_offsets.clear();
        self.current_match = None;
        self.message = None;
        self.cached_pattern_len = 0;
    }
}

/// Parse a hex pattern string into pattern elements
/// Supports formats like "FF D8 FF" or "FFD8FF" or "FF ?? FF" (with wildcards)
pub fn parse_hex_pattern(pattern: &str) -> Result<Vec<PatternElement>, String> {
    let mut elements = Vec::new();
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }

        if c == '?' {
            // Wildcard - consume second ? if present
            if chars.peek() == Some(&'?') {
                chars.next();
            }
            elements.push(PatternElement::Wildcard);
        } else if let Some(high) = c.to_digit(16) {
            // First hex digit - need the second one
            let low_char = chars
                .next()
                .ok_or_else(|| "Incomplete hex byte: expected second digit".to_string())?;

            // Skip whitespace between digits (e.g., "F F" should work)
            let low_char = if low_char.is_whitespace() {
                chars.next().ok_or_else(|| {
                    "Incomplete hex byte: expected second digit after space".to_string()
                })?
            } else {
                low_char
            };

            let low = low_char
                .to_digit(16)
                .ok_or_else(|| format!("Invalid hex digit: '{}'", low_char))?;

            elements.push(PatternElement::Byte(((high << 4) | low) as u8));
        } else {
            return Err(format!("Invalid character in hex pattern: '{}'", c));
        }
    }

    if elements.is_empty() {
        return Err("Empty pattern".to_string());
    }

    Ok(elements)
}

/// Parse replace pattern (hex format only)
pub fn parse_hex_replace(pattern: &str) -> Result<Vec<u8>, String> {
    let elements = parse_hex_pattern(pattern)?;

    // Replace pattern cannot contain wildcards
    for elem in &elements {
        if matches!(elem, PatternElement::Wildcard) {
            return Err("Replace pattern cannot contain wildcards".to_string());
        }
    }

    Ok(elements
        .iter()
        .filter_map(|e| match e {
            PatternElement::Byte(b) => Some(*b),
            PatternElement::Wildcard => None,
        })
        .collect())
}

/// Search for a hex pattern in the buffer
pub fn search_hex(data: &[u8], pattern: &[PatternElement]) -> Vec<usize> {
    if pattern.is_empty() || data.len() < pattern.len() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    for i in 0..=(data.len() - pattern.len()) {
        let mut found = true;
        for (j, elem) in pattern.iter().enumerate() {
            match elem {
                PatternElement::Byte(b) => {
                    if data[i + j] != *b {
                        found = false;
                        break;
                    }
                }
                PatternElement::Wildcard => {
                    // Matches any byte
                }
            }
        }
        if found {
            matches.push(i);
        }
    }

    matches
}

/// Search for an ASCII string in the buffer
pub fn search_ascii(data: &[u8], query: &str, case_sensitive: bool) -> Vec<usize> {
    if query.is_empty() || data.len() < query.len() {
        return Vec::new();
    }

    let lowered;
    let query_bytes: &[u8] = if case_sensitive {
        query.as_bytes()
    } else {
        lowered = query.to_lowercase();
        lowered.as_bytes()
    };

    let mut matches = Vec::new();

    for i in 0..=(data.len() - query_bytes.len()) {
        let mut found = true;
        for (j, &qb) in query_bytes.iter().enumerate() {
            let db = if case_sensitive {
                data[i + j]
            } else {
                data[i + j].to_ascii_lowercase()
            };

            if db != qb {
                found = false;
                break;
            }
        }
        if found {
            matches.push(i);
        }
    }

    matches
}

/// Execute search based on current state
pub fn execute_search(state: &mut SearchState, data: &[u8]) {
    state.clear_results();

    if state.query.is_empty() {
        return;
    }

    let pattern_len = match state.mode {
        SearchMode::Hex => match parse_hex_pattern(&state.query) {
            Ok(pattern) => {
                let len = pattern.len();
                state.matches = search_hex(data, &pattern);
                len
            }
            Err(e) => {
                state.message = Some(SearchMessage::Error(e));
                return;
            }
        },
        SearchMode::Ascii => {
            let len = state.query.len();
            state.matches = search_ascii(data, &state.query, state.case_sensitive);
            len
        }
    };

    // Cache the pattern length and build highlighted offsets
    state.cached_pattern_len = pattern_len;
    state.rebuild_highlighted_offsets(pattern_len);

    // Record what produced these matches (for stale detection)
    state.last_searched_query = state.query.clone();
    state.last_searched_mode = state.mode.clone();
    state.last_searched_case_sensitive = state.case_sensitive;

    // Set current match to first one if any found
    if !state.matches.is_empty() {
        state.current_match = Some(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_pattern_simple() {
        let pattern = parse_hex_pattern("FF D8 FF").unwrap();
        assert_eq!(pattern.len(), 3);
        assert_eq!(pattern[0], PatternElement::Byte(0xFF));
        assert_eq!(pattern[1], PatternElement::Byte(0xD8));
        assert_eq!(pattern[2], PatternElement::Byte(0xFF));
    }

    #[test]
    fn test_parse_hex_pattern_no_spaces() {
        let pattern = parse_hex_pattern("FFD8FF").unwrap();
        assert_eq!(pattern.len(), 3);
        assert_eq!(pattern[0], PatternElement::Byte(0xFF));
        assert_eq!(pattern[1], PatternElement::Byte(0xD8));
        assert_eq!(pattern[2], PatternElement::Byte(0xFF));
    }

    #[test]
    fn test_parse_hex_pattern_wildcard() {
        let pattern = parse_hex_pattern("FF ?? FF").unwrap();
        assert_eq!(pattern.len(), 3);
        assert_eq!(pattern[0], PatternElement::Byte(0xFF));
        assert_eq!(pattern[1], PatternElement::Wildcard);
        assert_eq!(pattern[2], PatternElement::Byte(0xFF));
    }

    #[test]
    fn test_parse_hex_pattern_lowercase() {
        let pattern = parse_hex_pattern("ff d8 ff").unwrap();
        assert_eq!(pattern.len(), 3);
        assert_eq!(pattern[0], PatternElement::Byte(0xFF));
    }

    #[test]
    fn test_parse_hex_pattern_invalid() {
        assert!(parse_hex_pattern("GG").is_err());
        assert!(parse_hex_pattern("").is_err());
        assert!(parse_hex_pattern("F").is_err()); // Incomplete byte
    }

    #[test]
    fn test_search_hex_simple() {
        let data = vec![0x00, 0xFF, 0xD8, 0xFF, 0x00, 0xFF, 0xD8, 0xFF, 0x00];
        let pattern = vec![
            PatternElement::Byte(0xFF),
            PatternElement::Byte(0xD8),
            PatternElement::Byte(0xFF),
        ];
        let matches = search_hex(&data, &pattern);
        assert_eq!(matches, vec![1, 5]);
    }

    #[test]
    fn test_search_hex_wildcard() {
        let data = vec![0xFF, 0x00, 0xFF, 0xFF, 0xAB, 0xFF];
        let pattern = vec![
            PatternElement::Byte(0xFF),
            PatternElement::Wildcard,
            PatternElement::Byte(0xFF),
        ];
        let matches = search_hex(&data, &pattern);
        assert_eq!(matches, vec![0, 3]);
    }

    #[test]
    fn test_search_ascii_case_sensitive() {
        let data = b"Hello World hello";
        let matches = search_ascii(data, "hello", true);
        assert_eq!(matches, vec![12]);
    }

    #[test]
    fn test_search_ascii_case_insensitive() {
        let data = b"Hello World hello";
        let matches = search_ascii(data, "hello", false);
        assert_eq!(matches, vec![0, 12]);
    }

    #[test]
    fn test_search_state_navigation() {
        let mut state = SearchState::default();
        state.matches = vec![10, 20, 30];

        assert_eq!(state.current_match, None);

        state.next_match();
        assert_eq!(state.current_match, Some(0));
        assert_eq!(state.current_match_offset(), Some(10));

        state.next_match();
        assert_eq!(state.current_match, Some(1));
        assert_eq!(state.current_match_offset(), Some(20));

        state.next_match();
        assert_eq!(state.current_match, Some(2));

        state.next_match();
        assert_eq!(state.current_match, Some(0)); // Wrap around

        state.prev_match();
        assert_eq!(state.current_match, Some(2)); // Wrap to end
    }

    #[test]
    fn test_parse_hex_replace_no_wildcards() {
        let bytes = parse_hex_replace("FF D8 FF").unwrap();
        assert_eq!(bytes, vec![0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn test_parse_hex_replace_rejects_wildcards() {
        assert!(parse_hex_replace("FF ?? FF").is_err());
    }

    #[test]
    fn test_query_changed_since_search() {
        let data = b"hello world hello";
        let mut state = SearchState::default();
        state.mode = SearchMode::Ascii;
        state.query = "hello".to_string();

        // Before any search, query_changed should be true (query differs from empty default)
        assert!(state.query_changed_since_search());

        // After search, query_changed should be false
        execute_search(&mut state, data);
        assert!(!state.query_changed_since_search());
        assert_eq!(state.matches.len(), 2);

        // Change query — should detect change
        state.query = "world".to_string();
        assert!(state.query_changed_since_search());

        // Change mode — should detect change
        state.query = "hello".to_string();
        assert!(!state.query_changed_since_search());
        state.mode = SearchMode::Hex;
        assert!(state.query_changed_since_search());

        // Change case sensitivity — should detect change
        state.mode = SearchMode::Ascii;
        assert!(!state.query_changed_since_search());
        state.case_sensitive = true;
        assert!(state.query_changed_since_search());
    }

    #[test]
    fn test_invalid_hex_query_sets_error_message() {
        let data = b"hello";
        let mut state = SearchState::default();
        state.mode = SearchMode::Hex;
        state.query = "GG".to_string();

        execute_search(&mut state, data);

        match &state.message {
            Some(SearchMessage::Error(text)) => {
                assert!(text.contains("Invalid"));
            }
            other => panic!("Expected SearchMessage::Error, got {:?}", other),
        }
    }

    #[test]
    fn test_clear_results_clears_message() {
        let mut state = SearchState::default();
        state.message = Some(SearchMessage::Info("test".to_string()));

        state.clear_results();
        assert!(state.message.is_none());
    }
}
