//! Editor module: buffer management, history, and edit operations

pub mod bookmarks;
pub mod buffer;
pub mod go_to_offset;
mod history;
pub mod savepoints;
pub mod search;

pub use bookmarks::BookmarkManager;
pub use buffer::{EditMode, EditorState, NibblePosition};
pub use go_to_offset::GoToOffsetState;
pub use savepoints::{SavePoint, SavePointManager};
pub use search::SearchState;
