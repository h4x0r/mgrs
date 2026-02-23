use std::io::Read;
use anyhow::Result;
use crate::formats::{InputFormat, InputRecord};

pub struct GpxInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl GpxInput {
    pub fn new<R: Read>(mut input: R) -> Result<Self> {
        let mut xml = String::new();
        input.read_to_string(&mut xml)?;
        let waypoints = extract_waypoints(&xml);
        let headers = vec!["Name".to_string()];
        let records: Vec<InputRecord> = waypoints.into_iter().map(|wp| {
            InputRecord {
                fields: vec![("Name".into(), wp.name)],
                latitude: wp.lat,
                longitude: wp.lon,
            }
        }).collect();
        Ok(Self { headers, records: records.into_iter() })
    }
}

struct Waypoint { name: String, lat: Option<f64>, lon: Option<f64> }

fn extract_waypoints(xml: &str) -> Vec<Waypoint> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(s) = xml[pos..].find("<wpt ") {
        let s = pos + s;
        let e = match xml[s..].find("</wpt>") {
            Some(e) => s + e + "</wpt>".len(), None => break,
        };
        let block = &xml[s..e];
        let lat = attr(block, "lat").and_then(|s| s.parse().ok());
        let lon = attr(block, "lon").and_then(|s| s.parse().ok());
        let name = tag_content(block, "name").unwrap_or_default();
        out.push(Waypoint { name, lat, lon });
        pos = e;
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{}=\"", name);
    let s = tag.find(&pat)? + pat.len();
    let e = tag[s..].find('"')? + s;
    Some(tag[s..e].to_string())
}

fn tag_content(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let s = xml.find(&open)? + open.len();
    let e = xml[s..].find(&close)? + s;
    Some(unescape(&xml[s..e]))
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&apos;", "'")
}

impl InputFormat for GpxInput {
    fn headers(&self) -> Vec<String> { self.headers.clone() }
    fn next_record(&mut self) -> Result<Option<InputRecord>> { Ok(self.records.next()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="mgrs" xmlns="http://www.topografix.com/GPX/1/1">
  <wpt lat="38.8977" lon="-77.0365"><name>White House</name></wpt>
  <wpt lat="51.5055" lon="-0.0754"><name>Tower Bridge</name></wpt>
</gpx>"#
    }

    #[test]
    fn test_reads_waypoints() {
        let mut r = GpxInput::new(Cursor::new(sample())).unwrap();
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_none());
    }

    #[test]
    fn test_extracts_coordinates() {
        let mut r = GpxInput::new(Cursor::new(sample())).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.0001);
    }

    #[test]
    fn test_extracts_names() {
        let mut r = GpxInput::new(Cursor::new(sample())).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        let name = rec.fields.iter().find(|(k,_)| k == "Name").unwrap();
        assert_eq!(name.1, "White House");
    }

    #[test]
    fn test_roundtrip() {
        use crate::formats::gpx::GpxOutput;
        use crate::formats::{ConvertedRow, OutputFormat};
        let mut buf = Vec::new();
        {
            let mut w = GpxOutput::new(&mut buf, None);
            w.write_header(&["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()], headers: vec!["Name".into()],
                latitude: Some(38.8977), longitude: Some(-77.0365), mgrs_source: None,
            }).unwrap();
            w.finish().unwrap();
        }
        let mut r = GpxInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
    }
}
