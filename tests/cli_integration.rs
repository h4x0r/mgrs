use std::process::Command;

fn cargo_bin() -> Command {
    let bin = env!("CARGO_BIN_EXE_mgrs");
    Command::new(bin)
}

#[test]
fn test_csv_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latitude"), "Missing Latitude header in: {}", stdout);
    assert!(stdout.contains("Longitude"), "Missing Longitude header in: {}", stdout);
    assert!(stdout.contains("White House"), "Missing White House row in: {}", stdout);
}

#[test]
fn test_geojson_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "geojson"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(json["type"], "FeatureCollection");
}

#[test]
fn test_kml_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "kml"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("<kml"), "Missing KML root element: {}", stdout);
    assert!(stdout.contains("<Placemark>"), "Missing Placemark: {}", stdout);
    assert!(stdout.contains("White House"), "Missing placemark name: {}", stdout);
}

#[test]
fn test_gpx_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "gpx"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("<gpx"), "Missing GPX root element: {}", stdout);
    assert!(stdout.contains("<wpt"), "Missing waypoint: {}", stdout);
    assert!(stdout.contains("White House"), "Missing waypoint name: {}", stdout);
}

#[test]
fn test_explicit_column_flag() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--column", "MGRS"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_strict_mode_exits_nonzero() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--strict"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success(), "Strict mode should fail with invalid data");
}

#[test]
fn test_to_mgrs_flag() {
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join("mgrs_test_latlon.csv");
    std::fs::write(&input_path, "Name,Latitude,Longitude\nDC,38.8977,-77.0365\n").unwrap();

    let output = cargo_bin()
        .args([input_path.to_str().unwrap(), "--to-mgrs"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("MGRS"), "Missing MGRS header: {}", stdout);
    assert!(stdout.contains("18S"), "Missing MGRS grid zone: {}", stdout);

    std::fs::remove_file(&input_path).ok();
}

#[test]
fn test_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_output.csv");

    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));

    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("Latitude"), "Output file missing Latitude: {}", contents);
    assert!(contents.contains("White House"), "Output file missing data: {}", contents);

    std::fs::remove_file(&output_path).ok();
}

// --- New format output tests ---

#[test]
fn test_wkt_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "wkt"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("POINT("), "Missing POINT in WKT output: {}", stdout);
}

#[test]
fn test_topojson_output() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "topojson"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(json["type"], "Topology");
}

#[test]
fn test_kmz_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_output.kmz");
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "kmz", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let data = std::fs::read(&output_path).unwrap();
    assert!(data.len() > 4, "KMZ file should not be empty");
    std::fs::remove_file(&output_path).ok();
}

#[test]
fn test_flatgeobuf_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_output.fgb");
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "flatgeobuf", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let data = std::fs::read(&output_path).unwrap();
    assert_eq!(&data[0..4], b"fgb\x03", "FlatGeobuf magic bytes");
    std::fs::remove_file(&output_path).ok();
}

#[test]
fn test_shapefile_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_output.shp");
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "shapefile", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    assert!(output_path.exists(), "Shapefile .shp should exist");
    assert!(temp_dir.join("mgrs_test_output.dbf").exists(), ".dbf should exist");
    std::fs::remove_file(&output_path).ok();
    std::fs::remove_file(temp_dir.join("mgrs_test_output.shx")).ok();
    std::fs::remove_file(temp_dir.join("mgrs_test_output.dbf")).ok();
}

#[test]
fn test_geopackage_output_to_file() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_output.gpkg");
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "geopackage", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    assert!(output_path.exists(), "GeoPackage file should exist");
    std::fs::remove_file(&output_path).ok();
}

// --- Format auto-detection tests ---

#[test]
fn test_auto_detect_output_format_from_extension() {
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join("mgrs_test_auto.geojson");
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let contents = std::fs::read_to_string(&output_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&contents).expect("invalid JSON");
    assert_eq!(json["type"], "FeatureCollection");
    std::fs::remove_file(&output_path).ok();
}

// --- Cross-format conversion tests ---

#[test]
fn test_geojson_to_csv() {
    let temp_dir = std::env::temp_dir();
    // First create a GeoJSON file
    let geojson_path = temp_dir.join("mgrs_test_cross.geojson");
    cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "geojson", "-o", geojson_path.to_str().unwrap()])
        .output()
        .expect("failed to run");

    // Now convert GeoJSON to CSV
    let output = cargo_bin()
        .args([geojson_path.to_str().unwrap(), "--format", "csv"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("White House") || stdout.contains("Latitude"),
        "Cross-format output should contain data: {}", stdout);

    std::fs::remove_file(&geojson_path).ok();
}

#[test]
fn test_kml_to_geojson() {
    let temp_dir = std::env::temp_dir();
    // First create a KML file
    let kml_path = temp_dir.join("mgrs_test_cross.kml");
    cargo_bin()
        .args(["tests/fixtures/sample.csv", "--format", "kml", "-o", kml_path.to_str().unwrap()])
        .output()
        .expect("failed to run");

    // Now convert KML to GeoJSON
    let output = cargo_bin()
        .args([kml_path.to_str().unwrap(), "--format", "geojson"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON from KML->GeoJSON");
    assert_eq!(json["type"], "FeatureCollection");

    std::fs::remove_file(&kml_path).ok();
}
