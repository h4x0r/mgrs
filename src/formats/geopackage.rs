use crate::formats::{ConvertedRow, InputFormat, InputRecord, PathOutputFormat};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

/// Encode a WGS84 point as GeoPackage Binary (GPB).
/// Format: GP header (8 bytes) + WKB Point (21 bytes).
fn encode_gpb_point(lon: f64, lat: f64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(29);
    // GP magic bytes
    buf.push(b'G');
    buf.push(b'P');
    // Version 0
    buf.push(0);
    // Flags: little-endian, no envelope
    buf.push(0x01);
    // SRS ID (4326) as little-endian i32
    buf.extend_from_slice(&4326i32.to_le_bytes());
    // WKB Point (little-endian)
    buf.push(0x01); // byte order: little-endian
    buf.extend_from_slice(&1u32.to_le_bytes()); // geometry type: Point
    buf.extend_from_slice(&lon.to_le_bytes());
    buf.extend_from_slice(&lat.to_le_bytes());
    buf
}

/// Decode a GeoPackage Binary point into (lat, lon).
fn decode_gpb_point(data: &[u8]) -> Option<(f64, f64)> {
    if data.len() < 29 {
        return None;
    }
    if data[0] != b'G' || data[1] != b'P' {
        return None;
    }
    // Skip 8-byte GP header, read WKB
    let wkb = &data[8..];
    if wkb.len() < 21 {
        return None;
    }
    let is_le = wkb[0] == 0x01;
    let geom_type = if is_le {
        u32::from_le_bytes([wkb[1], wkb[2], wkb[3], wkb[4]])
    } else {
        u32::from_be_bytes([wkb[1], wkb[2], wkb[3], wkb[4]])
    };
    if geom_type != 1 {
        return None;
    } // Not a Point
    let (lon, lat) = if is_le {
        (
            f64::from_le_bytes(wkb[5..13].try_into().ok()?),
            f64::from_le_bytes(wkb[13..21].try_into().ok()?),
        )
    } else {
        (
            f64::from_be_bytes(wkb[5..13].try_into().ok()?),
            f64::from_be_bytes(wkb[13..21].try_into().ok()?),
        )
    };
    Some((lat, lon))
}

pub struct GeoPackageOutput {
    conn: Connection,
    headers: Vec<String>,
}

impl PathOutputFormat for GeoPackageOutput {
    fn new(path: &Path, headers: &[String]) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to create GeoPackage")?;

