//! Audio pipelines - high-level orchestration
//!
//! Composes pure functions and operations into complete workflows.

mod setup_session;
mod teardown_session;

pub use setup_session::setup_audio_session;
pub use teardown_session::teardown_audio_session;
