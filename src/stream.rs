use crate::convert;
use crate::detect;
use crate::formats::csv_format::CsvOutput;
#[cfg(feature = "flatgeobuf-format")]
use crate::formats::flatgeobuf_format::FlatGeobufOutput;
use crate::formats::geojson::GeoJsonOutput;
use crate::formats::gpx::GpxOutput;
use crate::formats::kml::KmlOutput;
use crate::formats::kmz::KmzOutput;
use crate::formats::topojson::TopoJsonOutput;
use crate::formats::wkt::WktOutput;
use crate::formats::{ConvertedRow, InputFormat, OutputFormat, PathOutputFormat};
use anyhow::{Context, Result};
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

/// Format selection (input and output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Csv,
    GeoJson,
    Kml,
    Gpx,
    Wkt,
    TopoJson,
    Kmz,
    Shapefile,
    GeoPackage,
    FlatGeobuf,
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
        Some(ColumnSpec::Name(name)) => headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("Column '{name}' not found in headers"))?,
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
                process_record(
                    record,
                    &headers,
                    col,
                    &mut *writer,
                    &mut stats,
                    config.strict,
                )?;
            }

            for result in reader.records() {
                let record = result?;
                process_record(
                    &record,
                    &headers,
                    col,
                    &mut *writer,
                    &mut stats,
                    config.strict,
                )?;
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
        process_record(
            &record,
            &headers,
            mgrs_col,
            &mut *writer,
            &mut stats,
            config.strict,
        )?;
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
        FormatKind::Wkt => Box::new(WktOutput::new(output)),
        FormatKind::TopoJson => Box::new(TopoJsonOutput::new(output)),
        FormatKind::Kmz => Box::new(KmzOutput::new(output, name_column.clone())),
        #[cfg(feature = "flatgeobuf-format")]
        FormatKind::FlatGeobuf => Box::new(FlatGeobufOutput::new(output)),
        #[cfg(not(feature = "flatgeobuf-format"))]
        FormatKind::FlatGeobuf => {
            anyhow::bail!("FlatGeobuf support not compiled (enable 'flatgeobuf-format' feature)")
        }
        FormatKind::Shapefile | FormatKind::GeoPackage => {
            anyhow::bail!(
                "Format {:?} requires --output flag (path-based format)",
                format
            )
        }
    })
}

/// Create a reader for the given input format.
pub fn create_reader(data: Vec<u8>, format: FormatKind) -> Result<Box<dyn InputFormat>> {
    use crate::formats::csv_input::CsvInput;
    #[cfg(feature = "flatgeobuf-format")]
    use crate::formats::flatgeobuf_format::FlatGeobufInput;
    use crate::formats::geojson_input::GeoJsonInput;
    use crate::formats::gpx_input::GpxInput;
    use crate::formats::kml_input::KmlInput;
    use crate::formats::kmz::KmzInput;
    use crate::formats::topojson::TopoJsonInput;
    use crate::formats::wkt::WktInput;

    Ok(match format {
        FormatKind::Csv => Box::new(CsvInput::new(Cursor::new(data))?),
        FormatKind::GeoJson => Box::new(GeoJsonInput::new(Cursor::new(data))?),
        FormatKind::Kml => Box::new(KmlInput::new(Cursor::new(data))?),
        FormatKind::Gpx => Box::new(GpxInput::new(Cursor::new(data))?),
        FormatKind::Wkt => Box::new(WktInput::new(Cursor::new(data))?),
        FormatKind::TopoJson => Box::new(TopoJsonInput::new(Cursor::new(data))?),
        FormatKind::Kmz => Box::new(KmzInput::new(Cursor::new(data))?),
        #[cfg(feature = "flatgeobuf-format")]
        FormatKind::FlatGeobuf => Box::new(FlatGeobufInput::new(Cursor::new(data))?),
        #[cfg(not(feature = "flatgeobuf-format"))]
        FormatKind::FlatGeobuf => {
            anyhow::bail!("FlatGeobuf support not compiled (enable 'flatgeobuf-format' feature)")
        }
        FormatKind::Shapefile => {
            anyhow::bail!(
                "Shapefile input requires a file path, use --input-format with a .shp file"
            )
        }
        FormatKind::GeoPackage => {
            anyhow::bail!(
                "GeoPackage input requires a file path, use --input-format with a .gpkg file"
            )
        }
    })
}

