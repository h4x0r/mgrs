use anyhow::{Context, Result};
use geoconvert::{LatLon, Mgrs};

/// A latitude/longitude coordinate pair.
#[derive(Debug, Clone, Copy)]
pub struct Coordinate {
    pub latitude: f64,
    pub longitude: f64,
}

/// An MGRS coordinate string.
#[derive(Debug, Clone)]
pub struct MgrsCoord(pub String);

/// Convert an MGRS string to latitude/longitude.
pub fn mgrs_to_latlon(mgrs_str: &str) -> Result<Coordinate> {
    let normalized = mgrs_str.replace(" ", "");
    if normalized.is_empty() {
        anyhow::bail!("Empty MGRS string");
    }
    let mgrs = Mgrs::parse_str(&normalized)
        .with_context(|| format!("Failed to parse MGRS coordinate: {}", mgrs_str))?;
    let latlon = mgrs.to_latlon();
    Ok(Coordinate {
        latitude: latlon.latitude(),
        longitude: latlon.longitude(),
    })
}

/// Convert latitude/longitude to an MGRS string with given precision (1-5).
pub fn latlon_to_mgrs(lat: f64, lon: f64, precision: u8) -> Result<MgrsCoord> {
    let latlon = LatLon::create(lat, lon)
        .with_context(|| format!("Failed to create LatLon from ({}, {})", lat, lon))?;
    let mgrs = latlon.to_mgrs(precision as i32);
    Ok(MgrsCoord(mgrs.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mgrs_to_latlon_known_coordinate() {
        // 18SUJ2337006519 is near Washington DC
        let result = mgrs_to_latlon("18SUJ2337006519").unwrap();
        assert!((result.latitude - 38.8977).abs() < 0.01);
        assert!((result.longitude - (-77.0365)).abs() < 0.01);
    }

    #[test]
    fn test_mgrs_to_latlon_with_spaces() {
        let result = mgrs_to_latlon("18S UJ 23370 06519").unwrap();
        assert!((result.latitude - 38.8977).abs() < 0.01);
    }

    #[test]
    fn test_mgrs_to_latlon_invalid() {
        let result = mgrs_to_latlon("INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_mgrs_to_latlon_empty() {
        let result = mgrs_to_latlon("");
        assert!(result.is_err());
    }

    #[test]
    fn test_latlon_to_mgrs_known_coordinate() {
        // Washington DC
        let result = latlon_to_mgrs(38.8977, -77.0365, 5).unwrap();
        assert!(result.0.starts_with("18SUJ"));
    }

    #[test]
    fn test_latlon_to_mgrs_precision_3() {
        let result = latlon_to_mgrs(38.8977, -77.0365, 3).unwrap();
        // 3-digit precision = 6 digits in easting+northing
        let digits: String = result.0.chars().filter(|c| c.is_ascii_digit()).collect();
        // Grid zone (2 digits) + 6 easting/northing digits = 8 total
        assert!(digits.len() == 8, "Expected 8 digits, got {}: {}", digits.len(), result.0);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let original_lat = 51.5074;
        let original_lon = -0.1278;
        let mgrs = latlon_to_mgrs(original_lat, original_lon, 5).unwrap();
        let back = mgrs_to_latlon(&mgrs.0).unwrap();
        assert!((back.latitude - original_lat).abs() < 0.001);
        assert!((back.longitude - original_lon).abs() < 0.001);
    }
}
