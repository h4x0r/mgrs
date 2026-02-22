use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter};
use std::process;
use mgrs::stream::{self, FormatKind, ColumnSpec, ProcessConfig};

#[derive(Parser)]
#[command(name = "mgrs")]
#[command(about = "Convert between MGRS coordinates and latitude/longitude in CSV files")]
#[command(author = "Albert Hui <albert@securityronin.com>")]
#[command(version)]
struct Cli {
    /// Input CSV file path
    input: String,

    /// Convert lat/lon to MGRS (default is MGRS to lat/lon)
    #[arg(long)]
    to_mgrs: bool,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Output format: csv, geojson, kml, gpx
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    let format = parse_format(&cli.format)?;

    let pb = if cli.output.is_some() {
        let total = count_lines(&cli.input).unwrap_or(0);
        if total > 0 {
            Some(create_progress_bar(total))
        } else {
            None
        }
    } else {
        None
    };

    let input = File::open(&cli.input)
        .map_err(|e| anyhow::anyhow!("Failed to open '{}': {}", cli.input, e))?;

    let output: Box<dyn io::Write> = match &cli.output {
        Some(path) => Box::new(BufWriter::new(File::create(path)?)),
        None => Box::new(io::stdout()),
    };

    let config = ProcessConfig {
        column: cli.column.as_deref().map(parse_column),
        strict: cli.strict,
        name_column: cli.name_column.clone(),
    };

    let stats = if cli.to_mgrs {
        stream::process_to_mgrs(input, output, format, &config, cli.precision)?
    } else {
        stream::process_to_latlon(input, output, format, &config)?
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
