use std::io::Write;
use anyhow::Result;
use crate::formats::{ConvertedRow, OutputFormat};

pub struct KmlOutput<W: Write> {
    pub(crate) output: W,
    name_column: Option<String>,
    placemarks: Vec<String>,
}

impl<W: Write> KmlOutput<W> {
    pub fn new(output: W, name_column: Option<String>) -> Self {
        Self {
            output,
            name_column,
            placemarks: Vec::new(),
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
        // Default: use first non-numeric field
        for field in &row.fields {
            if !field.trim().is_empty() && field.parse::<f64>().is_err() {
                return escape_xml(field);
            }
        }
        String::from("Unnamed")
    }
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl<W: Write> OutputFormat for KmlOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let (lat, lon) = match (row.latitude, row.longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => return Ok(()),
        };

        let name = self.get_name(row);
        let mut extended_data = String::new();
        for (header, field) in row.headers.iter().zip(row.fields.iter()) {
            extended_data.push_str(&format!(
                "        <Data name=\"{}\"><value>{}</value></Data>\n",
                escape_xml(header),
                escape_xml(field)
            ));
        }

        self.placemarks.push(format!(
            "    <Placemark>\n      <name>{}</name>\n      <ExtendedData>\n{}      </ExtendedData>\n      <Point>\n        <coordinates>{},{},0</coordinates>\n      </Point>\n    </Placemark>",
            name, extended_data, lon, lat
        ));
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        write!(
            self.output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n{}\n  </Document>\n</kml>\n",
            self.placemarks.join("\n")
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
    fn test_kml_output_valid_structure() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, None);
            writer.write_header(&["Name".to_string(), "MGRS".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["White House".to_string(), "18SUJ2337006519".to_string()],
                headers: vec!["Name".to_string(), "MGRS".to_string()],
                latitude: Some(38.8977),
                longitude: Some(-77.0365),
                mgrs_source: Some("18SUJ2337006519".to_string()),
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<?xml"));
        assert!(output.contains("<kml"));
        assert!(output.contains("<Placemark>"));
        assert!(output.contains("<name>White House</name>"));
        assert!(output.contains("-77.0365,38.8977"));
    }

    #[test]
    fn test_kml_uses_name_column() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, Some("Location".to_string()));
            writer.write_header(&["ID".to_string(), "Location".to_string()]).unwrap();
            writer.write_row(&ConvertedRow {
                fields: vec!["1".to_string(), "My Place".to_string()],
                headers: vec!["ID".to_string(), "Location".to_string()],
                latitude: Some(38.0),
                longitude: Some(-77.0),
                mgrs_source: None,
            }).unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<name>My Place</name>"));
    }

    #[test]
    fn test_kml_skips_rows_without_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = KmlOutput::new(&mut buf, None);
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
        assert!(!output.contains("<Placemark>"));
    }
}
