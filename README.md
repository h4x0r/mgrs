# mgrs

Bidirectional MGRS/lat-long coordinate conversion CLI and Rust library. Reads and writes 10 geospatial formats with automatic format detection.

## Author

Albert Hui <albert@securityronin.com>

## Features

- **Bidirectional conversion**: MGRS to lat/lon (default) and lat/lon to MGRS (`--to-mgrs`)
- **10 formats, all bidirectional**: CSV, GeoJSON, KML, KMZ, GPX, WKT, TopoJSON, Shapefile, GeoPackage, FlatGeobuf
- **Cross-format conversion**: Any input format to any output format (e.g., GeoJSON to KML, Shapefile to CSV)
- **Auto-detection**: Detects both input and output formats from file extensions
- **Auto-detect MGRS columns**: Finds MGRS columns automatically, no configuration needed
- **Streaming**: Processes CSV rows one at a time — handles large files without loading everything into memory
- **Strict mode**: Optionally abort on first conversion error
- **Optional heavy deps**: Shapefile, GeoPackage, and FlatGeobuf support can be disabled at compile time
- **Library API**: Use as a Rust library in your own projects

## Installation

### From crates.io

```bash
cargo install mgrs
```

### Without optional formats

```bash
cargo install mgrs --no-default-features
```

This builds without Shapefile, GeoPackage, and FlatGeobuf support, avoiding the heavier native dependencies (SQLite, FlatBuffers).

### Windows binary

Download `mgrs-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/h4x0r/mgrs/releases/latest).

### From source

```bash
git clone https://github.com/h4x0r/mgrs.git
cd mgrs
cargo build --release
```

The binary will be at `target/release/mgrs`.

## Usage

### MGRS to Lat/Lon (default)

```bash
# CSV to stdout
mgrs input.csv

# Output as GeoJSON (format auto-detected from extension)
mgrs input.csv -o output.geojson

# Explicit format flag
mgrs input.csv --format geojson -o output.geojson

# KML with custom name column
mgrs input.csv -f kml --name-column "Site Name" -o output.kml

# GPX waypoints
mgrs input.csv -f gpx -o output.gpx

# WKT points
mgrs input.csv -f wkt -o output.wkt

# Shapefile (requires -o, creates .shp/.shx/.dbf)
mgrs input.csv -f shapefile -o output.shp

# GeoPackage (requires -o)
mgrs input.csv -f geopackage -o output.gpkg

# FlatGeobuf
mgrs input.csv -f flatgeobuf -o output.fgb
```

### Cross-Format Conversion

Any supported input format can be converted to any output format:

```bash
# GeoJSON to CSV
mgrs input.geojson -o output.csv

# KML to GeoJSON
mgrs input.kml -o output.geojson

# Shapefile to KML
mgrs input.shp -o output.kml

# GeoPackage to CSV (stdout)
mgrs input.gpkg -f csv

# Explicit input format override
mgrs data.json -F geojson -o output.csv
```

### Lat/Lon to MGRS

```bash
# Reverse conversion
mgrs input.csv --to-mgrs

# Control MGRS precision (1-5, default 6)
mgrs input.csv --to-mgrs --precision 3
```

### Other Options

```bash
# Specify the MGRS column explicitly
mgrs input.csv --column "Grid Ref"

# Abort on first error
mgrs input.csv --strict
```

## Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--to-mgrs` | | Convert lat/lon to MGRS (default is MGRS to lat/lon) | off |
| `--format` | `-f` | Output format (see table below) | auto-detect from `-o` extension, else `csv` |
| `--input-format` | `-F` | Input format override | auto-detect from input extension, else `csv` |
| `--output` | `-o` | Output file path (omit for stdout) | stdout |
| `--column` | `-c` | Column name or index containing coordinates | auto-detect |
| `--precision` | `-p` | Decimal places in output coordinates | `6` |
| `--strict` | | Abort on first conversion error | off |
| `--name-column` | | Column for placemark/waypoint names (KML/GPX) | first text column |

## Supported Formats

All formats support both reading and writing. Format is auto-detected from file extensions.

| Format | `-f` value | Extension | Notes |
|--------|-----------|-----------|-------|
| **CSV** | `csv` | `.csv` | Comma-separated values with header row |
| **GeoJSON** | `geojson` | `.geojson`, `.json` | RFC 7946 FeatureCollection with Point geometries |
| **KML** | `kml` | `.kml` | Keyhole Markup Language (Google Earth) |
| **KMZ** | `kmz` | `.kmz` | Compressed KML (ZIP archive) |
| **GPX** | `gpx` | `.gpx` | GPS Exchange Format (waypoints) |
| **WKT** | `wkt` | `.wkt` | Well-Known Text (`POINT(lon lat)` per line) |
| **TopoJSON** | `topojson` | `.topojson` | Topology-aware JSON with shared arcs |
| **Shapefile** | `shapefile`, `shp` | `.shp` | ESRI Shapefile (creates .shp/.shx/.dbf); requires `-o` |
| **GeoPackage** | `geopackage`, `gpkg` | `.gpkg` | OGC GeoPackage (SQLite); requires `-o` |
| **FlatGeobuf** | `flatgeobuf`, `fgb` | `.fgb` | FlatBuffers-based binary format with spatial index |

**Note:** Shapefile and GeoPackage write to the filesystem and require the `--output` flag.

## Feature Flags

Heavy native dependencies are optional Cargo features (all enabled by default):

| Feature | Dependency | What it enables |
|---------|-----------|----------------|
| `shapefile-format` | `shapefile` | Shapefile (.shp) read/write |
| `geopackage` | `rusqlite` (bundled SQLite) | GeoPackage (.gpkg) read/write |
| `flatgeobuf-format` | `flatgeobuf` + `geozero` | FlatGeobuf (.fgb) read/write |

To build without these:

```bash
cargo build --release --no-default-features
```

The remaining 7 formats (CSV, GeoJSON, KML, KMZ, GPX, WKT, TopoJSON) are always available.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All rows converted successfully |
| `1` | Fatal error (bad input file, unknown format, etc.) |
| `2` | Partial success — some rows failed conversion |

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mgrs = "0.4"
```

```rust
use mgrs::convert::{mgrs_to_latlon, latlon_to_mgrs};

// MGRS to Lat/Lon
let coord = mgrs_to_latlon("18SUJ2337006519").unwrap();
println!("{}, {}", coord.latitude, coord.longitude);

// Lat/Lon to MGRS
let mgrs = latlon_to_mgrs(38.8977, -77.0365, 5).unwrap();
println!("{}", mgrs.0);
```

## License

Dual-licensed under [MIT](LICENSE) or Apache-2.0, at your option.
