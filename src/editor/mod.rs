//! Editor module: buffer management, history, and edit operations

pub mod buffer;
mod history;
pub mod savepoints;
pub mod search;

pub use buffer::{EditorState, NibblePosition};
pub use savepoints::{SavePoint, SavePointManager};
pub use search::SearchState;
