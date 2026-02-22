# mgrs

Bidirectional MGRS/lat-long coordinate conversion CLI and Rust library. Reads CSV files and outputs to CSV, GeoJSON, KML, or GPX.

## Author

Albert Hui <albert@securityronin.com>

## Features

- **Bidirectional conversion**: MGRS to lat/lon (default) and lat/lon to MGRS (`--to-mgrs`)
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

```bash
# Convert MGRS to lat/lon (CSV to stdout)
mgrs input.csv

# Output as GeoJSON
mgrs input.csv --format geojson -o output.geojson

# Output as KML with custom name column
mgrs input.csv --format kml --name-column "Site Name" -o output.kml

# Output as GPX waypoints
mgrs input.csv --format gpx -o output.gpx

# Specify the MGRS column explicitly
mgrs input.csv --column "Grid Ref"

# Abort on first error
mgrs input.csv --strict

# Reverse: convert lat/lon to MGRS
mgrs input.csv --to-mgrs

# Control MGRS precision (1-5, default 6)
mgrs input.csv --to-mgrs --precision 3
```

## Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--to-mgrs` | | Convert lat/lon to MGRS (default is MGRS to lat/lon) | off |
| `--format` | `-f` | Output format: `csv`, `geojson`, `kml`, `gpx` | `csv` |
| `--output` | `-o` | Output file path (omit for stdout) | stdout |
| `--column` | `-c` | Column name or index containing coordinates | auto-detect |
| `--precision` | `-p` | Decimal places in output coordinates | `6` |
| `--strict` | | Abort on first conversion error | off |
| `--name-column` | | Column for placemark/waypoint names (KML/GPX) | first text column |

## Input Format

CSV files with either:
- An **MGRS column** (default mode): values like `18SUJ2337006519` or `18S UJ 23370 06519`
- **Latitude/Longitude columns** (`--to-mgrs` mode): columns with names containing "lat" and "lon"/"lng"

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
