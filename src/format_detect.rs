use std::path::Path;
use crate::stream::FormatKind;

pub fn detect_format(path: &str) -> Option<FormatKind> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "csv" => Some(FormatKind::Csv),
        "geojson" | "json" => Some(FormatKind::GeoJson),
        "kml" => Some(FormatKind::Kml),
        "kmz" => Some(FormatKind::Kmz),
        "gpx" => Some(FormatKind::Gpx),
        "topojson" => Some(FormatKind::TopoJson),
        "wkt" => Some(FormatKind::Wkt),
        "shp" => Some(FormatKind::Shapefile),
        "gpkg" => Some(FormatKind::GeoPackage),
        "fgb" => Some(FormatKind::FlatGeobuf),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_csv() {
        assert!(matches!(detect_format("data.csv"), Some(FormatKind::Csv)));
    }

    #[test]
    fn test_detect_geojson_extensions() {
        assert!(matches!(detect_format("data.geojson"), Some(FormatKind::GeoJson)));
        assert!(matches!(detect_format("data.json"), Some(FormatKind::GeoJson)));
    }

    #[test]
    fn test_detect_kml() {
        assert!(matches!(detect_format("data.kml"), Some(FormatKind::Kml)));
    }

    #[test]
    fn test_detect_gpx() {
        assert!(matches!(detect_format("data.gpx"), Some(FormatKind::Gpx)));
    }

    #[test]
    fn test_detect_wkt() {
        assert!(matches!(detect_format("data.wkt"), Some(FormatKind::Wkt)));
    }

    #[test]
    fn test_detect_topojson() {
        assert!(matches!(detect_format("data.topojson"), Some(FormatKind::TopoJson)));
    }

    #[test]
    fn test_detect_kmz() {
        assert!(matches!(detect_format("data.kmz"), Some(FormatKind::Kmz)));
    }

    #[test]
    fn test_detect_shapefile() {
        assert!(matches!(detect_format("data.shp"), Some(FormatKind::Shapefile)));
    }

    #[test]
    fn test_detect_geopackage() {
        assert!(matches!(detect_format("data.gpkg"), Some(FormatKind::GeoPackage)));
    }

    #[test]
    fn test_detect_flatgeobuf() {
        assert!(matches!(detect_format("data.fgb"), Some(FormatKind::FlatGeobuf)));
    }

    #[test]
    fn test_detect_unknown_returns_none() {
        assert!(detect_format("data.xyz").is_none());
    }

    #[test]
    fn test_detect_case_insensitive() {
        assert!(matches!(detect_format("data.GeoJSON"), Some(FormatKind::GeoJson)));
        assert!(matches!(detect_format("data.KML"), Some(FormatKind::Kml)));
    }

    #[test]
    fn test_detect_with_path() {
        assert!(matches!(detect_format("/tmp/out.geojson"), Some(FormatKind::GeoJson)));
    }
}
