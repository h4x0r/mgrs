use std::io::{Read, Write, BufReader};
use anyhow::{Context, Result};
use crate::convert;
use crate::detect;
use crate::formats::{ConvertedRow, OutputFormat};
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
            // Need to read some records for auto-detection
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

            // Process the buffered sample records, then continue with remaining
            let mut writer = create_writer(output, format, &config.name_column)?;
            writer.write_header(&headers)?;

            let mut stats = ProcessStats {
                total_rows: 0,
                succeeded_rows: 0,
                failed_rows: 0,
            };

            for record in &sample_records {
                process_record(record, &headers, col, &mut *writer, &mut stats, config.strict)?;
            }

            for result in reader.records() {
                let record = result?;
                process_record(&record, &headers, col, &mut *writer, &mut stats, config.strict)?;
            }

            writer.finish()?;
            return Ok(stats);
        }
    };

    // Explicit column path -- stream all records
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_stream_processor_explicit_column_by_name() {
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
