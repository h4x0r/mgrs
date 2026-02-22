# mgrs2latlong Enhancement Design

**Date:** 2026-02-22
**Status:** Approved

## Context

mgrs2latlong is a Rust CLI that converts MGRS coordinates to latitude/longitude in CSV files. It auto-detects the MGRS column, appends Latitude/Longitude columns, and writes to file or stdout.

**Primary use case:** GIS/mapping workflows. File sizes vary widely.

**Naming convention:** "latlong" in user-facing names (matches crate name), "latlon" in internal code (matches geoconvert API).

## Architecture: Modular Monolith

Restructure from single-file binary into library crate + binary crate in the same package.

```
src/
  lib.rs          -- public API re-exports
  main.rs         -- CLI binary (thin wrapper)
  convert.rs      -- MGRS<->LatLon bidirectional conversion
  detect.rs       -- column detection heuristics
  formats/
    mod.rs        -- OutputFormat trait
    csv.rs        -- CSV reader/writer (streaming)
    geojson.rs    -- GeoJSON FeatureCollection output
    kml.rs        -- KML Placemark output
    gpx.rs        -- GPX waypoint output
  stream.rs       -- StreamProcessor tying input + conversion + output
```

## CLI Interface

Subcommand-based, backward-compatible:

```
mgrs2latlong to-latlon input.csv -o output.csv --format csv
mgrs2latlong to-mgrs input.csv -o output.geojson --format geojson
mgrs2latlong to-latlon input.csv --column "MGRS Grid" --format kml
```

### Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--format` / `-f` | Output format: csv, geojson, kml, gpx | csv |
| `--output` / `-o` | Output file path (omit for stdout) | stdout |
| `--column` / `-c` | MGRS/latlon column name or index | auto-detect |
| `--precision` / `-p` | Decimal places in output coordinates | 6 |
| `--strict` | Abort on first conversion error | false |
| `--name-column` | Column to use as placemark/waypoint name (KML/GPX) | first text column |

### Backward Compatibility

Running `mgrs2latlong input.csv` without a subcommand defaults to `to-latlon` behavior.

## Output Formats

### CSV (enhanced)
- Streaming row-by-row instead of buffering all records
- Appends Latitude/Longitude columns (to-latlon) or MGRS column (to-mgrs)

### GeoJSON
- `FeatureCollection` with `Point` features
- All CSV columns become feature `properties`
- Original MGRS coordinate preserved as a property

### KML
- `<Placemark>` elements per row
- `<name>` from `--name-column` or first text column
- `<ExtendedData>` carries all CSV fields

### GPX
- `<wpt>` elements per row
- Waypoint name from `--name-column` or first text column

## Streaming & Performance

- Read/write row-by-row (streaming)
- Column detection: read first 100 rows, seek back (or buffer for stdin)
- `--column` flag bypasses detection entirely
- Compile MGRS regex once with `OnceLock`
- Progress bar via `indicatif` when output is a file (not stdout)

## Error Handling

- Conversion failures: warn to stderr with row number and MGRS value, write empty values, continue
- `--strict` flag: abort on first error
- Exit codes: 0 = success, 1 = total failure, 2 = partial failures

## Library API

```rust
pub mod convert;
pub mod detect;
pub mod formats;
pub mod stream;

pub struct Coordinate { pub latitude: f64, pub longitude: f64 }
pub struct MgrsCoord(pub String);

pub fn mgrs_to_latlon(mgrs: &str) -> Result<Coordinate>;
pub fn latlon_to_mgrs(lat: f64, lon: f64, precision: u8) -> Result<MgrsCoord>;
```

## New Dependencies

| Crate | Purpose |
|-------|---------|
| `indicatif` | Progress bars |
| `serde_json` | GeoJSON output |
| `quick-xml` | KML and GPX output |

## Testing

- **Unit tests**: `convert` module (known MGRS <-> latlon pairs), `detect` module (edge cases)
- **Integration tests**: each output format with small test CSVs
- **Test fixtures**: `tests/fixtures/` directory with sample CSVs
- **Snapshot/comparison tests**: validate GeoJSON/KML/GPX structure
