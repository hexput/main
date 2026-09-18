//! Central test crate for the workspace. Deliberately empty: every test lives under `tests/`,
//! one file per crate under test, and each crate under test is a dev-dependency.
//!
//! Two consequences worth knowing before adding a test here.
//!
//! **These are integration tests, not unit tests.** A separate crate sees only the public API of
//! the crate it tests, so anything reaching into a private function has to stay in a
//! `#[cfg(test)] mod tests` inside its own crate. That is the exception, not the rule — prefer
//! testing the public surface here, since that is what the rest of the workspace depends on.
//!
//! **Crates under test are dev-dependencies.** `scripts/check-crate-graph.py` enforces the
//! Architecture Spine's dependency rules over normal dependencies only, because those rules are
//! about what production code can reach. Keeping the edges here in `[dev-dependencies]` lets this
//! crate test `hexput-enforce` (AD-3) or `hexput-transport` (AD-1) without claiming a production
//! path to them.
