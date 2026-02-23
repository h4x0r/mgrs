pub mod csv_format;
pub mod csv_input;
pub mod geojson;
pub mod kml;
pub mod gpx;
pub mod wkt;
pub mod geojson_input;
pub mod kml_input;
pub mod gpx_input;
pub mod topojson;
pub mod kmz;
#[cfg(feature = "shapefile-format")]
pub mod shapefile_format;
#[cfg(feature = "geopackage")]
pub mod geopackage;
#[cfg(feature = "flatgeobuf-format")]
pub mod flatgeobuf_format;

use std::path::Path;
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

/// A record produced by an input format reader.
pub struct InputRecord {
    pub fields: Vec<(String, String)>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// Trait for input format readers.
pub trait InputFormat {
    fn headers(&self) -> Vec<String>;
    fn next_record(&mut self) -> Result<Option<InputRecord>>;
}

/// Trait for output formats that write to a filesystem path (e.g. Shapefile, GeoPackage).
pub trait PathOutputFormat {
    fn new(path: &Path, headers: &[String]) -> Result<Self> where Self: Sized;
    fn write_row(&mut self, row: &ConvertedRow) -> Result<()>;
    fn finish(&mut self) -> Result<()>;
}
