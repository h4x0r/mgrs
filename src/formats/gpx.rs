use std::io::Write;
use anyhow::Result;
use crate::formats::{ConvertedRow, OutputFormat};
use crate::formats::kml::escape_xml;

pub struct GpxOutput<W: Write> {
    output: W,
    name_column: Option<String>,
    waypoints: Vec<String>,
}

impl<W: Write> GpxOutput<W> {
    pub fn new(output: W, name_column: Option<String>) -> Self {
        Self {
            output,
            name_column,
            waypoints: Vec::new(),
        }
    }

    fn get_name(&self, row: &ConvertedRow) -> String {
        if let Some(ref name_col) = self.name_column {
            for (header, field) in row.headers.iter().zip(row.fields.iter()) {
                if header == name_col {
                    return escape_xml(field);
                }
            }
        }
        for field in &row.fields {
            if !field.trim().is_empty() && field.parse::<f64>().is_err() {
                return escape_xml(field);
            }
        }
        String::from("Unnamed")
    }
}

impl<W: Write> OutputFormat for GpxOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let name = self.get_name(row);
        self.waypoints.push(format!(
            "  <wpt lat=\"{}\" lon=\"{}\">\n    <name>{}</name>\n  </wpt>",
            lat, lon, name
        ));
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        write!(
            self.output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"mgrs\"\n     xmlns=\"http://www.topografix.com/GPX/1/1\">\n{}\n</gpx>\n",
            self.waypoints.join("\n")
        )?;
        self.output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_gpx_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["White House".to_string()],
                headers: vec!["Name".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<?xml"));
        assert!(output.contains("<gpx"));
        assert!(output.contains("<wpt"));
        assert!(output.contains("lat=\"38.8977\""));
        assert!(output.contains("lon=\"-77.0365\""));
        assert!(output.contains("<name>White House</name>"));
    }

    #[test]
    fn test_gpx_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["NoCoords".to_string()],
                headers: vec!["Name".to_string()],
                latitude: None,
                longitude: None,
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("<wpt"));
    }

    #[test]
    fn test_gpx_uses_name_column() {
        let mut buf = Vec::new();
        {
            let mut writer = GpxOutput::new(&mut buf, Some("Site".to_string()));
            writer.write_header(&["ID".to_string(), "Site".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["1".to_string(), "Alpha".to_string()],
                headers: vec!["ID".to_string(), "Site".to_string()],
                latitude: Some(51.0),
                longitude: Some(-0.1),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<name>Alpha</name>"));
    }
}
