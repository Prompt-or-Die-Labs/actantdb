//! Provider adapters. Each provider is feature-gated; the default CI build
//! never compiles any of them.

#[cfg(feature = "fastembed")]
pub mod fastembed;

#[cfg(feature = "openai")]
pub mod openai;
