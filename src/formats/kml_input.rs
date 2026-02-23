use std::io::Read;
use anyhow::Result;
use crate::formats::{InputFormat, InputRecord};

pub struct KmlInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<InputRecord>,
}

impl KmlInput {
    pub fn new<R: Read>(mut input: R) -> Result<Self> {
        let mut xml = String::new();
        input.read_to_string(&mut xml)?;
        Self::from_kml_string(&xml)
    }

    pub fn from_kml_string(xml: &str) -> Result<Self> {
        let placemarks = extract_placemarks(xml);
        let mut key_order = Vec::new();
        key_order.push("Name".to_string());
        for pm in &placemarks {
            for (k, _) in &pm.data {
                if !key_order.contains(k) { key_order.push(k.clone()); }
            }
        }

        let mut records = Vec::new();
        for pm in placemarks {
            let mut fields = Vec::new();
            for key in &key_order {
                if key == "Name" {
                    fields.push(("Name".into(), pm.name.clone()));
                } else {
                    let val = pm.data.iter().find(|(k,_)| k == key)
                        .map(|(_,v)| v.clone()).unwrap_or_default();
                    fields.push((key.clone(), val));
                }
            }
            records.push(InputRecord { fields, latitude: pm.lat, longitude: pm.lon });
        }
        Ok(Self { headers: key_order, records: records.into_iter() })
    }
}

struct Placemark { name: String, lat: Option<f64>, lon: Option<f64>, data: Vec<(String, String)> }

fn extract_placemarks(xml: &str) -> Vec<Placemark> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(s) = xml[pos..].find("<Placemark>") {
        let s = pos + s;
        let e = match xml[s..].find("</Placemark>") {
            Some(e) => s + e + "</Placemark>".len(), None => break,
        };
        let block = &xml[s..e];
        let name = tag_content(block, "name").unwrap_or_default();
        let (lat, lon) = parse_coords(block);
        let data = parse_extended_data(block);
        out.push(Placemark { name, lat, lon, data });
        pos = e;
    }
    out
}

fn tag_content(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let s = xml.find(&open)? + open.len();
    let e = xml[s..].find(&close)? + s;
    Some(unescape(&xml[s..e]))
}

fn parse_coords(xml: &str) -> (Option<f64>, Option<f64>) {
    if let Some(c) = tag_content(xml, "coordinates") {
        let parts: Vec<&str> = c.trim().split(',').collect();
        if parts.len() >= 2 {
            return (parts[1].trim().parse().ok(), parts[0].trim().parse().ok());
        }
    }
    (None, None)
}

fn parse_extended_data(xml: &str) -> Vec<(String, String)> {
    let mut data = Vec::new();
    let mut pos = 0;
    while let Some(s) = xml[pos..].find("<Data name=\"") {
        let s = pos + s;
        let ns = s + "<Data name=\"".len();
        let ne = match xml[ns..].find('"') { Some(e) => ns + e, None => break };
        let name = unescape(&xml[ns..ne]);
        let de = match xml[s..].find("</Data>") { Some(e) => s + e, None => break };
        let val = tag_content(&xml[s..de], "value").unwrap_or_default();
        data.push((name, val));
        pos = de + "</Data>".len();
    }
    data
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&apos;", "'")
}

impl InputFormat for KmlInput {
    fn headers(&self) -> Vec<String> { self.headers.clone() }
    fn next_record(&mut self) -> Result<Option<InputRecord>> { Ok(self.records.next()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
    <Placemark><name>White House</name>
      <ExtendedData>
        <Data name="Name"><value>White House</value></Data>
        <Data name="MGRS"><value>18SUJ2337006519</value></Data>
      </ExtendedData>
      <Point><coordinates>-77.0365,38.8977,0</coordinates></Point>
    </Placemark>
</Document></kml>"#
    }

    #[test]
    fn test_reads_placemarks() {
        let mut r = KmlInput::new(Cursor::new(sample())).unwrap();
        assert!(r.next_record().unwrap().is_some());
        assert!(r.next_record().unwrap().is_none());
    }

    #[test]
    fn test_extracts_coordinates() {
        let mut r = KmlInput::new(Cursor::new(sample())).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
        assert!((rec.longitude.unwrap() - (-77.0365)).abs() < 0.0001);
    }

    #[test]
    fn test_extracts_extended_data() {
        let mut r = KmlInput::new(Cursor::new(sample())).unwrap();
        assert!(r.headers().contains(&"MGRS".to_string()));
        let rec = r.next_record().unwrap().unwrap();
        let name = rec.fields.iter().find(|(k,_)| k == "Name").unwrap();
        assert_eq!(name.1, "White House");
    }

    #[test]
    fn test_roundtrip() {
        use crate::formats::kml::KmlOutput;
        use crate::formats::{ConvertedRow, OutputFormat};
        let mut buf = Vec::new();
        {
            let mut w = KmlOutput::new(&mut buf, None);
            w.write_header(&["Name".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["DC".into()], headers: vec!["Name".into()],
                latitude: Some(38.8977), longitude: Some(-77.0365), mgrs_source: None,
            }).unwrap();
            w.finish().unwrap();
        }
        let mut r = KmlInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.0001);
    }
}
