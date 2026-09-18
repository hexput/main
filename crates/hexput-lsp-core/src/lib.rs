//! Language server logic (FR-15). Depends on hexput-lexer, hexput-parser, and hexput-check
//! only — never hexput-interpreter or any daemon crate — so it stays a pure library usable
//! for diagnostics without pulling in execution or host access.
//! Exposes `run_server()`, called by the thin `hexput-lsp` binary in hexput-bin.

/// Entry point for the `hexput-lsp` language server binary.
///
/// Stub: real LSP wiring lands with Epic 9.
pub fn run_server() {
    todo!("language server logic lands with Epic 9")
}
