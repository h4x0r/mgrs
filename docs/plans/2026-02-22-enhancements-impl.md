# mgrs2latlong v0.2.0 Enhancement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform mgrs2latlong from a single-file MGRS→LatLon CSV converter into a modular, bidirectional coordinate conversion tool with multiple output formats (CSV, GeoJSON, KML, GPX), streaming processing, and a reusable library API.

**Architecture:** Modular monolith — library crate (`lib.rs`) with modules for conversion, detection, formats, and streaming, plus a thin CLI binary (`main.rs`) using clap subcommands. All modules are unit-tested independently.

**Tech Stack:** Rust 1.87+, clap 4 (derive), csv 1.3, geoconvert 1.0, regex 1.10, anyhow 1.0, serde/serde_json (GeoJSON), quick-xml (KML/GPX), indicatif (progress bars)

**Design doc:** `docs/plans/2026-02-22-enhancements-design.md`

---

## Phase 1: Restructure into Library + Binary

### Task 1: Add new dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update Cargo.toml**

Add to `[dependencies]`:
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
quick-xml = "0.37"
indicatif = "0.17"
```

Add `[lib]` section and update package metadata:
```toml
[lib]
name = "mgrs2latlong"
path = "src/lib.rs"

[[bin]]
name = "mgrs2latlong"
path = "src/main.rs"
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Success (lib.rs doesn't exist yet, but we'll create it next)

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "build: add dependencies for v0.2.0 enhancements"
```

---

### Task 2: Extract convert module with TDD

**Files:**
- Create: `src/convert.rs`
- Create: `src/lib.rs`

**Step 1: Write failing tests for mgrs_to_latlon**

Create `src/convert.rs` with only tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mgrs_to_latlon_known_coordinate() {
        // 18SUJ2337006519 is near Washington DC
        let result = mgrs_to_latlon("18SUJ2337006519").unwrap();
        assert!((result.latitude - 38.8977).abs() < 0.01);
        assert!((result.longitude - (-77.0365)).abs() < 0.01);
    }

    #[test]
    fn test_mgrs_to_latlon_with_spaces() {
        let result = mgrs_to_latlon("18S UJ 23370 06519").unwrap();
        assert!((result.latitude - 38.8977).abs() < 0.01);
    }

    #[test]
    fn test_mgrs_to_latlon_invalid() {
        let result = mgrs_to_latlon("INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_mgrs_to_latlon_empty() {
        let result = mgrs_to_latlon("");
        assert!(result.is_err());
    }

    #[test]
    fn test_latlon_to_mgrs_known_coordinate() {
        // Washington DC
        let result = latlon_to_mgrs(38.8977, -77.0365, 5).unwrap();
        assert!(result.0.starts_with("18SUJ"));
    }

    #[test]
    fn test_latlon_to_mgrs_precision_3() {
        let result = latlon_to_mgrs(38.8977, -77.0365, 3).unwrap();
        // 3-digit precision = 6 digits in easting+northing
        let digits: String = result.0.chars().filter(|c| c.is_ascii_digit()).collect();
        // Grid zone (2 digits) + 6 easting/northing digits = 8 total
        assert!(digits.len() == 8, "Expected 8 digits, got {}: {}", digits.len(), result.0);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let original_lat = 51.5074;
        let original_lon = -0.1278;
        let mgrs = latlon_to_mgrs(original_lat, original_lon, 5).unwrap();
        let back = mgrs_to_latlon(&mgrs.0).unwrap();
        assert!((back.latitude - original_lat).abs() < 0.001);
        assert!((back.longitude - original_lon).abs() < 0.001);
    }
}
```

Create minimal `src/lib.rs`:
```rust
pub mod convert;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib convert`
Expected: FAIL — `mgrs_to_latlon` and `latlon_to_mgrs` not defined, `Coordinate` and `MgrsCoord` types don't exist

**Step 3: Implement minimal convert module**

Add to top of `src/convert.rs` (above the tests):

```rust
use anyhow::{Context, Result};
use geoconvert::Mgrs;

/// A latitude/longitude coordinate pair.
#[derive(Debug, Clone, Copy)]
pub struct Coordinate {
    pub latitude: f64,
    pub longitude: f64,
}

/// An MGRS coordinate string.
#[derive(Debug, Clone)]
pub struct MgrsCoord(pub String);

/// Convert an MGRS string to latitude/longitude.
pub fn mgrs_to_latlon(mgrs_str: &str) -> Result<Coordinate> {
    let normalized = mgrs_str.replace(" ", "");
    if normalized.is_empty() {
        anyhow::bail!("Empty MGRS string");
    }
    let mgrs = Mgrs::parse_str(&normalized)
        .with_context(|| format!("Failed to parse MGRS coordinate: {}", mgrs_str))?;
    let latlon = mgrs.to_latlon();
    Ok(Coordinate {
        latitude: latlon.latitude(),
        longitude: latlon.longitude(),
    })
}

/// Convert latitude/longitude to an MGRS string with given precision (1-5).
pub fn latlon_to_mgrs(lat: f64, lon: f64, precision: u8) -> Result<MgrsCoord> {
    let mgrs_str = geoconvert::Mgrs::from_latlon(lat, lon, precision as i32)
        .with_context(|| format!("Failed to convert ({}, {}) to MGRS", lat, lon))?
        .to_string();
    Ok(MgrsCoord(mgrs_str))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib convert -- --nocapture`
