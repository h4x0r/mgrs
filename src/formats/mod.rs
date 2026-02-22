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
