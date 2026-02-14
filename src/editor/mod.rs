//! Editor module: buffer management, history, and edit operations

pub mod bookmarks;
pub mod buffer;
mod cursor;
pub mod go_to_offset;
mod history;
mod modes;
pub mod savepoints;
pub mod search;
mod selection;

pub use buffer::EditorState;
pub use go_to_offset::GoToOffsetState;
pub use search::SearchState;