/// Create a path-based reader for Shapefile or GeoPackage.
pub fn create_path_reader(path: &Path, format: FormatKind) -> Result<Box<dyn InputFormat>> {
    match format {
        #[cfg(feature = "shapefile-format")]
        FormatKind::Shapefile => {
            use crate::formats::shapefile_format::ShapefileInput;
            Ok(Box::new(ShapefileInput::new(path)?))
        }
        #[cfg(not(feature = "shapefile-format"))]
        FormatKind::Shapefile => {
            anyhow::bail!("Shapefile support not compiled (enable 'shapefile-format' feature)")
        }
        #[cfg(feature = "geopackage")]
        FormatKind::GeoPackage => {
            use crate::formats::geopackage::GeoPackageInput;
            Ok(Box::new(GeoPackageInput::new(path)?))
        }
        #[cfg(not(feature = "geopackage"))]
        FormatKind::GeoPackage => {
            anyhow::bail!("GeoPackage support not compiled (enable 'geopackage' feature)")
        }
        _ => anyhow::bail!("Format {:?} is not a path-based input format", format),
    }
}

/// Create a path-based writer for Shapefile or GeoPackage.
fn create_path_writer(
    path: &Path,
    format: FormatKind,
    headers: &[String],
) -> Result<Box<dyn PathOutputFormat>> {
    match format {
        #[cfg(feature = "shapefile-format")]
        FormatKind::Shapefile => {
            use crate::formats::shapefile_format::ShapefileOutput;
            Ok(Box::new(ShapefileOutput::new(path, headers)?))
        }
        #[cfg(not(feature = "shapefile-format"))]
        FormatKind::Shapefile => {
            anyhow::bail!("Shapefile support not compiled (enable 'shapefile-format' feature)")
        }
        #[cfg(feature = "geopackage")]
        FormatKind::GeoPackage => {
            use crate::formats::geopackage::GeoPackageOutput;
            Ok(Box::new(GeoPackageOutput::new(path, headers)?))
        }
        #[cfg(not(feature = "geopackage"))]
        FormatKind::GeoPackage => {
            anyhow::bail!("GeoPackage support not compiled (enable 'geopackage' feature)")
        }
        _ => anyhow::bail!("Format {:?} is not a path-based output format", format),
    }
}

/// Process input from any supported format, converting to any output format.
/// Used when input is a non-CSV format.
pub fn process_format_to_format<W: Write>(
    mut reader: Box<dyn InputFormat>,
    output: W,
    out_format: FormatKind,
    config: &ProcessConfig,
) -> Result<ProcessStats> {
    let headers = reader.headers();
    let mut writer = create_writer(output, out_format, &config.name_column)?;
    writer.write_header(&headers)?;

    let mut stats = ProcessStats {
        total_rows: 0,
        succeeded_rows: 0,
        failed_rows: 0,
    };

    while let Some(record) = reader.next_record()? {
        stats.total_rows += 1;
        let fields: Vec<String> = headers
            .iter()
            .map(|h| {
                record
                    .fields
                    .iter()
                    .find(|(k, _)| k == h)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            })
            .collect();

        if record.latitude.is_some() && record.longitude.is_some() {
            stats.succeeded_rows += 1;
        } else {
            stats.failed_rows += 1;
        }

        writer.write_row(&ConvertedRow {
            fields,
            headers: headers.clone(),
            latitude: record.latitude,
            longitude: record.longitude,
            mgrs_source: None,
        })?;
    }

    writer.finish()?;
    Ok(stats)
}

/// Process input from any format to a path-based output (Shapefile, GeoPackage).
pub fn process_format_to_path(
    mut reader: Box<dyn InputFormat>,
    output_path: &Path,
    out_format: FormatKind,
) -> Result<ProcessStats> {
    let headers = reader.headers();

    let mut writer: Box<dyn PathOutputFormat> =
        create_path_writer(output_path, out_format, &headers)?;

    let mut stats = ProcessStats {
        total_rows: 0,
        succeeded_rows: 0,
        failed_rows: 0,
    };

    while let Some(record) = reader.next_record()? {
        stats.total_rows += 1;
        let fields: Vec<String> = headers
            .iter()
            .map(|h| {
                record
                    .fields
                    .iter()
                    .find(|(k, _)| k == h)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            })
            .collect();

        if record.latitude.is_some() && record.longitude.is_some() {
            stats.succeeded_rows += 1;
        } else {
            stats.failed_rows += 1;
        }

        writer.write_row(&ConvertedRow {
            fields,
            headers: headers.clone(),
            latitude: record.latitude,
            longitude: record.longitude,
            mgrs_source: None,
        })?;
    }

    writer.finish()?;
    Ok(stats)
}

