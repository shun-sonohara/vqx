//! JSON Normalization for git-friendly diffs
//!
//! This module normalizes JSON output from the Vantiq CLI to produce
//! stable, diff-friendly output suitable for version control.
//!
//! Features:
//! - Alphabetically sorted object keys
//! - Stable array ordering (by name/id fields)
//! - Removal of volatile fields (timestamps, versions)
//!
//! Based on: CLI Reference Guide PDF - Export section
//! The export command produces JSON files that this module normalizes.

use crate::config::NormalizationConfig;
use crate::error::{Result, VqxError};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

/// JSON Normalizer for producing stable, diff-friendly output
pub struct Normalizer {
    config: NormalizationConfig,
}

impl Normalizer {
    /// Create a new normalizer with default configuration
    pub fn new() -> Self {
        Self {
            config: NormalizationConfig::default(),
        }
    }

    /// Create a normalizer with custom configuration
    pub fn with_config(config: NormalizationConfig) -> Self {
        Self { config }
    }

    /// Normalize a JSON value
    pub fn normalize(&self, value: &Value) -> Value {
        self.normalize_value(value)
    }

    /// Normalize a JSON string
    pub fn normalize_str(&self, json_str: &str) -> Result<String> {
        let value: Value = serde_json::from_str(json_str)?;
        let normalized = self.normalize(&value);
        Ok(serde_json::to_string_pretty(&normalized)?)
    }

    /// Normalize a JSON file in place
    pub fn normalize_file(&self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|_| VqxError::FileReadFailed {
            path: path.display().to_string(),
        })?;

        let normalized = self.normalize_str(&content)?;

        fs::write(path, normalized).map_err(|_| VqxError::FileWriteFailed {
            path: path.display().to_string(),
        })?;

        debug!(path = %path.display(), "Normalized JSON file");
        Ok(())
    }

    /// Normalize all JSON files in a directory recursively
    pub fn normalize_directory(&self, dir: &Path) -> Result<NormalizationStats> {
        let mut stats = NormalizationStats::default();

        if !dir.is_dir() {
            return Err(VqxError::Other(format!(
                "Not a directory: {}",
                dir.display()
            )));
        }

        self.normalize_directory_recursive(dir, &mut stats)?;

        info!(
            files = stats.files_processed,
            errors = stats.errors,
            "Normalization complete"
        );

        Ok(stats)
    }

    fn normalize_directory_recursive(&self, dir: &Path, stats: &mut NormalizationStats) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| VqxError::Other(e.to_string()))? {
            let entry = entry.map_err(|e| VqxError::Other(e.to_string()))?;
            let path = entry.path();

            if path.is_dir() {
                self.normalize_directory_recursive(&path, stats)?;
            } else if path.extension().map(|e| e == "json").unwrap_or(false) {
                match self.normalize_file(&path) {
                    Ok(()) => stats.files_processed += 1,
                    Err(e) => {
                        stats.errors += 1;
                        stats.error_files.push((path.display().to_string(), e.to_string()));
                    }
                }
            }
        }

        Ok(())
    }

    /// Recursively normalize a JSON value
    fn normalize_value(&self, value: &Value) -> Value {
        match value {
            Value::Object(map) => self.normalize_object(map),
            Value::Array(arr) => self.normalize_array(arr),
            _ => value.clone(),
        }
    }

    /// Normalize a JSON object
    /// - Sort keys alphabetically (using BTreeMap)
    /// - Remove excluded fields
    /// - Recursively normalize nested values
    fn normalize_object(&self, map: &Map<String, Value>) -> Value {
        let mut sorted: BTreeMap<String, Value> = BTreeMap::new();

        for (key, value) in map {
            // Skip excluded fields
            if self.config.excluded_fields.contains(key) {
                continue;
            }

            // Recursively normalize the value
            let normalized_value = self.normalize_value(value);
            sorted.insert(key.clone(), normalized_value);
        }

        // Convert BTreeMap back to serde_json Map (which preserves insertion order)
        let result: Map<String, Value> = sorted.into_iter().collect();
        Value::Object(result)
    }

    /// Normalize a JSON array
    /// - Sort by configured sort fields (name, id, _id)
    /// - Recursively normalize elements
    fn normalize_array(&self, arr: &[Value]) -> Value {
        // First, normalize all elements
        let mut normalized: Vec<Value> = arr.iter().map(|v| self.normalize_value(v)).collect();

        // Sort if enabled and array contains objects
        if self.config.sort_arrays && !normalized.is_empty() {
            if normalized.iter().all(|v| v.is_object()) {
                normalized.sort_by(|a, b| self.compare_objects(a, b));
            }
        }

        Value::Array(normalized)
    }

    /// Compare two JSON objects for sorting
    /// Uses configured sort fields in priority order
    fn compare_objects(&self, a: &Value, b: &Value) -> Ordering {
        for field in &self.config.array_sort_fields {
            let a_val = a.get(field);
            let b_val = b.get(field);

            match (a_val, b_val) {
                (Some(av), Some(bv)) => {
                    let cmp = self.compare_values(av, bv);
                    if cmp != Ordering::Equal {
                        return cmp;
                    }
                }
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => continue,
            }
        }

        Ordering::Equal
    }

    /// Compare two JSON values
    fn compare_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::String(s1), Value::String(s2)) => s1.cmp(s2),
            (Value::Number(n1), Value::Number(n2)) => {
                let f1 = n1.as_f64().unwrap_or(0.0);
                let f2 = n2.as_f64().unwrap_or(0.0);
                f1.partial_cmp(&f2).unwrap_or(Ordering::Equal)
            }
            (Value::Bool(b1), Value::Bool(b2)) => b1.cmp(b2),
            _ => Ordering::Equal,
        }
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics from normalization operation
#[derive(Debug, Default)]
pub struct NormalizationStats {
    pub files_processed: usize,
    pub errors: usize,
    pub error_files: Vec<(String, String)>,
}

