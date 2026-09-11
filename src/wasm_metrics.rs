//! WASM static analysis for Soroban contracts.
//!
//! Analyzes compiled `.wasm` files for:
//! - Code/data section sizes
//! - Function, global, table, and memory counts
//! - Export/import structure
//! - Total binary size

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::BenchError;

/// Complete WASM metrics for a Soroban contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmMetrics {
    /// Path to the WASM file.
    pub path: String,
    /// Total binary size in bytes.
    pub total_size: u64,
    /// Code section size in bytes.
    pub code_section_size: u64,
    /// Data section size in bytes.
    pub data_section_size: u64,
    /// Custom sections total size (includes Soroban contract spec).
    pub custom_section_size: u64,
    /// Number of exported functions.
    pub function_count: u32,
    /// Number of exported globals.
    pub global_count: u32,
    /// Number of tables.
    pub table_count: u32,
    /// Number of declared memories.
    pub memory_count: u32,
    /// Number of imports.
    pub import_count: u32,
    /// Number of exports (functions + globals + memories).
    pub export_count: u32,
    /// Whether a Soroban contract spec section was found.
    pub has_contract_spec: bool,
    /// Exported function names.
    pub exported_functions: Vec<String>,
}

/// Analyze a WASM file and return metrics.
///
/// # Errors
///
/// Returns `BenchError` if the file cannot be read or parsed.
pub fn analyze_wasm(path: &Path) -> Result<WasmMetrics, BenchError> {
    let bytes = std::fs::read(path)?;
    analyze_wasm_bytes(&bytes, path.to_string_lossy().as_ref())
}

/// Analyze WASM bytes directly.
///
/// # Errors
///
/// Returns `BenchError::WasmParse` if the WASM is invalid.
pub fn analyze_wasm_bytes(bytes: &[u8], path_label: &str) -> Result<WasmMetrics, BenchError> {
    wasmparser::validate(bytes).map_err(|e| BenchError::WasmParse(e.to_string()))?;

    let mut metrics = WasmMetrics {
        path: path_label.to_string(),
        total_size: bytes.len() as u64,
        code_section_size: 0,
        data_section_size: 0,
        custom_section_size: 0,
        function_count: 0,
        global_count: 0,
        table_count: 0,
        memory_count: 0,
        import_count: 0,
        export_count: 0,
        has_contract_spec: false,
        exported_functions: Vec::new(),
    };

    // wasmparser 0.254 splits CodeSection into CodeSectionStart + CodeSectionEntry
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|e| BenchError::WasmParse(e.to_string()))?;
        match payload {
            wasmparser::Payload::CodeSectionStart { range, size, .. } => {
                metrics.code_section_size = (range.end - range.start) as u64;
                metrics.function_count = size;
            }

            wasmparser::Payload::DataSection(section) => {
                metrics.data_section_size = section.range().len() as u64;
            }
            wasmparser::Payload::CustomSection(section) => {
                let name = section.name();
                let size = section.data().len() as u64;
                metrics.custom_section_size += size;
                if name == "contractspecv0" {
                    metrics.has_contract_spec = true;
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|e| BenchError::WasmParse(e.to_string()))?;
                    metrics.export_count += 1;
                    match export.kind {
                        wasmparser::ExternalKind::Func => {
                            metrics.exported_functions.push(export.name.to_string());
                        }
                        wasmparser::ExternalKind::Global => {
                            metrics.global_count += 1;
                        }
                        wasmparser::ExternalKind::Table => {
                            metrics.table_count += 1;
                        }
                        wasmparser::ExternalKind::Memory => {
                            metrics.memory_count += 1;
                        }
                        _ => {}
                    }
                }
            }
            wasmparser::Payload::ImportSection(section) => {
                for group in section {
                    let group = group.map_err(|e| BenchError::WasmParse(e.to_string()))?;
                    match group {
                        wasmparser::Imports::Single(_, _) => metrics.import_count += 1,
                        wasmparser::Imports::Compact1 { .. }
                        | wasmparser::Imports::Compact2 { .. } => {
                            metrics.import_count += 1;
                        }
                    }
                }
            }
            wasmparser::Payload::GlobalSection(section) => {
                metrics.global_count += section.count();
            }
            wasmparser::Payload::TableSection(section) => {
                metrics.table_count += section.count();
            }
            wasmparser::Payload::MemorySection(section) => {
                metrics.memory_count += section.count();
            }
            _ => {}
        }
    }

    Ok(metrics)
}

/// Format WASM metrics as a human-readable table.
#[must_use]
pub fn format_metrics_table(metrics: &WasmMetrics) -> String {
    use comfy_table::{Cell, Color, Table};

    let mut table = Table::new();
    table.set_header(vec!["Metric", "Value"]);

    let rows = [
        ("File", metrics.path.clone()),
        (
            "Total size",
            format!(
                "{} bytes ({:.1} KB)",
                metrics.total_size,
                metrics.total_size as f64 / 1024.0
            ),
        ),
        (
            "Code section",
            format!(
                "{} bytes ({:.1} KB)",
                metrics.code_section_size,
                metrics.code_section_size as f64 / 1024.0
            ),
        ),
        (
            "Data section",
            format!(
                "{} bytes ({:.1} KB)",
                metrics.data_section_size,
                metrics.data_section_size as f64 / 1024.0
            ),
        ),
        (
            "Custom sections",
            format!(
                "{} bytes ({:.1} KB)",
                metrics.custom_section_size,
                metrics.custom_section_size as f64 / 1024.0
            ),
        ),
        ("Functions (exported)", metrics.function_count.to_string()),
        ("Globals", metrics.global_count.to_string()),
        ("Tables", metrics.table_count.to_string()),
        ("Memories", metrics.memory_count.to_string()),
        ("Imports", metrics.import_count.to_string()),
        ("Exports", metrics.export_count.to_string()),
        ("Has contract spec", metrics.has_contract_spec.to_string()),
    ];

    for (label, value) in &rows {
        table.add_row(vec![Cell::new(label).fg(Color::Cyan), Cell::new(value)]);
    }

    if !metrics.exported_functions.is_empty() {
        table.add_row(vec![
            Cell::new("Exported functions").fg(Color::Cyan),
            Cell::new(metrics.exported_functions.join(", ")),
        ]);
    }

    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_wasm_bytes() -> Vec<u8> {
        wat::parse_str(
            r#"(module
                (func (export "hello") (result i32)
                    i32.const 42
                )
            )"#,
        )
        .unwrap()
    }

    #[test]
    fn test_analyze_minimal_wasm() {
        let wasm = minimal_wasm_bytes();
        let metrics = analyze_wasm_bytes(&wasm, "test.wasm").unwrap();
        assert_eq!(metrics.total_size, wasm.len() as u64);
        assert!(metrics.code_section_size > 0);
        assert_eq!(metrics.exported_functions, vec!["hello".to_string()]);
        assert!(!metrics.has_contract_spec);
    }

    #[test]
    fn test_invalid_wasm() {
        let result = analyze_wasm_bytes(&[0x00, 0x01, 0x02], "bad.wasm");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_wasm_invalid() {
        let result = analyze_wasm_bytes(&[], "empty.wasm");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_metrics_table() {
        let wasm = minimal_wasm_bytes();
        let metrics = analyze_wasm_bytes(&wasm, "test.wasm").unwrap();
        let table = format_metrics_table(&metrics);
        assert!(table.contains("test.wasm"));
        assert!(table.contains("hello"));
    }
}
