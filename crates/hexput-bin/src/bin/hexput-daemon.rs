//! Thin OS entry point for the daemon. Parses no real arguments yet — resolves System Config
//! per AD-7's precedence (stubbed) and hands off to `hexput_daemon::run`. No logic beyond
//! argument parsing and hand-off belongs in this file or this crate; see AGENTS.md's Structural
//! Seed for `hexput-bin`'s role.

fn main() {
    let config = hexput_daemon::SystemConfig::resolve();
    hexput_daemon::run(config);
}
