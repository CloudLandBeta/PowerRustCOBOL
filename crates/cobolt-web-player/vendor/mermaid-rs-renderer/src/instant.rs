//! A clock that exists on every target (PowerRustCOBOL's patch to the vendored crate).
//!
//! `std::time::Instant::now()` panics on `wasm32-unknown-unknown`, and the layout stages read it
//! unconditionally to report how long they took. Elsewhere this is `std::time::Instant`; on `wasm32`
//! it is a clock that always reads zero, so a stage timing is `0` there and nothing else changes.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
mod fixed {
    use std::time::Duration;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Instant;

    impl Instant {
        pub fn now() -> Instant {
            Instant
        }

        pub fn elapsed(&self) -> Duration {
            Duration::ZERO
        }

        pub fn duration_since(&self, _earlier: Instant) -> Duration {
            Duration::ZERO
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use fixed::Instant;
