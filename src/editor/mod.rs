//! Editor module: buffer management, history, and edit operations

pub mod bookmarks;
pub mod buffer;
mod history;
pub mod savepoints;
pub mod search;

pub use bookmarks::BookmarkManager;
pub use buffer::{EditorState, NibblePosition};
pub use savepoints::{SavePoint, SavePointManager};
pub use search::SearchState;
