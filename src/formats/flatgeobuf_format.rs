use std::io::{Read, Write};
use anyhow::Result;
use flatgeobuf::{FgbWriter, FgbReader, FallibleStreamingIterator, FeatureProperties, GeozeroGeometry};
use flatgeobuf::{ColumnType, GeometryType};
use geozero::{GeomProcessor, PropertyProcessor, ColumnValue};
use geozero::geojson::GeoJson;
use crate::formats::{ConvertedRow, OutputFormat, InputFormat, InputRecord};

pub struct FlatGeobufOutput<W: Write> {
    output: W,
    fgb: Option<FgbWriter<'static>>,
    headers: Vec<String>,
}

impl<W: Write> FlatGeobufOutput<W> {
    pub fn new(output: W) -> Self {
        Self { output, fgb: None, headers: Vec::new() }
    }
}

impl<W: Write> OutputFormat for FlatGeobufOutput<W> {
    fn write_header(&mut self, headers: &[String]) -> Result<()> {
        self.headers = headers.to_vec();
        let mut fgb = FgbWriter::create("points", GeometryType::Point)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        for h in headers {
            fgb.add_column(h, ColumnType::String, |_, _| {});
        }
        self.fgb = Some(fgb);
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let fgb = self.fgb.as_mut().ok_or_else(|| anyhow::anyhow!("write_header not called"))?;
        let geom_json = format!(r#"{{"type": "Point", "coordinates": [{}, {}]}}"#, lon, lat);
        let geom = GeoJson(&geom_json);
        let fields: Vec<(usize, String)> = row.fields.iter().enumerate()
            .map(|(i, f)| (i, f.clone()))
            .collect();
        fgb.add_feature_geom(geom, |feat| {
            for (i, val) in &fields {
                let _ = feat.property(*i, "", &ColumnValue::String(val));
            }
        }).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        if let Some(fgb) = self.fgb.take() {
            fgb.write(&mut self.output).map_err(|e| anyhow::anyhow!("{}", e))?;
            self.output.flush()?;
        }
        Ok(())
    }
}

/// Helper to collect coordinates from a geozero geometry
struct PointCollector {
    x: Option<f64>,
    y: Option<f64>,
}

impl PointCollector {
    fn new() -> Self { Self { x: None, y: None } }
}

impl GeomProcessor for PointCollector {
    fn xy(&mut self, x: f64, y: f64, _idx: usize) -> geozero::error::Result<()> {
        self.x = Some(x);
        self.y = Some(y);
        Ok(())
    }
}

/// Helper to collect properties from a feature
struct PropCollector {
    props: Vec<(String, String)>,
    col_names: Vec<String>,
}

impl PropCollector {
    fn new(col_names: Vec<String>) -> Self { Self { props: Vec::new(), col_names } }
}

impl PropertyProcessor for PropCollector {
    fn property(&mut self, idx: usize, _colname: &str, colval: &ColumnValue) -> geozero::error::Result<bool> {
        let name = self.col_names.get(idx).cloned().unwrap_or_default();
        let val = colval.to_string();
        self.props.push((name, val));
        Ok(false)
    }
}

pub struct FlatGeobufInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl FlatGeobufInput {
    pub fn new<R: Read + std::io::Seek>(input: R) -> Result<Self> {
        let reader = FgbReader::open(input)
            .map_err(|e| anyhow::anyhow!("Failed to open FlatGeobuf: {}", e))?;

        // Extract column names from header
        let header = reader.header();
        let col_names: Vec<String> = if let Some(columns) = header.columns() {
            (0..columns.len()).map(|i| columns.get(i).name().to_string()).collect()
        } else {
            Vec::new()
        };

        let mut iter = reader.select_all()
            .map_err(|e| anyhow::anyhow!("Failed to select features: {}", e))?;

        let mut records = Vec::new();
        while let Some(feature) = iter.next()
            .map_err(|e| anyhow::anyhow!("Failed to read feature: {}", e))?
        {
            // Extract geometry
            let mut gc = PointCollector::new();
            feature.process_geom(&mut gc).map_err(|e| anyhow::anyhow!("{}", e))?;
            let (lat, lon) = match (gc.y, gc.x) {
                (Some(lat), Some(lon)) => (Some(lat), Some(lon)),
                _ => (None, None),
            };

            // Extract properties
            let mut pc = PropCollector::new(col_names.clone());
            feature.process_properties(&mut pc).map_err(|e| anyhow::anyhow!("{}", e))?;

            records.push(InputRecord { fields: pc.props, latitude: lat, longitude: lon });
        }

        Ok(Self { headers: col_names, records: records.into_iter() })
    }
}

impl InputFormat for FlatGeobufInput {
    fn headers(&self) -> Vec<String> { self.headers.clone() }
    fn next_record(&mut self) -> Result<Option<InputRecord>> { Ok(self.records.next()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{ConvertedRow, OutputFormat, InputFormat};
    use std::io::Cursor;

    fn write_sample() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = FlatGeobufOutput::new(&mut buf);
            w.write_header(&["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()], headers: vec!["Name".into()],
                latitude: Some(38.8977), longitude: Some(-77.0365), mgrs_source: None,
            }).unwrap();
            w.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_valid_header() {
        let buf = write_sample();
        // FlatGeobuf magic bytes: 0x6667620366676200
        assert!(buf.len() > 8);
        assert_eq!(&buf[0..4], b"fgb\x03");
    }

    #[test]
    fn test_correct_geometry() {
        let buf = write_sample();
        let mut r = FlatGeobufInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.0001);
    }

    #[test]
    fn test_correct_attributes() {
        let buf = write_sample();
        let mut r = FlatGeobufInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        let name = rec.fields.iter().find(|(k,_)| k == "Name").unwrap();
        assert_eq!(name.1, "DC");
    }

    #[test]
    fn test_input_reads_features() {
        let buf = write_sample();
        let mut r = FlatGeobufInput::new(Cursor::new(&buf)).unwrap();
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_none());
    }

    #[test]
    fn test_roundtrip() {
        let mut buf = Vec::new();
        {
            let mut w = FlatGeobufOutput::new(&mut buf);
            w.write_header(&["Name".into(), "Code".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into(), "20001".into()],
                headers: vec!["Name".into(), "Code".into()],
                latitude: Some(38.8977), longitude: Some(-77.0365), mgrs_source: None,
            }).unwrap();
            w.finish().unwrap();
        }
        let mut r = FlatGeobufInput::new(Cursor::new(&buf)).unwrap();
        let hdrs = r.headers();
        assert!(hdrs.contains(&"Name".to_string()));
        assert!(hdrs.contains(&"Code".to_string()));
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
        let name = rec.fields.iter().find(|(k,_)| k == "Name").unwrap();
        assert_eq!(name.1, "DC");
        let code = rec.fields.iter().find(|(k,_)| k == "Code").unwrap();
        assert_eq!(code.1, "20001");
        assert!(r.next_record().unwrap().is_none());
    }
}
