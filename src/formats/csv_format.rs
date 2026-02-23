use crate::formats::{ConvertedRow, OutputFormat};
use anyhow::Result;
use std::io::Write;

pub struct CsvOutput<W: Write> {
    writer: csv::Writer<W>,
}

impl<W: Write> CsvOutput<W> {
    pub fn new(output: W) -> Self {
        Self {
            writer: csv::Writer::from_writer(output),
        }
    }
}

impl<W: Write> OutputFormat for CsvOutput<W> {
    fn write_header(&mut self, headers: &[String]) -> Result<()> {
        let mut row: Vec<&str> = headers.iter().map(|h| h.as_str()).collect();
        row.push("Latitude");
        row.push("Longitude");
        self.writer.write_record(&row)?;
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        let mut record: Vec<String> = row.fields.clone();
        record.push(row.latitude.map(|l| l.to_string()).unwrap_or_default());
        record.push(row.longitude.map(|l| l.to_string()).unwrap_or_default());
        self.writer.write_record(&record)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::ConvertedRow;

    #[test]
    fn test_csv_output_writes_headers_with_latlon() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
            writer
                .write_header(&["Name".to_string(), "MGRS".to_string()])
                .unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Name"));
        assert!(output.contains("MGRS"));
        assert!(output.contains("Latitude"));
        assert!(output.contains("Longitude"));
    }

    #[test]
    fn test_csv_output_writes_row_with_coordinates() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
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
        assert!(output.contains("38.8977"));
        assert!(output.contains("-77.0365"));
    }

    #[test]
    fn test_csv_output_writes_empty_on_failed_conversion() {
        let mut buf = Vec::new();
        {
            let mut writer = CsvOutput::new(&mut buf);
            writer.write_header(&["Name".to_string()]).unwrap();
            writer
                .write_row(&ConvertedRow {
                    fields: vec!["Place".to_string()],
                    headers: vec!["Name".to_string()],
                    latitude: None,
                    longitude: None,
                    mgrs_source: None,
                })
                .unwrap();
            writer.finish().unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2); // header + 1 row
                                    // Row should end with two empty fields
        assert!(lines[1].ends_with(",,"));
    }
}
