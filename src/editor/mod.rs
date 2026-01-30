//! Editor module: buffer management, history, and edit operations

mod buffer;
mod history;

pub use buffer::EditorState;
pub use history::{EditOperation, History};
