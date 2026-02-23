use crate::formats::{ConvertedRow, InputFormat, InputRecord, OutputFormat};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};

pub struct TopoJsonOutput<W: Write> {
    output: W,
    geometries: Vec<Value>,
}

impl<W: Write> TopoJsonOutput<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            geometries: Vec::new(),
        }
    }
}

impl<W: Write> OutputFormat for TopoJsonOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        if let (Some(lat), Some(lon)) = (row.latitude, row.longitude) {
            let mut props = serde_json::Map::new();
            for (h, f) in row.headers.iter().zip(row.fields.iter()) {
                props.insert(h.clone(), Value::String(f.clone()));
            }
            self.geometries.push(json!({
                "type": "Point", "coordinates": [lon, lat], "properties": props
            }));
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        let topo = json!({
            "type": "Topology",
            "objects": { "points": { "type": "GeometryCollection", "geometries": self.geometries } },
            "arcs": []
        });
        serde_json::to_writer_pretty(&mut self.output, &topo)?;
        self.output.flush()?;
        Ok(())
    }
}

pub struct TopoJsonInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl TopoJsonInput {
    pub fn new<R: Read>(mut input: R) -> Result<Self> {
        let mut buf = String::new();
        input.read_to_string(&mut buf)?;
        let json: Value = serde_json::from_str(&buf).context("Failed to parse TopoJSON")?;
        if json.get("type").and_then(|t| t.as_str()) != Some("Topology") {
            anyhow::bail!("Not a Topology");
        }
        let objects = json
            .get("objects")
            .and_then(|o| o.as_object())
            .context("Missing objects")?;

        let mut all_geoms = Vec::new();
        for (_, obj) in objects {
            if let Some(gs) = obj.get("geometries").and_then(|g| g.as_array()) {
                all_geoms.extend(gs.iter());
            }
        }

        let mut keys = Vec::new();
        for g in &all_geoms {
            if let Some(props) = g.get("properties").and_then(|p| p.as_object()) {
                for k in props.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
        }

        let mut records = Vec::new();
        for g in &all_geoms {
            let (lat, lon) = extract_point(g);
            let mut fields = Vec::new();
            if let Some(props) = g.get("properties").and_then(|p| p.as_object()) {
                for k in &keys {
                    let v = props
                        .get(k)
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Null => String::new(),
                            o => o.to_string(),
                        })
                        .unwrap_or_default();
                    fields.push((k.clone(), v));
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

fn extract_point(g: &Value) -> (Option<f64>, Option<f64>) {
    let inner = || -> Option<(f64, f64)> {
        if g.get("type")?.as_str()? != "Point" {
            return None;
        }
        let c = g.get("coordinates")?.as_array()?;
        Some((c.get(1)?.as_f64()?, c.first()?.as_f64()?))
    };
    match inner() {
        Some((lat, lon)) => (Some(lat), Some(lon)),
        None => (None, None),
    }
}

impl InputFormat for TopoJsonInput {
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

    #[test]
    fn test_valid_structure() {
        let mut buf = Vec::new();
        let mut w = TopoJsonOutput::new(&mut buf);
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
        let json: Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(json["type"], "Topology");
        assert!(json["objects"].is_object());
        assert_eq!(json["arcs"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_point_geometry() {
        let mut buf = Vec::new();
        let mut w = TopoJsonOutput::new(&mut buf);
        w.write_header(&[]).unwrap();
        w.write_row(&ConvertedRow {
            fields: vec![],
            headers: vec![],
            latitude: Some(38.8977),
            longitude: Some(-77.0365),
            mgrs_source: None,
        })
        .unwrap();
        w.finish().unwrap();
        let json: Value = serde_json::from_slice(&buf).unwrap();
        let geom = &json["objects"]["points"]["geometries"][0];
        assert_eq!(geom["type"], "Point");
        assert_eq!(geom["coordinates"][0], -77.0365);
        assert_eq!(geom["coordinates"][1], 38.8977);
    }

    #[test]
    fn test_properties() {
        let mut buf = Vec::new();
        let mut w = TopoJsonOutput::new(&mut buf);
        w.write_header(&["Name".into()]).unwrap();
        w.write_row(&ConvertedRow {
            fields: vec!["DC".into()],
            headers: vec!["Name".into()],
            latitude: Some(38.0),
            longitude: Some(-77.0),
            mgrs_source: None,
        })
        .unwrap();
        w.finish().unwrap();
        let json: Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(
            json["objects"]["points"]["geometries"][0]["properties"]["Name"],
            "DC"
        );
    }

    #[test]
    fn test_input_reads_topology() {
        let data = r#"{"type":"Topology","objects":{"points":{"type":"GeometryCollection",
            "geometries":[{"type":"Point","coordinates":[-77.0365,38.8977],
            "properties":{"Name":"DC"}}]}},"arcs":[]}"#;
        let mut r = TopoJsonInput::new(Cursor::new(data)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
    }

    #[test]
    fn test_roundtrip() {
        let mut buf = Vec::new();
        let mut w = TopoJsonOutput::new(&mut buf);
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
        let mut r = TopoJsonInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        let name = rec.fields.iter().find(|(k, _)| k == "Name").unwrap();
        assert_eq!(name.1, "DC");
        assert!(r.next_record().unwrap().is_none());
    }
}
