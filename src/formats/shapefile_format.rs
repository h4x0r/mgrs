use crate::formats::{ConvertedRow, InputFormat, InputRecord, PathOutputFormat};
use anyhow::Result;
use shapefile::dbase;
use std::path::Path;

/// Truncate a field name to fit in dBase's 11-byte limit.
fn truncate_field_name(name: &str) -> String {
    if name.len() <= 11 {
        name.to_string()
    } else {
        name[..11].to_string()
    }
}

pub struct ShapefileOutput {
    writer: shapefile::Writer<std::io::BufWriter<std::fs::File>>,
    truncated_headers: Vec<String>,
}

impl PathOutputFormat for ShapefileOutput {
    fn new(path: &Path, headers: &[String]) -> Result<Self> {
        let truncated: Vec<String> = headers.iter().map(|h| truncate_field_name(h)).collect();

        let mut builder = dbase::TableWriterBuilder::new();
        for name in &truncated {
            let field_name: dbase::FieldName = name
                .as_str()
                .try_into()
                .map_err(|e: &str| anyhow::anyhow!("{}", e))?;
            builder = builder.add_character_field(field_name, 254);
        }

        let writer = shapefile::Writer::from_path(path, builder)
            .map_err(|e| anyhow::anyhow!("Failed to create shapefile: {}", e))?;

        Ok(Self {
            writer,
            truncated_headers: truncated,
        })
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let point = shapefile::Point::new(lon, lat);
        let mut record = dbase::Record::default();
        for (header, field) in self.truncated_headers.iter().zip(row.fields.iter()) {
            record.insert(
                header.clone(),
                dbase::FieldValue::Character(Some(field.clone())),
            );
        }

        self.writer
            .write_shape_and_record(&point, &record)
            .map_err(|e| anyhow::anyhow!("Failed to write shape: {}", e))?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct ShapefileInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl ShapefileInput {
    pub fn new(path: &Path) -> Result<Self> {
        let mut reader = shapefile::Reader::from_path(path)
            .map_err(|e| anyhow::anyhow!("Failed to open shapefile: {}", e))?;

        let mut headers = Vec::new();
        let mut records = Vec::new();

        for result in reader.iter_shapes_and_records() {
            let (shape, record) =
                result.map_err(|e| anyhow::anyhow!("Failed to read shapefile record: {}", e))?;

            // Extract point coordinates
            let (lat, lon) = match shape {
                shapefile::Shape::Point(p) => (Some(p.y), Some(p.x)),
                _ => (None, None),
            };

            // Extract fields
            let mut fields = Vec::new();
            for (name, value) in record.into_iter() {
                if !headers.contains(&name) {
                    headers.push(name.clone());
                }
                let val_str = match value {
                    dbase::FieldValue::Character(Some(s)) => s,
                    dbase::FieldValue::Character(None) => String::new(),
                    other => other.to_string(),
                };
                fields.push((name, val_str));
            }
            records.push(InputRecord {
                fields,
                latitude: lat,
                longitude: lon,
            });
        }

        Ok(Self {
            headers,
            records: records.into_iter(),
        })
    }
}

impl InputFormat for ShapefileInput {
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
    fn test_creates_three_files() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        {
            let mut w = ShapefileOutput::new(&path, &["Name".into()]).unwrap();
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
        assert!(dir.path().join("out.shp").exists());
        assert!(dir.path().join("out.shx").exists());
        assert!(dir.path().join("out.dbf").exists());
    }

    #[test]
    fn test_correct_geometry() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        {
            let mut w = ShapefileOutput::new(&path, &["Name".into()]).unwrap();
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
        let mut reader = shapefile::Reader::from_path(&path).unwrap();
        let records: Vec<_> = reader
            .iter_shapes_and_records()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(records.len(), 1);
        match &records[0].0 {
            shapefile::Shape::Point(p) => {
                assert!((p.x - (-77.0365)).abs() < 0.0001);
                assert!((p.y - 38.8977).abs() < 0.0001);
            }
            _ => panic!("Expected Point shape"),
        }
    }

    #[test]
    fn test_correct_attributes() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        {
            let mut w = ShapefileOutput::new(&path, &["Name".into(), "Code".into()]).unwrap();
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
        let mut reader = shapefile::Reader::from_path(&path).unwrap();
        let records: Vec<_> = reader
            .iter_shapes_and_records()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(records.len(), 1);
        let (_, rec) = &records[0];
        let name = rec.get("Name").unwrap();
        match name {
            dbase::FieldValue::Character(Some(s)) => assert_eq!(s, "DC"),
            other => panic!("Expected Character, got {other:?}"),
        }
    }

    #[test]
    fn test_truncates_long_field_names() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        let long_name = "VeryLongFieldName".to_string(); // > 11 chars
        {
            let mut w = ShapefileOutput::new(&path, &[long_name.clone()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["val".into()],
                headers: vec![long_name],
                latitude: Some(38.0),
                longitude: Some(-77.0),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        // Should not error — long names get truncated
        let mut reader = shapefile::Reader::from_path(&path).unwrap();
        let records: Vec<_> = reader
            .iter_shapes_and_records()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_input_reads_points() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        {
            let mut w = ShapefileOutput::new(&path, &["Name".into()]).unwrap();
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
        let mut r = ShapefileInput::new(&path).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip() {
        let dir = make_tempdir();
        let path = dir.path().join("out.shp");
        {
            let mut w = ShapefileOutput::new(&path, &["Name".into()]).unwrap();
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
        let mut r = ShapefileInput::new(&path).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
        let name = rec.fields.iter().find(|(k, _)| k == "Name").unwrap();
        assert_eq!(name.1, "DC");
        assert!(r.next_record().unwrap().is_none());
    }
}
