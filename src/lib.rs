//! soroban-cost-benchmarks — Storage-rent forecasting, WASM metrics, and PR comment bot.
//!
//! Built on top of [`soroban-cost-estimator`] for RPC simulation and config snapshots.
//!
//! # Features
//!
//! - **Storage-rent forecasting**: 30/180/365-day projections per storage tier using
//!   real `ConfigSetting*` rent rates from the network.
//! - **WASM static analysis**: Code/data section sizes, function/global counts.
//! - **Multi-scenario benchmarking**: Empty vs. populated state comparisons.
//! - **PR comment bot**: Posts structured cost + rent forecasts inline in GitHub PRs.

pub mod benchmark;
pub mod compare;
pub mod error;
pub mod pr_comment;
pub mod rent_forecast;
pub mod wasm_metrics;

/// Re-export key types for downstream consumers.
pub use error::{BenchError, BenchResult};
