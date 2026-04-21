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

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

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
