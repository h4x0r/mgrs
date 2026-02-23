use crate::formats::kml::KmlOutput;
use crate::formats::kml_input::KmlInput;
use crate::formats::{ConvertedRow, InputFormat, InputRecord, OutputFormat};
use anyhow::{Context, Result};
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;

/// Writes KMZ (zipped KML) output. Buffers KML internally, then wraps in ZIP on finish.
pub struct KmzOutput<W: Write> {
    output: W,
    kml_writer: KmlOutput<Vec<u8>>,
}

impl<W: Write> KmzOutput<W> {
    pub fn new(output: W, name_column: Option<String>) -> Self {
        Self {
            output,
            kml_writer: KmlOutput::new(Vec::new(), name_column),
        }
    }
}

impl<W: Write> OutputFormat for KmzOutput<W> {
    fn write_header(&mut self, headers: &[String]) -> Result<()> {
        self.kml_writer.write_header(headers)
    }

    fn write_row(&mut self, row: &ConvertedRow) -> Result<()> {
        self.kml_writer.write_row(row)
    }

    fn finish(&mut self) -> Result<()> {
        self.kml_writer.finish()?;

        // Take the KML bytes out of the inner writer
        let kml_bytes = std::mem::take(&mut self.kml_writer.output);

        // Build ZIP in memory (ZipWriter requires Write + Seek)
        let mut zip_buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut zip_buf);
            zip.start_file("doc.kml", SimpleFileOptions::default())?;
            zip.write_all(&kml_bytes)?;
            zip.finish()?;
        }

        // Write ZIP bytes to actual output
        self.output.write_all(&zip_buf.into_inner())?;
        self.output.flush()?;
        Ok(())
    }
}

/// Reads KMZ input by extracting doc.kml and delegating to KmlInput.
pub struct KmzInput {
    inner: KmlInput,
}

impl KmzInput {
    pub fn new<R: Read + std::io::Seek>(input: R) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(input).context("Failed to read KMZ archive")?;
        let mut kml_content = String::new();
        archive
            .by_name("doc.kml")
            .context("KMZ archive missing doc.kml")?
            .read_to_string(&mut kml_content)?;
        let inner = KmlInput::from_kml_string(&kml_content)?;
        Ok(Self { inner })
    }
}

impl InputFormat for KmzInput {
    fn headers(&self) -> Vec<String> {
        self.inner.headers()
    }
    fn next_record(&mut self) -> Result<Option<InputRecord>> {
        self.inner.next_record()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{ConvertedRow, InputFormat, OutputFormat};
    use std::io::Cursor;

    #[test]
    fn test_output_is_valid_zip() {
        let mut buf = Vec::new();
        {
            let mut w = KmzOutput::new(&mut buf, None);
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
        // Verify it's a valid ZIP archive
        let reader = Cursor::new(&buf);
        let archive = zip::ZipArchive::new(reader).expect("output should be valid ZIP");
        assert!(!archive.is_empty());
    }

    #[test]
    fn test_output_contains_doc_kml() {
        let mut buf = Vec::new();
        {
            let mut w = KmzOutput::new(&mut buf, None);
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
        let reader = Cursor::new(&buf);
        let mut archive = zip::ZipArchive::new(reader).unwrap();
        let kml_file = archive.by_name("doc.kml").expect("should contain doc.kml");
        assert!(kml_file.size() > 0);
    }

    #[test]
    fn test_kml_content_valid() {
        let mut buf = Vec::new();
        {
            let mut w = KmzOutput::new(&mut buf, None);
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
        let reader = Cursor::new(&buf);
        let mut archive = zip::ZipArchive::new(reader).unwrap();
        let mut kml_content = String::new();
        archive
            .by_name("doc.kml")
            .unwrap()
            .read_to_string(&mut kml_content)
            .unwrap();
        assert!(kml_content.contains("<?xml"));
        assert!(kml_content.contains("<kml"));
        assert!(kml_content.contains("<Placemark>"));
        assert!(kml_content.contains("<name>DC</name>"));
    }

    #[test]
    fn test_input_reads_doc_kml() {
        // Build a KMZ first
        let mut buf = Vec::new();
        {
            let mut w = KmzOutput::new(&mut buf, None);
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
        let mut r = KmzInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8977).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip() {
        let mut buf = Vec::new();
        {
            let mut w = KmzOutput::new(&mut buf, Some("Location".to_string()));
            w.write_header(&["ID".into(), "Location".into()]).unwrap();
            w.write_row(&ConvertedRow {
                fields: vec!["1".into(), "Pentagon".into()],
                headers: vec!["ID".into(), "Location".into()],
                latitude: Some(38.8719),
                longitude: Some(-77.0563),
                mgrs_source: None,
            })
            .unwrap();
            w.finish().unwrap();
        }
        let mut r = KmzInput::new(Cursor::new(&buf)).unwrap();
        let rec = r.next_record().unwrap().unwrap();
        assert!((rec.latitude.unwrap() - 38.8719).abs() < 0.001);
        assert!((rec.longitude.unwrap() - (-77.0563)).abs() < 0.001);
        assert!(r.next_record().unwrap().is_none());
    }
}
