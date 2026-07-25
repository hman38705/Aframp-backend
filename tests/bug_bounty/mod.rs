//! Bug Bounty Programme integration test modules.
//!
//! Each module below can be run independently, e.g.:
//!   cargo test --test bug_bounty_integration bug_bounty::lifecycle_tests::
//!   cargo test --test bug_bounty_integration bug_bounty::duplicate_detection_tests::
//!   cargo test --test bug_bounty_integration bug_bounty::invitation_tests::
//!   cargo test --test bug_bounty_integration bug_bounty::transition_tests::

pub mod helpers;

mod duplicate_detection_tests;
mod invitation_tests;
mod lifecycle_tests;
mod transition_tests;

#[cfg(feature = "integration")]
mod db_integration_tests;