/// Resource-specific normalizer that understands Vantiq resource types
/// Based on PDF export directory structure:
/// - types/
/// - procedures/
/// - rules/
/// - sources/
/// - etc.
pub struct ResourceNormalizer {
    base_normalizer: Normalizer,
}

impl ResourceNormalizer {
    pub fn new(config: NormalizationConfig) -> Self {
        Self {
            base_normalizer: Normalizer::with_config(config),
        }
    }

    /// Normalize based on resource type
    /// Different resources may have different normalization needs
    pub fn normalize_resource(&self, resource_type: &str, value: &Value) -> Value {
        match resource_type {
            // Types may have schema definitions that need special handling
            "types" => self.normalize_type(value),
            // Procedures are VAIL code, stored differently
            "procedures" | "rules" => value.clone(), // Don't modify code files
            // Default normalization for other resources
            _ => self.base_normalizer.normalize(value),
        }
    }

    /// Normalize a type definition
    fn normalize_type(&self, value: &Value) -> Value {
        let mut normalized = self.base_normalizer.normalize(value);

        // Sort properties array by name if present
        if let Value::Object(ref mut map) = normalized {
            if let Some(Value::Array(props)) = map.get("properties") {
                let mut sorted_props = props.clone();
                sorted_props.sort_by(|a, b| {
                    let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    name_a.cmp(name_b)
                });
                map.insert("properties".to_string(), Value::Array(sorted_props));
            }

            // Sort indexes by name if present
            if let Some(Value::Array(indexes)) = map.get("indexes") {
                let mut sorted_indexes = indexes.clone();
                sorted_indexes.sort_by(|a, b| {
                    let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    name_a.cmp(name_b)
                });
                map.insert("indexes".to_string(), Value::Array(sorted_indexes));
            }
        }

        normalized
    }

    /// Normalize an export directory
    /// PDF: Export creates directories like types/, procedures/, rules/, etc.
    pub fn normalize_export_directory(&self, dir: &Path) -> Result<NormalizationStats> {
        let mut stats = NormalizationStats::default();

        // Known resource directories from PDF Export section
        let resource_dirs = [
            "aicomponents",
            "catalogs",
            "clients",
            "collaborationtypes",
            "configurations",
            "debugconfigs",
            "deployconfigs",
            "environments",
            "procedures",
            "projects",
            "rules",
            "scheduledevents",
            "services",
            "sources",
            "subscriptions",
            "systemmodels",
            "topics",
            "types",
            "data",
            "documents",
        ];

        for resource_type in &resource_dirs {
            let resource_dir = dir.join(resource_type);
            if resource_dir.is_dir() {
                debug!(resource_type, "Normalizing resource directory");
                self.normalize_resource_directory(&resource_dir, resource_type, &mut stats)?;
            }
        }

        Ok(stats)
    }

