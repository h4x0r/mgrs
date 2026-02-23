use crate::formats::{ConvertedRow, InputFormat, InputRecord, OutputFormat};
use anyhow::Result;
use std::io::{BufRead, BufReader, Read, Write};

pub struct WktOutput<W: Write> {
    output: W,
}

impl<W: Write> WktOutput<W> {
    pub fn new(output: W) -> Self {
        Self { output }
    }
}

impl<W: Write> OutputFormat for WktOutput<W> {
    fn write_header(&mut self, _headers: &[String]) -> Result<()> {
        Ok(())
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        if let (Some(lat), Some(lon)) = (row.latitude, row.longitude) {
            writeln!(self.output, "POINT({lon} {lat})")?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.output.flush()?;
        Ok(())
    }
}

pub struct WktInput {
    records: std::vec::IntoIter<(f64, f64)>,
}

impl WktInput {
    pub fn new<R: Read>(input: R) -> Result<Self> {
        let reader = BufReader::new(input);
        let mut points = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((lon, lat)) = parse_wkt_point(trimmed) {
                points.push((lat, lon));
            }
        }
        Ok(Self {
            records: points.into_iter(),
        })
    }
}

fn parse_wkt_point(s: &str) -> Option<(f64, f64)> {
    let upper = s.trim().to_uppercase();
    if !upper.starts_with("POINT") {
        return None;
    }
    let rest = s.trim()[5..].trim();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?))
}

impl InputFormat for WktInput {
    fn headers(&self) -> Vec<String> {
        vec!["Latitude".into(), "Longitude".into()]
    }

    fn next_record(&mut self) -> Result<Option<InputRecord>> {
        match self.records.next() {
            Some((lat, lon)) => Ok(Some(InputRecord {
                fields: vec![
                    ("Latitude".into(), lat.to_string()),
                    ("Longitude".into(), lon.to_string()),
                ],
                latitude: Some(lat),
                longitude: Some(lon),
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_wkt_output_single_point() {
        let mut buf = Vec::new();
        let mut w = WktOutput::new(&mut buf);
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
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out.trim(), "POINT(-77.0365 38.8977)");
    }

    #[test]
    fn test_wkt_output_multiple_points() {
        let mut buf = Vec::new();
        let mut w = WktOutput::new(&mut buf);
        w.write_header(&[]).unwrap();
        for i in 0..3 {
            w.write_row(&ConvertedRow {
                fields: vec![],
                headers: vec![],
                latitude: Some(38.0 + i as f64),
                longitude: Some(-77.0 + i as f64),
                mgrs_source: None,
            })
            .unwrap();
        }
        w.finish().unwrap();
        let out = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_wkt_output_skips_missing_coords() {
        let mut buf = Vec::new();
        let mut w = WktOutput::new(&mut buf);
        w.write_header(&[]).unwrap();
        w.write_row(&ConvertedRow {
            fields: vec![],
            headers: vec![],
            latitude: None,
            longitude: None,
            mgrs_source: None,
        })
        .unwrap();
        w.finish().unwrap();
        assert!(String::from_utf8(buf).unwrap().trim().is_empty());
    }

    #[test]
    fn test_wkt_input_parses_point() {
        let data = "POINT(-77.0365 38.8977)\n";
        let mut r = WktInput::new(Cursor::new(data)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.0001);
    }

    #[test]
    fn test_wkt_input_handles_whitespace() {
        let data = "  POINT( -77.0365  38.8977 )  \n";
        let mut r = WktInput::new(Cursor::new(data)).unwrap();
        assert!(r.next_record().unwrap().unwrap().latitude.is_some());
    }

    #[test]
    fn test_wkt_roundtrip() {
        let mut buf = Vec::new();
        let mut w = WktOutput::new(&mut buf);
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

        let mut r = WktInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!(r.next_record().unwrap().is_none());
    }
}
