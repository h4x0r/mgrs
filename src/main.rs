use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use mgrs::format_detect::detect_format;
use mgrs::stream::{self, ColumnSpec, FormatKind, ProcessConfig};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read};
use std::process;

#[derive(Parser)]
#[command(name = "mgrs")]
#[command(about = "Bidirectional MGRS/lat-long conversion across 10 geospatial formats")]
#[command(author = "Albert Hui <albert@securityronin.com>")]
#[command(version)]
struct Cli {
    /// Input file path
    input: String,

    /// Convert lat/lon to MGRS (default is MGRS to lat/lon)
    #[arg(long)]
    to_mgrs: bool,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Output format: csv, geojson, kml, gpx, wkt, topojson, kmz, shapefile, geopackage, flatgeobuf
    #[arg(short, long)]
    format: Option<String>,

    /// Input format (auto-detected from extension if omitted)
    #[arg(short = 'F', long)]
    input_format: Option<String>,

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
        "wkt" => Ok(FormatKind::Wkt),
        "topojson" => Ok(FormatKind::TopoJson),
        "kmz" => Ok(FormatKind::Kmz),
        "shapefile" | "shp" => Ok(FormatKind::Shapefile),
        "geopackage" | "gpkg" => Ok(FormatKind::GeoPackage),
        "flatgeobuf" | "fgb" => Ok(FormatKind::FlatGeobuf),
        _ => anyhow::bail!(
            "Unknown format '{}'. Supported: csv, geojson, kml, gpx, wkt, topojson, kmz, shapefile, geopackage, flatgeobuf",
            s
        ),
    }
}

fn parse_column(s: &str) -> ColumnSpec {
    match s.parse::<usize>() {
        Ok(idx) => ColumnSpec::Index(idx),
        Err(_) => ColumnSpec::Name(s.to_string()),
    }
}

fn count_lines(path: &str) -> Result<u64> {
    let file = File::open(path)?;
    let count = BufReader::new(file).lines().count();
    Ok(count.saturating_sub(1) as u64)
}

fn create_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} rows ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb
}

fn is_path_based_format(format: FormatKind) -> bool {
    matches!(format, FormatKind::Shapefile | FormatKind::GeoPackage)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine input format
    let in_format = match &cli.input_format {
        Some(f) => parse_format(f)?,
        None => detect_format(&cli.input).unwrap_or(FormatKind::Csv),
    };

    // Determine output format
    let out_format = match &cli.format {
        Some(f) => parse_format(f)?,
        None => match &cli.output {
            Some(path) => detect_format(path).unwrap_or(FormatKind::Csv),
            None => FormatKind::Csv,
        },
    };

    // Validate path-based output formats require -o
    if is_path_based_format(out_format) && cli.output.is_none() {
        anyhow::bail!(
            "Format {:?} requires --output flag (writes to filesystem)",
            out_format
        );
    }

    let config = ProcessConfig {
        column: cli.column.as_deref().map(parse_column),
        strict: cli.strict,
        name_column: cli.name_column.clone(),
    };

    let pb = if cli.output.is_some() && in_format == FormatKind::Csv {
        let total = count_lines(&cli.input).unwrap_or(0);
        if total > 0 {
            Some(create_progress_bar(total))
        } else {
            None
        }
    } else {
        None
    };

    let stats = if cli.to_mgrs {
        // to-mgrs mode: CSV input only, CSV output
        let input = File::open(&cli.input)
            .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", cli.input, e))?;
        let output: Box<dyn io::Write> = match &cli.output {
            Some(path) => Box::new(BufWriter::new(File::create(path)?)),
            None => Box::new(io::stdout()),
        };
        stream::process_to_mgrs(input, output, out_format, &config, cli.precision)?
    } else if in_format == FormatKind::Csv {
        // CSV input → any output format (existing path)
        let input = File::open(&cli.input)
            .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", cli.input, e))?;

        if is_path_based_format(out_format) {
            let output_path = cli.output.as_ref().unwrap(); // validated above
            stream::process_csv_to_path(
                input,
                std::path::Path::new(output_path),
                out_format,
                &config,
            )?
        } else {
            let output: Box<dyn io::Write> = match &cli.output {
                Some(path) => Box::new(BufWriter::new(File::create(path)?)),
                None => Box::new(io::stdout()),
            };
            stream::process_to_latlon(input, output, out_format, &config)?
        }
    } else if is_path_based_format(in_format) {
        // Path-based input (Shapefile, GeoPackage) → any output
        let reader = stream::create_path_reader(std::path::Path::new(&cli.input), in_format)?;

        if is_path_based_format(out_format) {
            let output_path = cli.output.as_ref().unwrap();
            stream::process_format_to_path(reader, std::path::Path::new(output_path), out_format)?
        } else {
            let output: Box<dyn io::Write> = match &cli.output {
                Some(path) => Box::new(BufWriter::new(File::create(path)?)),
                None => Box::new(io::stdout()),
            };
            stream::process_format_to_format(reader, output, out_format, &config)?
        }
    } else {
        // Stream-based non-CSV input → any output
        let mut data = Vec::new();
        File::open(&cli.input)
            .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", cli.input, e))?
            .read_to_end(&mut data)?;

        let reader = stream::create_reader(data, in_format)?;

        if is_path_based_format(out_format) {
            let output_path = cli.output.as_ref().unwrap();
            stream::process_format_to_path(reader, std::path::Path::new(output_path), out_format)?
        } else {
            let output: Box<dyn io::Write> = match &cli.output {
                Some(path) => Box::new(BufWriter::new(File::create(path)?)),
                None => Box::new(io::stdout()),
            };
            stream::process_format_to_format(reader, output, out_format, &config)?
        }
    };

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    eprintln!(
        "Processed {} rows: {} succeeded, {} failed.",
        stats.total_rows, stats.succeeded_rows, stats.failed_rows
    );

    if stats.failed_rows > 0 && stats.succeeded_rows > 0 {
        process::exit(2);
    }

    Ok(())
}
