use crate::formats::{InputFormat, InputRecord};
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::Read;

pub struct GeoJsonInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl GeoJsonInput {
    pub fn new<R: Read>(mut input: R) -> Result<Self> {
        let mut buf = String::new();
        input.read_to_string(&mut buf)?;
        let json: Value = serde_json::from_str(&buf).context("Failed to parse GeoJSON")?;
        let features = json
            .get("features")
            .and_then(|f| f.as_array())
            .context("GeoJSON missing 'features' array")?;

        // Collect property keys in order
        let mut keys = Vec::new();
        for feat in features {
            if let Some(props) = feat.get("properties").and_then(|p| p.as_object()) {
                for key in props.keys() {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
            }
        }

        let mut records = Vec::new();
        for feat in features {
            let (lat, lon) = extract_point(feat);
            let mut fields = Vec::new();
            if let Some(props) = feat.get("properties").and_then(|p| p.as_object()) {
                for key in &keys {
                    let val = props
                        .get(key)
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Null => String::new(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default();
                    fields.push((key.clone(), val));
                }
            }
            records.push(InputRecord {
                fields,
                latitude: lat,
                longitude: lon,
            });
        }

        Ok(Self {
            headers: keys,
            records: records.into_iter(),
        })
    }
}

fn extract_point(feat: &Value) -> (Option<f64>, Option<f64>) {
    let inner = || -> Option<(f64, f64)> {
        let geom = feat.get("geometry").filter(|g| !g.is_null())?;
        if geom.get("type")?.as_str()? != "Point" {
            return None;
        }
        let c = geom.get("coordinates")?.as_array()?;
        Some((c.get(1)?.as_f64()?, c.first()?.as_f64()?))
    };
    match inner() {
        Some((lat, lon)) => (Some(lat), Some(lon)),
        None => (None, None),
    }
}

impl InputFormat for GeoJsonInput {
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
    use std::io::Cursor;

    fn sample() -> &'static str {
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","geometry":{"type":"Point","coordinates":[-77.0365,38.8977]},
             "properties":{"Name":"White House","MGRS":"18SUJ2337006519"}},
            {"type":"Feature","geometry":{"type":"Point","coordinates":[-0.0754,51.5055]},
             "properties":{"Name":"Tower Bridge","MGRS":"30UXC9983606474"}}
        ]}"#
    }

    #[test]
    fn test_reads_features() {
        let mut r = GeoJsonInput::new(Cursor::new(sample())).unwrap();
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_none());
    }

    #[test]
    fn test_extracts_coordinates() {
        let mut r = GeoJsonInput::new(Cursor::new(sample())).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.0001);
    }

    #[test]
    fn test_extracts_properties() {
        let mut r = GeoJsonInput::new(Cursor::new(sample())).unwrap();
        let hdrs = r.headers();
        assert!(hdrs.contains(&"Name".to_string()));
        let rec = r.next_record().unwrap().unwrap();
        let name = rec.fields.iter().find(|(k, _)| k == "Name").unwrap();
        assert_eq!(name.1, "White House");
    }

    #[test]
    fn test_roundtrip() {
        use crate::formats::geojson::GeoJsonOutput;
        use crate::formats::{ConvertedRow, OutputFormat};
        let mut buf = Vec::new();
        {
            let mut w = GeoJsonOutput::new(&mut buf);
            w.write_header(&["Name".into()]).unwrap();
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
        let mut r = GeoJsonInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
    }
}