/// Process CSV to a path-based output format (Shapefile, GeoPackage).
pub fn process_csv_to_path<R: Read>(
    input: R,
    output_path: &Path,
    out_format: FormatKind,
    config: &ProcessConfig,
) -> Result<ProcessStats> {
    let mut csv_reader = csv::Reader::from_reader(BufReader::new(input));
    let headers: Vec<String> = csv_reader
        .headers()?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mgrs_col = match &config.column {
        Some(ColumnSpec::Name(name)) => headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("Column '{name}' not found in headers"))?,
        Some(ColumnSpec::Index(idx)) => *idx,
        None => {
            let mut sample_records = Vec::new();
            for result in csv_reader.records() {
                let record = result?;
                sample_records.push(record);
                if sample_records.len() >= 100 {
                    break;
                }
            }
            let col = detect::detect_mgrs_column(&sample_records)
                .with_context(|| "No MGRS-like column detected")?;

            let mut writer = create_path_writer(output_path, out_format, &headers)?;

            let mut stats = ProcessStats {
                total_rows: 0,
                succeeded_rows: 0,
                failed_rows: 0,
            };
            for record in &sample_records {
                process_record_path(
                    record,
                    &headers,
                    col,
                    &mut *writer,
                    &mut stats,
                    config.strict,
                )?;
            }
            for result in csv_reader.records() {
                let record = result?;
                process_record_path(
                    &record,
                    &headers,
                    col,
                    &mut *writer,
                    &mut stats,
                    config.strict,
                )?;
            }
            writer.finish()?;
            return Ok(stats);
        }
    };

    let mut writer = create_path_writer(output_path, out_format, &headers)?;

    let mut stats = ProcessStats {
        total_rows: 0,
        succeeded_rows: 0,
        failed_rows: 0,
    };
    for result in csv_reader.records() {
        let record = result?;
        process_record_path(
            &record,
            &headers,
            mgrs_col,
            &mut *writer,
            &mut stats,
            config.strict,
        )?;
    }
    writer.finish()?;
    Ok(stats)
}

