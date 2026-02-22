use std::process::Command;

fn cargo_bin() -> Command {
    let bin = env!("CARGO_BIN_EXE_mgrs2latlong");
    Command::new(bin)
}

#[test]
fn test_to_latlon_csv_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2)); // exit 2 = partial failures
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latitude"), "Missing Latitude header in: {}", stdout);
    assert!(stdout.contains("Longitude"), "Missing Longitude header in: {}", stdout);
    assert!(stdout.contains("White House"), "Missing White House row in: {}", stdout);
}

#[test]
fn test_to_latlon_geojson_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--format", "geojson"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(json["type"], "FeatureCollection");
}

#[test]
fn test_backward_compat_no_subcommand() {
    let output = cargo_bin()
        .args(["tests/fixtures/sample.csv"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latitude"), "Backward compat failed: {}", stdout);
}

#[test]
fn test_explicit_column_flag() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--column", "MGRS"])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_strict_mode_exits_nonzero() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--strict"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success(), "Strict mode should fail with invalid data");
}

#[test]
fn test_to_latlon_kml_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--format", "kml"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("<kml"), "Missing KML root element: {}", stdout);
    assert!(stdout.contains("<Placemark>"), "Missing Placemark: {}", stdout);
    assert!(stdout.contains("White House"), "Missing placemark name: {}", stdout);
}

#[test]
fn test_to_latlon_gpx_output() {
    let output = cargo_bin()
        .args(["to-latlon", "tests/fixtures/sample.csv", "--format", "gpx"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("<gpx"), "Missing GPX root element: {}", stdout);
    assert!(stdout.contains("<wpt"), "Missing waypoint: {}", stdout);
    assert!(stdout.contains("White House"), "Missing waypoint name: {}", stdout);
}

#[test]
fn test_to_mgrs_csv_output() {
    // Create a temp CSV with lat/lon data
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join("mgrs_test_latlon.csv");
    std::fs::write(&input_path, "Name,Latitude,Longitude\nDC,38.8977,-77.0365\n").unwrap();

    let output = cargo_bin()
        .args(["to-mgrs", input_path.to_str().unwrap()])
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
        .args(["to-latlon", "tests/fixtures/sample.csv", "-o", output_path.to_str().unwrap()])
        .output()
        .expect("failed to run");
    assert!(output.status.success() || output.status.code() == Some(2));

    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("Latitude"), "Output file missing Latitude: {}", contents);
    assert!(contents.contains("White House"), "Output file missing data: {}", contents);

    std::fs::remove_file(&output_path).ok();
}
