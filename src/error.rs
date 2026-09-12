//! Error types for soroban-cost-benchmarks.

use thiserror::Error;

/// Errors that can occur during benchmarking operations.
#[derive(Debug, Error)]
pub enum BenchError {
    /// I/O error (file read/write, network).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// CSV error.
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    /// WASM parsing error.
    #[error("WASM parse error: {0}")]
    WasmParse(String),

    /// Soroban-cost-estimator error (wrapped).
    #[error("Estimator error: {0}")]
    Estimator(String),

    /// Missing required config data for rent calculation.
    #[error("Missing config data: {0}")]
    MissingConfig(String),

    /// Failed to fetch or decode live network configuration.
    #[error("live config error: {0}")]
    LiveConfig(String),

    /// GitHub API error.
    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    /// Invalid storage tier specified.
    #[error("Invalid storage tier: {0}")]
    InvalidTier(String),
}

/// Convenience result alias.
pub type BenchResult<T> = Result<T, BenchError>;
