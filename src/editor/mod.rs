//! Editor module: buffer management, history, and edit operations

pub mod buffer;
mod history;

pub use buffer::{EditorState, NibblePosition};
pub use history::{EditOperation, History};