    fn normalize_resource_directory(
        &self,
        dir: &Path,
        resource_type: &str,
        stats: &mut NormalizationStats,
    ) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| VqxError::Other(e.to_string()))? {
            let entry = entry.map_err(|e| VqxError::Other(e.to_string()))?;
            let path = entry.path();

            if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                match self.normalize_resource_file(&path, resource_type) {
                    Ok(()) => stats.files_processed += 1,
                    Err(e) => {
                        stats.errors += 1;
                        stats.error_files.push((path.display().to_string(), e.to_string()));
                    }
                }
            } else if path.is_dir() {
                // Recurse into subdirectories
                self.normalize_resource_directory(&path, resource_type, stats)?;
            }
        }

        Ok(())
    }

    fn normalize_resource_file(&self, path: &Path, resource_type: &str) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|_| VqxError::FileReadFailed {
            path: path.display().to_string(),
        })?;

        let value: Value = serde_json::from_str(&content)?;
        let normalized = self.normalize_resource(resource_type, &value);
        let output = serde_json::to_string_pretty(&normalized)?;

        fs::write(path, output).map_err(|_| VqxError::FileWriteFailed {
            path: path.display().to_string(),
        })?;

        debug!(path = %path.display(), resource_type, "Normalized resource file");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_sorts_keys() {
        let normalizer = Normalizer::new();
        let input: Value = serde_json::json!({
            "zebra": 1,
            "apple": 2,
            "mango": 3
        });

        let output = normalizer.normalize(&input);
        let output_str = serde_json::to_string(&output).unwrap();

        // Keys should be sorted alphabetically
        assert!(output_str.find("apple").unwrap() < output_str.find("mango").unwrap());
        assert!(output_str.find("mango").unwrap() < output_str.find("zebra").unwrap());
    }

    #[test]
    fn test_normalize_excludes_fields() {
        let normalizer = Normalizer::new();
        let input: Value = serde_json::json!({
            "name": "test",
            "ars_modifiedAt": "2024-01-01",
            "ars_createdAt": "2024-01-01",
            "_id": "12345"
        });

        let output = normalizer.normalize(&input);

        assert!(output.get("name").is_some());
        assert!(output.get("ars_modifiedAt").is_none());
        assert!(output.get("ars_createdAt").is_none());
        assert!(output.get("_id").is_none());
    }

    #[test]
    fn test_normalize_sorts_arrays_by_name() {
        let normalizer = Normalizer::new();
        let input: Value = serde_json::json!([
            {"name": "charlie", "value": 3},
            {"name": "alice", "value": 1},
            {"name": "bob", "value": 2}
        ]);

        let output = normalizer.normalize(&input);
        let arr = output.as_array().unwrap();

        assert_eq!(arr[0].get("name").unwrap(), "alice");
        assert_eq!(arr[1].get("name").unwrap(), "bob");
        assert_eq!(arr[2].get("name").unwrap(), "charlie");
    }

    #[test]
    fn test_normalize_nested_objects() {
        let normalizer = Normalizer::new();
        let input: Value = serde_json::json!({
            "outer": {
                "zebra": 1,
                "apple": 2
            }
        });

        let output = normalizer.normalize(&input);
        let inner = output.get("outer").unwrap();
        let inner_str = serde_json::to_string(inner).unwrap();

        assert!(inner_str.find("apple").unwrap() < inner_str.find("zebra").unwrap());
    }

    #[test]
    fn test_custom_excluded_fields() {
        let config = NormalizationConfig {
            excluded_fields: vec!["custom_field".to_string()],
            ..Default::default()
        };
        let normalizer = Normalizer::with_config(config);

        let input: Value = serde_json::json!({
            "name": "test",
            "custom_field": "should be removed"
        });

        let output = normalizer.normalize(&input);

        assert!(output.get("name").is_some());
        assert!(output.get("custom_field").is_none());
    }
}
