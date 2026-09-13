//! Pipeline module — transformation engine.
//!
//! Public API:
//! - [`transformer`]: the [`Transformer`] trait and per-rule implementations.
//! - [`service`]: the [`service::run_pipeline`] orchestrator.

pub mod service;
pub mod transformer;
pub mod validation;