fn process_record_path(
    record: &csv::StringRecord,
    headers: &[String],
    mgrs_col: usize,
    writer: &mut dyn PathOutputFormat,
    stats: &mut ProcessStats,
    strict: bool,
) -> Result<()> {
    stats.total_rows += 1;
    let mgrs_value = record.get(mgrs_col).unwrap_or("").trim();

    let (lat, lon, mgrs_src) = if !mgrs_value.is_empty() && detect::is_likely_mgrs(mgrs_value) {
        match convert::mgrs_to_latlon(mgrs_value) {
            Ok(coord) => (
                Some(coord.latitude),
                Some(coord.longitude),
                Some(mgrs_value.to_string()),
            ),
            Err(e) => {
                stats.failed_rows += 1;
                eprintln!(
                    "Warning: row {}: failed to convert '{}': {}",
                    stats.total_rows, mgrs_value, e
                );
                if strict {
                    return Err(
                        e.context(format!("Strict mode: aborting at row {}", stats.total_rows))
                    );
                }
                (None, None, Some(mgrs_value.to_string()))
            }
        }
    } else {
        stats.failed_rows += 1;
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
            Ok(coord) => (
                Some(coord.latitude),
                Some(coord.longitude),
                Some(mgrs_value.to_string()),
            ),
            Err(e) => {
                stats.failed_rows += 1;
                eprintln!(
                    "Warning: row {}: failed to convert '{}': {}",
                    stats.total_rows, mgrs_value, e
                );
                if strict {
                    return Err(
                        e.context(format!("Strict mode: aborting at row {}", stats.total_rows))
                    );
                }
                (None, None, Some(mgrs_value.to_string()))
            }
        }
    } else {
        stats.failed_rows += 1;
        if !mgrs_value.is_empty() {
            eprintln!(
                "Warning: row {}: '{}' does not look like MGRS",
                stats.total_rows, mgrs_value
            );
        }
        if strict && !mgrs_value.is_empty() {
            anyhow::bail!(
                "Strict mode: non-MGRS value '{}' at row {}",
                mgrs_value,
                stats.total_rows
            );
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

/// Process a CSV input, converting lat/lon columns to MGRS, writing CSV output.
/// The input CSV must have columns that look like latitude and longitude values.
pub fn process_to_mgrs<R: Read, W: Write>(
    input: R,
    output: W,
    _format: FormatKind, // Currently only CSV output for to-mgrs
    config: &ProcessConfig,
    precision: u8,
) -> Result<ProcessStats> {
    let mut reader = csv::Reader::from_reader(BufReader::new(input));
    let headers: Vec<String> = reader.headers()?.iter().map(|h| h.to_string()).collect();

    // Find lat/lon columns
    let (lat_col, lon_col) = match &config.column {
        Some(ColumnSpec::Name(name)) => {
            // If user specifies a column, use it as lat; find lon nearby
            let lat_idx = headers
                .iter()
                .position(|h| h.to_lowercase().contains(&name.to_lowercase()))
                .with_context(|| format!("Column '{name}' not found"))?;
            let lon_idx = headers
                .iter()
                .position(|h| {
                    let lower = h.to_lowercase();
                    lower.contains("lon") || lower.contains("lng")
                })
                .with_context(|| "Could not find a longitude column")?;
            (lat_idx, lon_idx)
        }
        Some(ColumnSpec::Index(idx)) => {
            // Assume idx is lat, idx+1 is lon
            (*idx, idx + 1)
        }
        None => {
            // Auto-detect: find columns named like "lat" and "lon"/"lng"
            let lat_idx = headers
                .iter()
                .position(|h| {
                    let lower = h.to_lowercase();
                    lower.contains("lat")
                })
                .with_context(|| "Could not find a latitude column. Use --column to specify.")?;

            let lon_idx = headers
                .iter()
                .position(|h| {
                    let lower = h.to_lowercase();
                    lower.contains("lon") || lower.contains("lng")
                })
                .with_context(|| "Could not find a longitude column.")?;

            (lat_idx, lon_idx)
        }
    };

    // Write output as CSV with MGRS column appended
    let mut csv_writer = csv::Writer::from_writer(output);

    let mut out_headers: Vec<&str> = headers.iter().map(|h| h.as_str()).collect();
    out_headers.push("MGRS");
    csv_writer.write_record(&out_headers)?;

    let mut stats = ProcessStats {
        total_rows: 0,
        succeeded_rows: 0,
        failed_rows: 0,
    };

    for result in reader.records() {
        let record = result?;
        stats.total_rows += 1;

        let lat_str = record.get(lat_col).unwrap_or("").trim();
        let lon_str = record.get(lon_col).unwrap_or("").trim();

        let mgrs_value = match (lat_str.parse::<f64>(), lon_str.parse::<f64>()) {
            (Ok(lat), Ok(lon)) => match convert::latlon_to_mgrs(lat, lon, precision) {
                Ok(mgrs) => {
                    stats.succeeded_rows += 1;
                    mgrs.0
                }
                Err(e) => {
                    stats.failed_rows += 1;
                    eprintln!(
                        "Warning: row {}: failed to convert ({}, {}): {}",
                        stats.total_rows, lat_str, lon_str, e
                    );
                    if config.strict {
                        return Err(e.context(format!(
                            "Strict mode: aborting at row {}",
                            stats.total_rows
                        )));
                    }
                    String::new()
                }
            },
            _ => {
                stats.failed_rows += 1;
                if !lat_str.is_empty() || !lon_str.is_empty() {
                    eprintln!(
                        "Warning: row {}: invalid lat/lon values '{}', '{}'",
                        stats.total_rows, lat_str, lon_str
                    );
                }
                if config.strict {
                    anyhow::bail!(
                        "Strict mode: invalid coordinates at row {}",
                        stats.total_rows
                    );
                }
                String::new()
            }
        };

        let mut out_record: Vec<String> = record.iter().map(|f| f.to_string()).collect();
        out_record.push(mgrs_value);
        csv_writer.write_record(&out_record)?;
    }

    csv_writer.flush()?;
    Ok(stats)
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
        assert!(
            result.contains("MGRS"),
            "Output should have MGRS header: {result}"
        );
        assert!(
            result.contains("18S"),
            "Output should contain MGRS grid zone: {result}"
        );
        assert_eq!(stats.total_rows, 1);
        assert_eq!(stats.succeeded_rows, 1);
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
