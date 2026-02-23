use crate::formats::{ConvertedRow, OutputFormat};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::Write;

pub struct GeoJsonOutput<W: Write> {
    output: W,
    features: Vec<Value>,
}

impl<W: Write> GeoJsonOutput<W> {
    pub fn new(output: W) -> Self {
        Self {
            output,
            features: Vec::new(),
        }
    }
}

impl<W: Write> OutputFormat for GeoJsonOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let mut properties = serde_json::Map::new();
        for (header, field) in row.headers.iter().zip(row.fields.iter()) {
            properties.insert(header.clone(), Value::String(field.clone()));
        }

        let feature = json!({
            "type": "Feature",
            "geometry": {
                "type": "Point",
                "coordinates": [lon, lat]
            },
            "properties": properties
        });

        self.features.push(feature);
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        let collection = json!({
            "type": "FeatureCollection",
            "features": self.features
        });
        serde_json::to_writer_pretty(&mut self.output, &collection)?;
        self.output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_geojson_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer
                .write_header(&["Name".to_string(), "MGRS".to_string()])
                .unwrap();
            writer
                .write_row(&ConvertedRow {
                    fields: vec!["DC".to_string(), "18SUJ2337006519".to_string()],
                    headers: vec!["Name".to_string(), "MGRS".to_string()],
                    latitude: Some(38.8977),
                    longitude: Some(-77.0365),
                    mgrs_source: Some("18SUJ2337006519".to_string()),
                })
                .unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["type"], "FeatureCollection");
        assert_eq!(json["features"].as_array().unwrap().len(), 1);
        assert_eq!(json["features"][0]["type"], "Feature");
        assert_eq!(json["features"][0]["geometry"]["type"], "Point");
        // GeoJSON uses [lon, lat] order
        assert_eq!(json["features"][0]["geometry"]["coordinates"][0], -77.0365);
        assert_eq!(json["features"][0]["geometry"]["coordinates"][1], 38.8977);
        assert_eq!(json["features"][0]["properties"]["Name"], "DC");
    }

    #[test]
    fn test_geojson_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer
                .write_row(&ConvertedRow {
                    fields: vec!["NoCoords".to_string()],
                    headers: vec!["Name".to_string()],
                    latitude: None,
                    longitude: None,
                    mgrs_source: None,
                })
                .unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["features"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_geojson_multiple_features() {
        let mut buf = Vec::new();
        {
            let mut writer = GeoJsonOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            for i in 0..3 {
                writer
                    .write_row(&ConvertedRow {
                        fields: vec![format!("Place{}", i)],
                        headers: vec!["Name".to_string()],
                        latitude: Some(38.0 + i as f64),
                        longitude: Some(-77.0 + i as f64),
                        mgrs_source: None,
                    })
                    .unwrap();
            }
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["features"].as_array().unwrap().len(), 3);
    }
}
