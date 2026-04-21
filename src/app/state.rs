//! Cohesive substates that partition `BendApp` by concern.
//!
//! Each substate groups fields that change together and are consumed by the
//! same set of callers. UI `show()` functions should accept the narrowest
//! substate(s) they need rather than a reference to the full `BendApp`.
//!
//! Migration order (see `openspec/changes/refactor-split-app-state/tasks.md`):
//! 1. `IoState`         — file dialog plumbing and window-resize debounce
//! 2. `AppConfig`       — persisted settings
//! 3. `UiState`         — UI panel/dialog/cache state
//! 4. `DocumentState`   — loaded document, editor, preview

use crate::editor::{EditorState, GoToOffsetState, SearchState};
use crate::formats::{FileSection, RiskLevel};
use crate::settings::AppSettings;
use crate::ui::bookmarks::BookmarksPanelState;
use crate::ui::hex_editor::ContextMenuState;
use crate::ui::savepoints::SavePointsPanelState;
use crate::ui::settings_dialog::SettingsDialogState;
use crate::ui::shortcuts_dialog::ShortcutsDialogState;
use crate::ui::theme::AppColors;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use super::{DialogState, PreviewState};

/// Result of a background file dialog thread.
pub(super) enum FileDialogResult {
    OpenFile(PathBuf),
    ExportSuccess(PathBuf),
    ExportError(String),
    Cancelled,
}

/// I/O plumbing: file-dialog receivers, deferred open paths, window-resize debounce.
#[derive(Default)]
pub struct IoState {
    /// Pending file path to open (for deferred actions from menus)
    pub pending_open_path: Option<PathBuf>,

    /// Last known window size (for change detection)
    pub last_window_size: Option<eframe::egui::Vec2>,

    /// Timer for debouncing window resize saves
    pub window_resize_timer: Option<Instant>,

    /// Receiver for a pending open-file dialog running on a background thread
    pub(super) open_dialog_rx: Option<mpsc::Receiver<FileDialogResult>>,

    /// Receiver for a pending export dialog running on a background thread
    pub(super) export_dialog_rx: Option<mpsc::Receiver<FileDialogResult>>,
}

impl IoState {
    /// Returns true if a file dialog is already open on a background thread.
    pub fn is_dialog_pending(&self) -> bool {
        self.open_dialog_rx.is_some() || self.export_dialog_rx.is_some()
    }
}

/// Application configuration. Currently wraps persisted settings; may grow to
/// hold runtime/non-persisted config without changing substate boundaries.
#[derive(Default)]
pub struct AppConfig {
    /// Application settings (persisted to disk)
    pub settings: AppSettings,
}

/// UI state: colors, dialogs, panel state, ephemeral scroll/selection intents.
///
/// Everything here is rebuilt or refreshed per frame (or per user interaction).
/// Nothing here is persisted — that lives on `AppConfig`. Nothing here owns the
/// document — that lives on `BendApp` (soon `DocumentState`).
#[derive(Default)]
pub struct UiState {
    /// Cached theme-aware color palette, refreshed once per frame in `update()`.
    pub colors: AppColors,

    /// Dialog state (close confirmation, high-risk warnings)
    pub dialogs: DialogState,

    /// Context menu state for hex editor
    pub context_menu_state: ContextMenuState,

    /// Search and replace state
    pub search_state: SearchState,

    /// Go to offset dialog state
    pub go_to_offset_state: GoToOffsetState,

    /// State for the save points panel
    pub savepoints_state: SavePointsPanelState,

    /// State for the bookmarks panel
    pub bookmarks_state: BookmarksPanelState,

    /// Keyboard shortcuts help dialog state
    pub shortcuts_dialog_state: ShortcutsDialogState,

    /// Settings/preferences dialog state
    pub settings_dialog_state: SettingsDialogState,

    /// Pending scroll offset for hex editor (Some(offset) = scroll to this byte offset)
    pub pending_hex_scroll: Option<usize>,
}

/// Document state: the loaded buffer/editor, its preview, parsed structure,
/// and the runtime header-protection toggle.
///
/// Everything in here is derived from or attached to the currently-open file.
/// Closing a file resets this substate; nothing else needs to change.
#[derive(Default)]
pub struct DocumentState {
    /// Editor state containing buffers, history, and file metadata
    pub editor: Option<EditorState>,

    /// Path to currently loaded file (for display purposes)
    pub current_file: Option<PathBuf>,

    /// Image preview state (textures, dirty flag, comparison mode)
    pub preview: PreviewState,

    /// Cached parsed file sections for structure visualization
    /// Re-parsed when file is loaded or structure potentially changed
    pub cached_sections: Option<Vec<FileSection>>,

    /// Whether header protection is enabled (blocks edits to high-risk sections)
    pub header_protection: bool,
}

impl DocumentState {
    /// Find the section containing a byte offset
    pub fn section_at_offset(&self, offset: usize) -> Option<&FileSection> {
        fn find_in_sections(sections: &[FileSection], offset: usize) -> Option<&FileSection> {
            for section in sections {
                if offset >= section.start && offset < section.end {
                    // Check children first for more specific match
                    if let Some(child) = find_in_sections(&section.children, offset) {
                        return Some(child);
                    }
                    return Some(section);
                }
            }
            None
        }

        self.cached_sections
            .as_ref()
            .and_then(|sections| find_in_sections(sections, offset))
    }

    /// Check if an offset is in a protected region (header protection enabled + High/Critical risk)
    pub fn is_offset_protected(&self, offset: usize) -> bool {
        if !self.header_protection {
            return false;
        }

        self.section_at_offset(offset)
            .map(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .unwrap_or(false)
    }

    /// Check if any byte in a range overlaps a protected region
    pub fn is_range_protected(&self, start: usize, len: usize) -> bool {
        if !self.header_protection || len == 0 {
            return false;
        }
        (start..start + len).any(|offset| self.is_offset_protected(offset))
    }

    /// Check if an offset is in a high-risk region that should show a warning.
    /// Returns the risk level if it's High or Critical, None otherwise.
    pub fn get_high_risk_level(&self, offset: usize) -> Option<RiskLevel> {
        self.section_at_offset(offset)
            .filter(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .map(|section| section.risk)
    }
}
