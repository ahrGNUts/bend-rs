//! Search and replace functionality for the hex editor

/// Search mode - either hex pattern or ASCII string
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMode {
    /// Hex pattern search (e.g., "FF D8 FF" or "FF ?? FF" with wildcards)
    Hex,
    /// ASCII string search
    Ascii,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::Hex
    }
}

/// Parsed search pattern element
#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    /// Exact byte match
    Byte(u8),
    /// Wildcard (matches any byte)
    Wildcard,
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
    /// Current match index (for Next/Previous navigation)
    pub current_match: Option<usize>,
    /// Whether the search dialog is visible
    pub dialog_open: bool,
    /// Replace text
    pub replace_with: String,
    /// Last search error message
    pub error: Option<String>,
}

impl SearchState {
    /// Create a new search state
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the search dialog
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
    }

    /// Close the search dialog
    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
    }

    /// Check if an offset is a match
    pub fn is_match(&self, offset: usize) -> bool {
        self.matches.iter().any(|&m| m == offset)
    }

    /// Check if an offset is within a match (considering pattern length)
    pub fn is_within_match(&self, offset: usize, pattern_len: usize) -> bool {
        self.matches.iter().any(|&m| offset >= m && offset < m + pattern_len)
    }

    /// Get the current pattern length based on query and mode
    pub fn pattern_length(&self) -> usize {
        match self.mode {
            SearchMode::Hex => {
                parse_hex_pattern(&self.query)
                    .map(|p| p.len())
                    .unwrap_or(0)
            }
            SearchMode::Ascii => self.query.len(),
        }
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
        self.current_match.and_then(|i| self.matches.get(i).copied())
    }

    /// Find the nearest match at or after the given offset
    pub fn find_nearest_match(&mut self, offset: usize) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        // Find the first match at or after offset
        for (i, &m) in self.matches.iter().enumerate() {
            if m >= offset {
                self.current_match = Some(i);
                return;
            }
        }
        // Wrap around to first match
        self.current_match = Some(0);
    }

    /// Clear search results
    pub fn clear_results(&mut self) {
        self.matches.clear();
        self.current_match = None;
        self.error = None;
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
            let low_char = chars.next().ok_or_else(|| {
                "Incomplete hex byte: expected second digit".to_string()
            })?;

            // Skip whitespace between digits (e.g., "F F" should work)
            let low_char = if low_char.is_whitespace() {
                chars.next().ok_or_else(|| {
                    "Incomplete hex byte: expected second digit after space".to_string()
                })?
            } else {
                low_char
            };

            let low = low_char.to_digit(16).ok_or_else(|| {
                format!("Invalid hex digit: '{}'", low_char)
            })?;

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

    Ok(elements.iter().filter_map(|e| {
        match e {
            PatternElement::Byte(b) => Some(*b),
            PatternElement::Wildcard => None,
        }
    }).collect())
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

    let query_bytes: Vec<u8> = if case_sensitive {
        query.as_bytes().to_vec()
    } else {
        query.to_lowercase().as_bytes().to_vec()
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

    match state.mode {
        SearchMode::Hex => {
            match parse_hex_pattern(&state.query) {
                Ok(pattern) => {
                    state.matches = search_hex(data, &pattern);
                }
                Err(e) => {
                    state.error = Some(e);
                }
            }
        }
        SearchMode::Ascii => {
            state.matches = search_ascii(data, &state.query, state.case_sensitive);
        }
    }

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
        let mut state = SearchState::new();
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
}
