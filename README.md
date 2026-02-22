# mgrs

Bidirectional MGRS/lat-long coordinate conversion CLI and Rust library. Reads CSV files and outputs to CSV, GeoJSON, KML, or GPX.

## Author

Albert Hui <albert@securityronin.com>

## Features

- **Bidirectional conversion**: MGRS to lat/lon (`to-latlon`) and lat/lon to MGRS (`to-mgrs`)
- **Multiple output formats**: CSV, GeoJSON, KML, GPX
- **Auto-detection**: Finds MGRS columns automatically, no configuration needed
- **Streaming**: Processes rows one at a time — handles large files without loading everything into memory
- **Strict mode**: Optionally abort on first conversion error
- **Library API**: Use as a Rust library in your own projects

## Installation

### From crates.io

```bash
cargo install mgrs
```

### From source

```bash
git clone https://github.com/4n6h4x0r/mgrs.git
cd mgrs
cargo build --release
```

The binary will be at `target/release/mgrs`.

## Usage

### MGRS to Lat/Lon

```bash
# Output CSV to stdout
mgrs to-latlon input.csv

# Output GeoJSON to a file
mgrs to-latlon input.csv --format geojson -o output.geojson

# Output KML with custom name column
mgrs to-latlon input.csv --format kml --name-column "Site Name" -o output.kml

# Output GPX waypoints
mgrs to-latlon input.csv --format gpx -o output.gpx

# Specify the MGRS column explicitly
mgrs to-latlon input.csv --column "Grid Ref"

# Abort on first error
mgrs to-latlon input.csv --strict
```

### Lat/Lon to MGRS

```bash
# Append MGRS column to CSV
mgrs to-mgrs input.csv

# Control MGRS precision (1-5, default 6)
mgrs to-mgrs input.csv --precision 3
```

### Backward Compatibility

Running without a subcommand defaults to `to-latlon`:

```bash
mgrs input.csv
```

## Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--format` | `-f` | Output format: `csv`, `geojson`, `kml`, `gpx` | `csv` |
| `--output` | `-o` | Output file path (omit for stdout) | stdout |
| `--column` | `-c` | Column name or index containing coordinates | auto-detect |
| `--precision` | `-p` | Decimal places in output coordinates | `6` |
| `--strict` | | Abort on first conversion error | off |
| `--name-column` | | Column for placemark/waypoint names (KML/GPX) | first text column |

## Input Format

CSV files with either:
- An **MGRS column** (for `to-latlon`): values like `18SUJ2337006519` or `18S UJ 23370 06519`
- **Latitude/Longitude columns** (for `to-mgrs`): columns with names containing "lat" and "lon"/"lng"

## Output Formats

| Format | Description |
|--------|-------------|
| **CSV** | Original columns plus appended Latitude/Longitude (or MGRS) columns |
| **GeoJSON** | `FeatureCollection` with `Point` features; all CSV columns become properties |
| **KML** | `Placemark` elements with `ExtendedData` carrying all CSV fields |
| **GPX** | `wpt` (waypoint) elements with names from `--name-column` or first text column |

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
mgrs = "0.2"
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
