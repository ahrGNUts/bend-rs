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

use crate::editor::{GoToOffsetState, SearchState};
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

use super::DialogState;

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
