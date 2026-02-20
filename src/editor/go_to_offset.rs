//! Go to offset functionality for the hex editor

/// State for the "Go to offset" dialog
#[derive(Debug, Default)]
pub struct GoToOffsetState {
    /// Whether the dialog is visible
    pub dialog_open: bool,
    /// The user's input text (can be decimal or hex with 0x prefix)
    pub input_text: String,
    /// Error message for invalid input
    pub error: Option<String>,
}

impl GoToOffsetState {
    /// Open the dialog, clearing previous input
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
        self.input_text.clear();
        self.error = None;
    }

    /// Close the dialog
    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
    }
}

/// Parse an offset string (supports decimal or hex with 0x/0X prefix)
///
/// # Examples
/// - "1024" -> Ok(1024)
/// - "0x400" -> Ok(1024)
/// - "0X400" -> Ok(1024)
/// - "invalid" -> Err(...)
pub fn parse_offset(input: &str) -> Result<usize, String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("Please enter an offset".to_string());
    }

    // Try hex with 0x/0X prefix
    if let Some(hex_part) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        if hex_part.is_empty() {
            return Err("Invalid hex value: missing digits after 0x".to_string());
        }
        return usize::from_str_radix(hex_part, 16)
            .map_err(|_| format!("Invalid hex value: {}", trimmed));
    }

    // Try decimal
    trimmed.parse::<usize>().map_err(|_| {
        format!(
            "Invalid offset '{}' (use decimal or 0x prefix for hex)",
            trimmed
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_offset("0"), Ok(0));
        assert_eq!(parse_offset("1024"), Ok(1024));
        assert_eq!(parse_offset("  1024  "), Ok(1024)); // with whitespace
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_offset("0x0"), Ok(0));
        assert_eq!(parse_offset("0x400"), Ok(1024));
        assert_eq!(parse_offset("0X400"), Ok(1024)); // uppercase X
        assert_eq!(parse_offset("0xFF"), Ok(255));
        assert_eq!(parse_offset("0xff"), Ok(255)); // lowercase
        assert_eq!(parse_offset("0xABCD"), Ok(0xABCD));
        assert_eq!(parse_offset("  0x400  "), Ok(1024)); // with whitespace
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_offset("").is_err());
        assert!(parse_offset("   ").is_err());
        assert!(parse_offset("abc").is_err());
        assert!(parse_offset("0x").is_err());
        assert!(parse_offset("0xGGG").is_err());
        assert!(parse_offset("-1").is_err());
    }

    #[test]
    fn test_dialog_state() {
        let mut state = GoToOffsetState::default();

        // Initially closed
        assert!(!state.dialog_open);
        assert!(state.input_text.is_empty());
        assert!(state.error.is_none());

        // Open dialog
        state.input_text = "old value".to_string();
        state.error = Some("old error".to_string());
        state.open_dialog();

        assert!(state.dialog_open);
        assert!(state.input_text.is_empty()); // cleared
        assert!(state.error.is_none()); // cleared

        // Close dialog
        state.close_dialog();
        assert!(!state.dialog_open);
    }
}