Expected: All 7 tests PASS

**Step 5: Commit**

```bash
git add src/convert.rs src/lib.rs
git commit -m "feat: extract convert module with bidirectional MGRS/LatLon conversion"
```

---

### Task 3: Extract detect module with TDD

**Files:**
- Create: `src/detect.rs`
- Modify: `src/lib.rs`

**Step 1: Write failing tests for detection logic**

Create `src/detect.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_mgrs_valid() {
        assert!(is_likely_mgrs("18SUJ2337006519"));
        assert!(is_likely_mgrs("4QFJ1234567890"));
        assert!(is_likely_mgrs("31U DQ 48251 11932"));
    }

    #[test]
    fn test_is_likely_mgrs_invalid() {
        assert!(!is_likely_mgrs("hello"));
        assert!(!is_likely_mgrs("12345"));
        assert!(!is_likely_mgrs(""));
        assert!(!is_likely_mgrs("38.8977"));
    }

    #[test]
    fn test_detect_mgrs_column_finds_correct_column() {
        let records = vec![
            csv::StringRecord::from(vec!["Name", "18SUJ2337006519", "Note"]),
            csv::StringRecord::from(vec!["Place", "33UUP0100010000", "Info"]),
        ];
        assert_eq!(detect_mgrs_column(&records), Some(1));
    }

    #[test]
    fn test_detect_mgrs_column_no_mgrs() {
        let records = vec![
            csv::StringRecord::from(vec!["Name", "Value", "Note"]),
        ];
        assert_eq!(detect_mgrs_column(&records), None);
    }

    #[test]
    fn test_detect_mgrs_column_empty() {
        let records: Vec<csv::StringRecord> = vec![];
        assert_eq!(detect_mgrs_column(&records), None);
    }

    #[test]
    fn test_detect_mgrs_column_first_column() {
        let records = vec![
            csv::StringRecord::from(vec!["18SUJ2337006519", "Washington DC"]),
            csv::StringRecord::from(vec!["33UUP0100010000", "Somewhere"]),
        ];
        assert_eq!(detect_mgrs_column(&records), Some(0));
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod convert;
pub mod detect;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib detect`
Expected: FAIL — `is_likely_mgrs` and `detect_mgrs_column` not defined

**Step 3: Implement detect module**

Add to top of `src/detect.rs`:

```rust
use std::sync::OnceLock;
use regex::Regex;

fn mgrs_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b\d{1,2}\s*[C-X]\s*[A-Z]{2}\s*\d{2,10}\b").unwrap())
}

/// Check if a string value looks like an MGRS coordinate.
pub fn is_likely_mgrs(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 7 && mgrs_regex().is_match(trimmed)
}

/// Auto-detect which column in a set of CSV records contains MGRS coordinates.
/// Scores each column by how many of the first 100 rows match the MGRS pattern.
/// Returns None if no column has any matches.
pub fn detect_mgrs_column(records: &[csv::StringRecord]) -> Option<usize> {
    if records.is_empty() {
        return None;
    }

    let num_columns = records[0].len();
    let mut column_scores = vec![0usize; num_columns];

    for record in records.iter().take(100) {
        for (col_idx, field) in record.iter().enumerate() {
            if is_likely_mgrs(field.trim()) {
                column_scores[col_idx] += 1;
            }
        }
    }

    column_scores
        .iter()
        .enumerate()
        .max_by_key(|&(_, score)| score)
        .filter(|&(_, score)| *score > 0)
        .map(|(idx, _)| idx)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib detect -- --nocapture`
Expected: All 6 tests PASS

**Step 5: Commit**

```bash
git add src/detect.rs src/lib.rs
git commit -m "feat: extract detect module with OnceLock regex caching"
```

---

### Task 4: Create OutputFormat trait and CSV format with TDD

**Files:**
- Create: `src/formats/mod.rs`
- Create: `src/formats/csv_format.rs`
- Modify: `src/lib.rs`

**Step 1: Write failing tests for CSV output**

Create `src/formats/mod.rs`:
```rust
pub mod csv_format;
pub mod geojson;
pub mod kml;
pub mod gpx;

use anyhow::Result;

/// Represents a single row of converted data.
pub struct ConvertedRow {
    /// Original CSV fields.
    pub fields: Vec<String>,
    /// Header names from the original CSV.
    pub headers: Vec<String>,
    /// Converted latitude (None if conversion failed).
    pub latitude: Option<f64>,
    /// Converted longitude (None if conversion failed).
    pub longitude: Option<f64>,
    /// Original MGRS coordinate string.
    pub mgrs_source: Option<String>,
}

/// Trait for output format writers.
pub trait OutputFormat {
    fn write_header(&mut self, headers: &[String]) -> Result<()>;
    fn write_row(&mut self, row: &ConvertedRow) -> Result<()>;
    fn finish(&mut self) -> Result<()>;
}
```

