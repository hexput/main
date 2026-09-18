//! Thin OS entry point for the `hexput` CLI (eval + check). Parses no real arguments yet;
//! hands off to `hexput_cli_core::run`. No logic beyond argument parsing and hand-off belongs
//! in this file or this crate; see AGENTS.md's Structural Seed for `hexput-bin`'s role.

fn main() {
    hexput_cli_core::run();
}
