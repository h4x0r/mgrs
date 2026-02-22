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