Create `src/formats/csv_format.rs` with tests only:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_csv_output_writes_headers_with_latlon() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
            writer.write_header(&[
                "Name".to_string(),
                "MGRS".to_string(),
            ]).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Name"));
        assert!(output.contains("MGRS"));
        assert!(output.contains("Latitude"));
        assert!(output.contains("Longitude"));
    }

    #[test]
    fn test_csv_output_writes_row_with_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
            writer.write_header(&[
                "Name".to_string(),
                "MGRS".to_string(),
            ]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["DC".to_string(), "18SUJ2337006519".to_string()],
                headers: vec!["Name".to_string(), "MGRS".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: Some("18SUJ2337006519".to_string()),
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("38.8977"));
        assert!(output.contains("-77.0365"));
    }

    #[test]
    fn test_csv_output_writes_empty_on_failed_conversion() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["Place".to_string()],
                headers: vec!["Name".to_string()],
                latitude: None,
                longitude: None,
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2); // header + 1 row
        // Row should end with two empty fields
        assert!(lines[1].ends_with(",,"));
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod convert;
pub mod detect;
pub mod formats;
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib formats::csv_format`
Expected: FAIL — `CsvOutput` not defined

**Step 3: Implement CsvOutput**

Add to top of `src/formats/csv_format.rs`:
```rust
use std::io::Write;
use anyhow::Result;
use crate::formats::{ConvertedRow, OutputFormat};

pub struct CsvOutput<W: Write> {
    writer: csv::Writer<W>,
}

impl<W: Write> CsvOutput<W> {
    pub fn new(output: W) -> Self {
        Self {
            writer: csv::Writer::from_writer(output),
        }
    }
}

