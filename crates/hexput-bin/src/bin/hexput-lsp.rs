//! Thin OS entry point for the `hexput-lsp` language server. Parses no real arguments yet;
//! hands off to `hexput_lsp_core::run_server`. No logic beyond argument parsing and hand-off
//! belongs in this file or this crate; see AGENTS.md's Structural Seed for `hexput-bin`'s role.

fn main() {
    hexput_lsp_core::run_server();
}
