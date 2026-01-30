//! Editor module: buffer management, history, and edit operations

pub mod buffer;
mod history;
pub mod savepoints;

pub use buffer::{EditorState, NibblePosition};
pub use savepoints::{SavePoint, SavePointManager};