impl<W: Write> OutputFormat for CsvOutput<W> {
    fn write_header(&mut self, headers: &[String]) -> Result<()> {
        let mut row: Vec<&str> = headers.iter().map(|h| h.as_str()).collect();
        row.push("Latitude");
        row.push("Longitude");
        self.writer.write_record(&row)?;
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let mut record: Vec<String> = row.fields.clone();
        record.push(row.latitude.map(|l| l.to_string()).unwrap_or_default());
        record.push(row.longitude.map(|l| l.to_string()).unwrap_or_default());
        self.writer.write_record(&record)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib formats::csv_format -- --nocapture`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add src/formats/
git commit -m "feat: add OutputFormat trait and CSV format writer"
```

---

### Task 5: GeoJSON format with TDD

**Files:**
- Create: `src/formats/geojson.rs`

**Step 1: Write failing tests**

Create `src/formats/geojson.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_geojson_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer.write_header(&["Name".to_string(), "MGRS".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["DC".to_string(), "18SUJ2337006519".to_string()],
                headers: vec!["Name".to_string(), "MGRS".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: Some("18SUJ2337006519".to_string()),
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["type"], "FeatureCollection");
        assert_eq!(json["features"].as_array().unwrap().len(), 1);
        assert_eq!(json["features"][0]["type"], "Feature");
        assert_eq!(json["features"][0]["geometry"]["type"], "Point");
        // GeoJSON uses [lon, lat] order
        assert_eq!(json["features"][0]["geometry"]["coordinates"][0], -77.0365);
        assert_eq!(json["features"][0]["geometry"]["coordinates"][1], 38.8977);
        assert_eq!(json["features"][0]["properties"]["Name"], "DC");
    }

    #[test]
    fn test_geojson_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["NoCoords".to_string()],
                headers: vec!["Name".to_string()],
                latitude: None,
                longitude: None,
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["features"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_geojson_multiple_features() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            for i in 0..3 {
                writer.write_row(&ConvertedRow {
                    fields: vec![format!("Place{}", i)],
                    headers: vec!["Name".to_string()],
                    latitude: Some(38.0 + i as f64),
                    longitude: Some(-77.0 + i as f64),
                    mgrs_source: None,
                }).unwrap();
            }
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["features"].as_array().unwrap().len(), 3);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib formats::geojson`
Expected: FAIL — `GeoJsonOutput` not defined

**Step 3: Implement GeoJsonOutput**

Add to top of `src/formats/geojson.rs`:
```rust
use std::io::Write;
use anyhow::Result;
use serde_json::{json, Value};
use crate::formats::{ConvertedRow, OutputFormat};

pub struct GeoJsonOutput<W: Write> {
    output: W,
    features: Vec<Value>,
}

impl<W: Write> GeoJsonOutput<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            features: Vec::new(),
        }
    }
}

impl<W: Write> OutputFormat for GeoJsonOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(()) // GeoJSON doesn't need header pre-processing
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()), // Skip rows without coordinates
        };

        let mut properties = serde_json::Map::new();
        for (header, field) in row.headers.iter().zip(row.fields.iter()) {
            properties.insert(header.clone(), Value::String(field.clone()));
        }

        let feature = json!({
            "type": "Feature",
            "geometry": {
                "type": "Point",
                "coordinates": [lon, lat]
            },
            "properties": properties
        });

        self.features.push(feature);
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        let collection = json!({
            "type": "FeatureCollection",
            "features": self.features
        });
        serde_json::to_writer_pretty(&mut self.output, &collection)?;
        self.output.flush()?;
        Ok(())
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib formats::geojson -- --nocapture`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add src/formats/geojson.rs
git commit -m "feat: add GeoJSON output format"
```

---

### Task 6: KML format with TDD

**Files:**
- Create: `src/formats/kml.rs`

**Step 1: Write failing tests**

Create `src/formats/kml.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_kml_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string(), "MGRS".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["White House".to_string(), "18SUJ2337006519".to_string()],
                headers: vec!["Name".to_string(), "MGRS".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: Some("18SUJ2337006519".to_string()),
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<?xml"));
        assert!(output.contains("<kml"));
        assert!(output.contains("<Placemark>"));
        assert!(output.contains("<name>White House</name>"));
        assert!(output.contains("-77.0365,38.8977"));
    }

    #[test]
    fn test_kml_uses_name_column() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, Some("Location".to_string()));
            writer.write_header(&["ID".to_string(), "Location".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["1".to_string(), "My Place".to_string()],
                headers: vec!["ID".to_string(), "Location".to_string()],
                latitude: Some(38.0),
                longitude: Some(-77.0),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<name>My Place</name>"));
    }

    #[test]
    fn test_kml_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["NoCoords".to_string()],
                headers: vec!["Name".to_string()],
                latitude: None,
                longitude: None,
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("<Placemark>"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib formats::kml`
Expected: FAIL — `KmlOutput` not defined

**Step 3: Implement KmlOutput**

Add to top of `src/formats/kml.rs`:
```rust
use std::io::Write;
use anyhow::Result;
use crate::formats::{ConvertedRow, OutputFormat};

pub struct KmlOutput<W: Write> {
    output: W,
    name_column: Option<String>,
    placemarks: Vec<String>,
    headers: Vec<String>,
}

impl<W: Write> KmlOutput<W> {
    pub fn new(output: W, name_column: Option<String>) -> Self {
        Self {
            output,
            name_column,
            placemarks: Vec::new(),
            headers: Vec::new(),
        }
    }

    fn get_name(&self, row: &ConvertedRow) -> String {
        if let Some(ref name_col) = self.name_column {
            for (header, field) in row.headers.iter().zip(row.fields.iter()) {
                if header == name_col {
                    return escape_xml(field);
                }
            }
        }
        // Default: use first non-numeric field
        for field in &row.fields {
            if !field.trim().is_empty() && field.parse::<f64>().is_err() {
                return escape_xml(field);
            }
        }
        String::from("Unnamed")
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl<W: Write> OutputFormat for KmlOutput<W> {
    fn write_header(&mut self, headers: &[String]) -> Result<()> {
        self.headers = headers.to_vec();
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let name = self.get_name(row);
        let mut extended_data = String::new();
        for (header, field) in row.headers.iter().zip(row.fields.iter()) {
            extended_data.push_str(&format!(
                "        <Data name=\"{}\"><value>{}</value></Data>\n",
                escape_xml(header),
                escape_xml(field)
            ));
        }

        self.placemarks.push(format!(
            "    <Placemark>\n      <name>{}</name>\n      <ExtendedData>\n{}\
            </ExtendedData>\n      <Point>\n        <coordinates>{},{},0</coordinates>\n\
            </Point>\n    </Placemark>",
            name, extended_data, lon, lat
        ));
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        write!(
            self.output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n{}\n  </Document>\n</kml>\n",
            self.placemarks.join("\n")
        )?;
        self.output.flush()?;
        Ok(())
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib formats::kml -- --nocapture`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add src/formats/kml.rs
git commit -m "feat: add KML output format"
```

---

### Task 7: GPX format with TDD

**Files:**
- Create: `src/formats/gpx.rs`

**Step 1: Write failing tests**

Create `src/formats/gpx.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_gpx_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["White House".to_string()],
                headers: vec!["Name".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<?xml"));
        assert!(output.contains("<gpx"));
        assert!(output.contains("<wpt"));
        assert!(output.contains("lat=\"38.8977\""));
        assert!(output.contains("lon=\"-77.0365\""));
        assert!(output.contains("<name>White House</name>"));
    }

    #[test]
    fn test_gpx_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["NoCoords".to_string()],
                headers: vec!["Name".to_string()],
                latitude: None,
                longitude: None,
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("<wpt"));
    }

    #[test]
    fn test_gpx_uses_name_column() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, Some("Site".to_string()));
            writer.write_header(&["ID".to_string(), "Site".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["1".to_string(), "Alpha".to_string()],
                headers: vec!["ID".to_string(), "Site".to_string()],
                latitude: Some(51.0),
                longitude: Some(-0.1),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<name>Alpha</name>"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib formats::gpx`
Expected: FAIL — `GpxOutput` not defined

**Step 3: Implement GpxOutput**

Add to top of `src/formats/gpx.rs`:
```rust
use std::io::Write;
use anyhow::Result;
use crate::formats::{ConvertedRow, OutputFormat};
use crate::formats::kml::escape_xml;

pub struct GpxOutput<W: Write> {
    output: W,
    name_column: Option<String>,
    waypoints: Vec<String>,
}

impl<W: Write> GpxOutput<W> {
    pub fn new(output: W, name_column: Option<String>) -> Self {
        Self {
            output,
            name_column,
            waypoints: Vec::new(),
        }
    }

    fn get_name(&self, row: &ConvertedRow) -> String {
        if let Some(ref name_col) = self.name_column {
            for (header, field) in row.headers.iter().zip(row.fields.iter()) {
                if header == name_col {
                    return escape_xml(field);
                }
            }
        }
        for field in &row.fields {
            if !field.trim().is_empty() && field.parse::<f64>().is_err() {
                return escape_xml(field);
            }
        }
        String::from("Unnamed")
    }
}

impl<W: Write> OutputFormat for GpxOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let name = self.get_name(row);
        self.waypoints.push(format!(
            "  <wpt lat=\"{}\" lon=\"{}\">\n    <name>{}</name>\n  </wpt>",
            lat, lon, name
        ));
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        write!(
            self.output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <gpx version=\"1.1\" creator=\"mgrs2latlong\"\n     \
            xmlns=\"http://www.topografix.com/GPX/1/1\">\n{}\n</gpx>\n",
            self.waypoints.join("\n")
        )?;
        self.output.flush()?;
        Ok(())
    }
}
```

Note: `escape_xml` in `src/formats/kml.rs` needs to be made `pub`:
```rust
pub fn escape_xml(s: &str) -> String {
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib formats::gpx -- --nocapture`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add src/formats/gpx.rs src/formats/kml.rs
git commit -m "feat: add GPX output format"
```

---

## Phase 2: Stream Processor & CLI

### Task 8: Stream processor with TDD

**Files:**
- Create: `src/stream.rs`
- Modify: `src/lib.rs`
- Create: `tests/fixtures/sample.csv`

**Step 1: Create test fixture**

Create `tests/fixtures/sample.csv`:
```csv
Name,MGRS,Notes
White House,18SUJ2337006519,DC landmark
Tower Bridge,30UXC9983606474,London landmark
Invalid,NOTMGRS,Bad data
```

**Step 2: Write failing tests**

Create `src/stream.rs` with tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::csv_format::CsvOutput;

    fn sample_csv() -> &'static str {
        "Name,MGRS,Notes\nWhite House,18SUJ2337006519,DC landmark\nInvalid,NOTMGRS,Bad data\n"
    }

    #[test]
    fn test_stream_processor_csv_output() {
        let input = std::io::Cursor::new(sample_csv());
        let mut output = Vec::new();
        let config = ProcessConfig {
            column: None,
            strict: false,
            name_column: None,
        };
        let stats = process_to_latlon(input, &mut output, FormatKind::Csv, &config).unwrap();
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Latitude"));
        assert!(result.contains("Longitude"));
        assert!(result.contains("White House"));
        assert_eq!(stats.total_rows, 2);
        assert_eq!(stats.failed_rows, 1); // "NOTMGRS" fails
    }

    #[test]
    fn test_stream_processor_auto_detects_column() {
        let input = std::io::Cursor::new(sample_csv());
        let mut output = Vec::new();
        let config = ProcessConfig {
            column: None,
            strict: false,
            name_column: None,
        };
        let stats = process_to_latlon(input, &mut output, FormatKind::Csv, &config).unwrap();
        assert!(stats.succeeded_rows >= 1);
    }

    #[test]
    fn test_stream_processor_explicit_column() {
        let input = std::io::Cursor::new(sample_csv());
        let mut output = Vec::new();
        let config = ProcessConfig {
            column: Some(ColumnSpec::Name("MGRS".to_string())),
            strict: false,
            name_column: None,
        };
        let stats = process_to_latlon(input, &mut output, FormatKind::Csv, &config).unwrap();
        assert!(stats.total_rows > 0);
    }

    #[test]
    fn test_stream_processor_strict_mode_fails_on_error() {
        let input = std::io::Cursor::new(sample_csv());
        let mut output = Vec::new();
        let config = ProcessConfig {
            column: None,
            strict: true,
            name_column: None,
        };
        let result = process_to_latlon(input, &mut output, FormatKind::Csv, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_processor_geojson_output() {
        let input = std::io::Cursor::new(sample_csv());
        let mut output = Vec::new();
        let config = ProcessConfig {
            column: None,
            strict: false,
            name_column: None,
        };
        let _stats = process_to_latlon(input, &mut output, FormatKind::GeoJson, &config).unwrap();
        let result = String::from_utf8(output).unwrap();
        let json: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(json["type"], "FeatureCollection");
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod convert;
pub mod detect;
pub mod formats;
pub mod stream;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib stream`
Expected: FAIL — `process_to_latlon`, `ProcessConfig`, etc. not defined

**Step 4: Implement stream processor**

Add to top of `src/stream.rs`:
```rust
use std::io::{Read, Write, BufReader, Seek, SeekFrom};
use anyhow::{Context, Result};
use crate::convert;
use crate::detect;
use crate::formats::{self, ConvertedRow, OutputFormat};
use crate::formats::csv_format::CsvOutput;
use crate::formats::geojson::GeoJsonOutput;
use crate::formats::kml::KmlOutput;
use crate::formats::gpx::GpxOutput;

/// Output format selection.
#[derive(Debug, Clone, Copy)]
pub enum FormatKind {
    Csv,
    GeoJson,
    Kml,
    Gpx,
}

/// How to identify the source column.
#[derive(Debug, Clone)]
pub enum ColumnSpec {
    Name(String),
    Index(usize),
}

/// Configuration for stream processing.
pub struct ProcessConfig {
    pub column: Option<ColumnSpec>,
    pub strict: bool,
    pub name_column: Option<String>,
}

/// Statistics from processing.
pub struct ProcessStats {
    pub total_rows: usize,
    pub succeeded_rows: usize,
    pub failed_rows: usize,
}

/// Process a CSV input, converting MGRS to lat/lon, writing to the specified format.
pub fn process_to_latlon<R: Read, W: Write>(
    input: R,
    output: W,
    format: FormatKind,
    config: &ProcessConfig,
) -> Result<ProcessStats> {
    let mut reader = csv::Reader::from_reader(BufReader::new(input));
    let headers: Vec<String> = reader.headers()?.iter().map(|h| h.to_string()).collect();

    // Determine the MGRS column
    let mgrs_col = match &config.column {
        Some(ColumnSpec::Name(name)) => {
            headers.iter().position(|h| h == name)
                .with_context(|| format!("Column '{}' not found in headers", name))?
        }
        Some(ColumnSpec::Index(idx)) => *idx,
        None => {
            // Read first 100 rows for detection
            let mut sample_records = Vec::new();
            for result in reader.records() {
                let record = result?;
                sample_records.push(record);
                if sample_records.len() >= 100 {
                    break;
                }
            }
            let col = detect::detect_mgrs_column(&sample_records)
                .with_context(|| "No MGRS-like column detected in the CSV file")?;

            // We consumed records for detection; process them, then continue with remaining
            let mut writer = create_writer(output, format, &config.name_column)?;
            writer.write_header(&headers)?;

            let mut stats = ProcessStats {
                total_rows: 0,
                succeeded_rows: 0,
                failed_rows: 0,
            };

            // Process the sample records
            for record in &sample_records {
                process_record(record, &headers, col, &mut *writer, &mut stats, config.strict)?;
            }

            // Process remaining records
            for result in reader.records() {
                let record = result?;
                process_record(&record, &headers, col, &mut *writer, &mut stats, config.strict)?;
            }

            writer.finish()?;
            return Ok(stats);
        }
    };

    let mut writer = create_writer(output, format, &config.name_column)?;
    writer.write_header(&headers)?;

    let mut stats = ProcessStats {
        total_rows: 0,
        succeeded_rows: 0,
        failed_rows: 0,
    };

    for result in reader.records() {
        let record = result?;
        process_record(&record, &headers, mgrs_col, &mut *writer, &mut stats, config.strict)?;
    }

    writer.finish()?;
    Ok(stats)
}

fn create_writer<'a, W: Write + 'a>(
    output: W,
    format: FormatKind,
    name_column: &Option<String>,
) -> Result<Box<dyn OutputFormat + 'a>> {
    Ok(match format {
        FormatKind::Csv => Box::new(CsvOutput::new(output)),
        FormatKind::GeoJson => Box::new(GeoJsonOutput::new(output)),
        FormatKind::Kml => Box::new(KmlOutput::new(output, name_column.clone())),
        FormatKind::Gpx => Box::new(GpxOutput::new(output, name_column.clone())),
    })
}

fn process_record(
    record: &csv::StringRecord,
    headers: &[String],
    mgrs_col: usize,
    writer: &mut dyn OutputFormat,
    stats: &mut ProcessStats,
    strict: bool,
) -> Result<()> {
    stats.total_rows += 1;
    let mgrs_value = record.get(mgrs_col).unwrap_or("").trim();

    let (lat, lon, mgrs_src) = if !mgrs_value.is_empty() && detect::is_likely_mgrs(mgrs_value) {
        match convert::mgrs_to_latlon(mgrs_value) {
            Ok(coord) => (Some(coord.latitude), Some(coord.longitude), Some(mgrs_value.to_string())),
            Err(e) => {
                stats.failed_rows += 1;
                eprintln!("Warning: row {}: failed to convert '{}': {}", stats.total_rows, mgrs_value, e);
                if strict {
                    return Err(e.context(format!("Strict mode: aborting at row {}", stats.total_rows)));
                }
                (None, None, Some(mgrs_value.to_string()))
            }
        }
    } else {
        stats.failed_rows += 1;
        if !mgrs_value.is_empty() {
            eprintln!("Warning: row {}: '{}' does not look like MGRS", stats.total_rows, mgrs_value);
        }
        if strict && !mgrs_value.is_empty() {
            anyhow::bail!("Strict mode: non-MGRS value '{}' at row {}", mgrs_value, stats.total_rows);
        }
        (None, None, None)
    };

    if lat.is_some() {
        stats.succeeded_rows += 1;
    }

    let fields: Vec<String> = record.iter().map(|f| f.to_string()).collect();
    writer.write_row(&ConvertedRow {
        fields,
        headers: headers.to_vec(),
        latitude: lat,
        longitude: lon,
        mgrs_source: mgrs_src,
    })?;

    Ok(())
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --lib stream -- --nocapture`
Expected: All 5 tests PASS

**Step 6: Commit**

```bash
git add src/stream.rs src/lib.rs
git commit -m "feat: add stream processor with format dispatch and strict mode"
```

---

### Task 9: Rewrite main.rs CLI with subcommands

**Files:**
- Modify: `src/main.rs`
- Create: `tests/fixtures/sample.csv`

**Step 1: Create test fixture file**

Create `tests/fixtures/sample.csv`:
```csv
Name,MGRS,Notes
White House,18SUJ2337006519,DC landmark
Tower Bridge,30UXC9983606474,London landmark
Invalid,NOTMGRS,Bad data
```

**Step 2: Write integration test**

Create `tests/cli_integration.rs`:
```rust
use std::process::Command;

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mgrs2latlong"))
}

#[test]
fn test_to_latlon_csv_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latitude"));
    assert!(stdout.contains("Longitude"));
    assert!(stdout.contains("White House"));
}

#[test]
fn test_to_latlon_geojson_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--format", "geojson"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["type"], "FeatureCollection");
}

#[test]
fn test_backward_compat_no_subcommand() {
    // Running without subcommand should default to to-latlon
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latitude"));
}

#[test]
fn test_explicit_column_flag() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--column", "MGRS"])
        .output()
        .expect("failed to run");
    assert!(output.status.success());
}

#[test]
fn test_strict_mode_exits_nonzero() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--strict"])
        .output()
        .expect("failed to run");
    // sample.csv has invalid data, so strict should fail
    assert!(!output.status.success());
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --test cli_integration`
Expected: FAIL — current main.rs doesn't have subcommands

**Step 4: Rewrite main.rs**

Replace `src/main.rs` entirely:
```rust
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{self, BufWriter};
use std::process;
use mgrs2latlong::stream::{self, FormatKind, ColumnSpec, ProcessConfig};

#[derive(Parser)]
#[command(name = "mgrs2latlong")]
#[command(about = "Convert between MGRS coordinates and latitude/longitude in CSV files")]
#[command(author = "Albert Hui <albert@securityronin.com>")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input CSV file path (for backward compatibility without subcommand)
    #[arg(global = false)]
    input: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert MGRS coordinates to latitude/longitude
    ToLatlon(ConvertArgs),
    /// Convert latitude/longitude to MGRS coordinates
    ToMgrs(ConvertArgs),
}

#[derive(Parser)]
struct ConvertArgs {
    /// Input CSV file path
    input: String,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Output format
    #[arg(short, long, default_value = "csv")]
    format: String,

    /// Column name or index containing coordinates
    #[arg(short, long)]
    column: Option<String>,

    /// Decimal places in output coordinates
    #[arg(short, long, default_value = "6")]
    precision: u8,

    /// Abort on first conversion error
    #[arg(long)]
    strict: bool,

    /// Column to use as placemark/waypoint name (KML/GPX)
    #[arg(long)]
    name_column: Option<String>,
}

fn parse_format(s: &str) -> Result<FormatKind> {
    match s.to_lowercase().as_str() {
        "csv" => Ok(FormatKind::Csv),
        "geojson" => Ok(FormatKind::GeoJson),
        "kml" => Ok(FormatKind::Kml),
        "gpx" => Ok(FormatKind::Gpx),
        _ => anyhow::bail!("Unknown format '{}'. Supported: csv, geojson, kml, gpx", s),
    }
}

fn parse_column(s: &str) -> ColumnSpec {
    match s.parse::<usize>() {
        Ok(idx) => ColumnSpec::Index(idx),
        Err(_) => ColumnSpec::Name(s.to_string()),
    }
}

fn run_to_latlon(args: &ConvertArgs) -> Result<()> {
    let format = parse_format(&args.format)?;
    let input = File::open(&args.input)
        .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", args.input, e))?;

    let output: Box<dyn io::Write> = match &args.output {
        Some(path) => Box::new(BufWriter::new(File::create(path)?)),
        None => Box::new(io::stdout()),
    };

    let config = ProcessConfig {
        column: args.column.as_deref().map(parse_column),
        strict: args.strict,
        name_column: args.name_column.clone(),
    };

    let stats = stream::process_to_latlon(input, output, format, &config)?;

    eprintln!(
        "Processed {} rows: {} succeeded, {} failed.",
        stats.total_rows, stats.succeeded_rows, stats.failed_rows
    );

    if stats.failed_rows > 0 && stats.succeeded_rows > 0 {
        process::exit(2);
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::ToLatlon(args)) => run_to_latlon(&args),
        Some(Commands::ToMgrs(_args)) => {
            // TODO: implement to-mgrs in next phase
            anyhow::bail!("to-mgrs is not yet implemented")
        }
        None => {
            // Backward compatibility: treat positional arg as to-latlon
            match cli.input {
                Some(input) => run_to_latlon(&ConvertArgs {
                    input,
                    output: None,
                    format: "csv".to_string(),
                    column: None,
                    precision: 6,
                    strict: false,
                    name_column: None,
                }),
                None => {
                    anyhow::bail!("No input file specified. Usage: mgrs2latlong <INPUT> or mgrs2latlong to-latlon <INPUT>")
                }
            }
        }
    }
}
```

**Step 5: Run all tests**

Run: `cargo test`
Expected: All unit tests and integration tests PASS

**Step 6: Commit**

```bash
git add src/main.rs tests/
git commit -m "feat: rewrite CLI with subcommands, backward compat, and all output formats"
```

---

## Phase 3: Polish

### Task 10: Add to-mgrs (reverse conversion) with TDD

**Files:**
- Modify: `src/stream.rs`
- Modify: `src/main.rs`

**Step 1: Write failing test for reverse conversion**

Add to `src/stream.rs` tests:
```rust
#[test]
fn test_process_to_mgrs() {
    let csv_data = "Name,Latitude,Longitude\nDC,38.8977,-77.0365\n";
    let input = std::io::Cursor::new(csv_data);
    let mut output = Vec::new();
    let config = ProcessConfig {
        column: None,
        strict: false,
        name_column: None,
    };
    let stats = process_to_mgrs(input, &mut output, FormatKind::Csv, &config, 5).unwrap();
    let result = String::from_utf8(output).unwrap();
    assert!(result.contains("MGRS"));
    assert!(result.contains("18S"));
    assert_eq!(stats.total_rows, 1);
    assert_eq!(stats.succeeded_rows, 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib stream::tests::test_process_to_mgrs`
Expected: FAIL — `process_to_mgrs` not defined

**Step 3: Implement process_to_mgrs**

Add `process_to_mgrs` function to `src/stream.rs` and wire up in `main.rs`.

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/stream.rs src/main.rs
git commit -m "feat: add to-mgrs reverse conversion command"
```

---

### Task 11: Add progress bar support

**Files:**
- Modify: `src/main.rs`

**Step 1: Add progress bar when output is a file**

Use `indicatif::ProgressBar` to show conversion progress when writing to a file (not stdout). Count total lines in input file first, then create a progress bar.

This is UI-only, so manual testing is appropriate here. Verify:
- `mgrs2latlong to-latlon tests/fixtures/sample.csv -o /tmp/out.csv` shows progress bar
- `mgrs2latlong to-latlon tests/fixtures/sample.csv` (stdout) shows no progress bar

**Step 2: Commit**

```bash
git add src/main.rs
git commit -m "feat: add progress bar for file output"
```

---

### Task 12: Final integration test and cleanup

**Files:**
- Modify: `tests/cli_integration.rs`
- Modify: `Cargo.toml` (version bump to 0.2.0)

**Step 1: Add integration tests for all formats**

Add KML and GPX integration tests to `tests/cli_integration.rs`.

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

**Step 3: Bump version**

Update `Cargo.toml` version to `"0.2.0"`.

**Step 4: Commit**

```bash
git add .
git commit -m "chore: v0.2.0 with all enhancements"
```

---

## Summary

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| 1: Restructure | Tasks 1-7 | Library crate, convert/detect modules, all 4 output formats |
| 2: Stream & CLI | Tasks 8-9 | Streaming processor, subcommand CLI, backward compat |
| 3: Polish | Tasks 10-12 | Reverse conversion, progress bar, final tests, v0.2.0 |

**Total: 12 tasks, each with explicit RED→GREEN→REFACTOR TDD steps.**
