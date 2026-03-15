// Re-export shared display message types so the copilot module uses the same
// wire format as all other tools (Claude, Codex, OpenCode).
pub use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};