        conn.execute_batch("
            PRAGMA journal_mode=WAL;

            CREATE TABLE gpkg_spatial_ref_sys (
                srs_name TEXT NOT NULL,
                srs_id INTEGER NOT NULL PRIMARY KEY,
                organization TEXT NOT NULL,
                organization_coordsys_id INTEGER NOT NULL,
                definition TEXT NOT NULL,
                description TEXT
            );

            INSERT INTO gpkg_spatial_ref_sys VALUES
                ('WGS 84', 4326, 'EPSG', 4326,
                 'GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]]',
                 'WGS 84 geographic coordinate system');

            CREATE TABLE gpkg_contents (
                table_name TEXT NOT NULL PRIMARY KEY,
                data_type TEXT NOT NULL,
                identifier TEXT,
                description TEXT DEFAULT '',
                last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                min_x DOUBLE,
                min_y DOUBLE,
                max_x DOUBLE,
                max_y DOUBLE,
                srs_id INTEGER REFERENCES gpkg_spatial_ref_sys(srs_id)
            );

            CREATE TABLE gpkg_geometry_columns (
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                geometry_type_name TEXT NOT NULL,
                srs_id INTEGER NOT NULL,
                z TINYINT NOT NULL,
                m TINYINT NOT NULL,
                CONSTRAINT pk_gc PRIMARY KEY (table_name, column_name)
            );
        ")?;

        // Build the points table with a geometry column and attribute columns
        let mut col_defs = vec!["fid INTEGER PRIMARY KEY AUTOINCREMENT".to_string()];
        col_defs.push("geom BLOB".to_string());
        for h in headers {
            col_defs.push(format!("\"{h}\" TEXT"));
        }
        let create_sql = format!("CREATE TABLE points ({})", col_defs.join(", "));
        conn.execute(&create_sql, [])?;

        // Register in gpkg metadata tables
        conn.execute(
            "INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) VALUES ('points', 'features', 'points', 4326)",
            [],
        )?;
        conn.execute(
            "INSERT INTO gpkg_geometry_columns (table_name, column_name, geometry_type_name, srs_id, z, m) VALUES ('points', 'geom', 'POINT', 4326, 0, 0)",
            [],
        )?;

        Ok(Self {
            conn,
            headers: headers.to_vec(),
        })
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let geom_blob = encode_gpb_point(lon, lat);
        let placeholders: Vec<String> = (0..self.headers.len()).map(|_| "?".to_string()).collect();
        let col_names: Vec<String> = self.headers.iter().map(|h| format!("\"{h}\"")).collect();
        let sql = format!(
            "INSERT INTO points (geom, {}) VALUES (?{})",
            col_names.join(", "),
            if placeholders.is_empty() {
                String::new()
            } else {
                format!(", {}", placeholders.join(", "))
            }
        );

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params.push(Box::new(geom_blob));
        for field in &row.fields {
            params.push(Box::new(field.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        self.conn.execute(&sql, param_refs.as_slice())?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct GeoPackageInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl GeoPackageInput {
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open GeoPackage")?;

        // Get column names (excluding fid and geom)
        let mut stmt = conn.prepare("PRAGMA table_info(points)")?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .filter(|n| n != "fid" && n != "geom")
            .collect();

        // Read all rows
        let select_cols: Vec<String> = columns.iter().map(|c| format!("\"{c}\"")).collect();
        let sql = format!(
            "SELECT geom, {} FROM points",
            if select_cols.is_empty() {
                "1".to_string()
            } else {
                select_cols.join(", ")
            }
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut records = Vec::new();

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let geom_data: Vec<u8> = row.get(0)?;
            let (lat, lon) = decode_gpb_point(&geom_data)
                .map(|(lat, lon)| (Some(lat), Some(lon)))
                .unwrap_or((None, None));

            let mut fields = Vec::new();
            for (i, col) in columns.iter().enumerate() {
                let val: String = row.get(i + 1).unwrap_or_default();
                fields.push((col.clone(), val));
            }
            records.push(InputRecord {
                fields,
                latitude: lat,
                longitude: lon,
            });
        }

        Ok(Self {
            headers: columns,
            records: records.into_iter(),
        })
    }
}

impl InputFormat for GeoPackageInput {
    fn headers(&self) -> Vec<String> {
        self.headers.clone()
    }
    fn next_record(&mut self) -> Result<Option<InputRecord>> {
        Ok(self.records.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    fn make_tempdir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_valid_sqlite() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()],
                headers: vec!["Name".into()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM points", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_required_tables_exist() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into()]).unwrap();
            w.finish().unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(tables.contains(&"gpkg_spatial_ref_sys".to_string()));
        assert!(tables.contains(&"gpkg_contents".to_string()));
        assert!(tables.contains(&"gpkg_geometry_columns".to_string()));
        assert!(tables.contains(&"points".to_string()));
    }

    #[test]
    fn test_correct_srs() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into()]).unwrap();
            w.finish().unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        let srs_id: i64 = conn
            .query_row(
                "SELECT srs_id FROM gpkg_spatial_ref_sys WHERE srs_name='WGS 84'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(srs_id, 4326);
    }

    #[test]
    fn test_attributes_match() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into(), "Code".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into(), "20001".into()],
                headers: vec!["Name".into(), "Code".into()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        let name: String = conn
            .query_row("SELECT Name FROM points", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "DC");
        let code: String = conn
            .query_row("SELECT Code FROM points", [], |r| r.get(0))
            .unwrap();
        assert_eq!(code, "20001");
    }

    #[test]
    fn test_input_reads_features() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()],
                headers: vec!["Name".into()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        let mut r = GeoPackageInput::new(&path).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip() {
        let dir = make_tempdir();
        let path = dir.path().join("out.gpkg");
        {
            let mut w = GeoPackageOutput::new(&path, &["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()],
                headers: vec!["Name".into()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        let mut r = GeoPackageInput::new(&path).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
        let name = rec.fields.iter().find(|(k, _)| k == "Name").unwrap();
        assert_eq!(name.1, "DC");
        assert!(r.next_record().unwrap().is_none());
    }
}
