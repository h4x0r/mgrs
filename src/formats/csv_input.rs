use std::io::{Read, BufReader};
use anyhow::Result;
use crate::formats::{InputFormat, InputRecord};

pub struct CsvInput {
    headers: Vec<String>,
    records: std::vec::IntoIter<csv::StringRecord>,
}

impl CsvInput {
    pub fn new<R: Read>(input: R) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(BufReader::new(input));
        let headers: Vec<String> = reader.headers()?.iter().map(|h| h.to_string()).collect();
        let records: Vec<csv::StringRecord> = reader.records().collect::<Result<_, _>>()?;
        Ok(Self {
            headers,
            records: records.into_iter(),
        })
    }
}

impl InputFormat for CsvInput {
    fn headers(&self) -> Vec<String> {
        self.headers.clone()
    }

    fn next_record(&mut self) -> Result<Option<InputRecord>> {
        match self.records.next() {
            Some(record) => {
                let fields: Vec<(String, String)> = self.headers.iter()
                    .zip(record.iter())
                    .map(|(h, v)| (h.clone(), v.to_string()))
                    .collect();
                Ok(Some(InputRecord {
                    fields,
                    latitude: None,
                    longitude: None,
                }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_csv_input_reads_headers() {
        let data = "Name,MGRS,Notes\nWhite House,18SUJ2337006519,DC\n";
        let reader = CsvInput::new(Cursor::new(data)).unwrap();
        assert_eq!(reader.headers(), vec!["Name", "MGRS", "Notes"]);
    }

    #[test]
    fn test_csv_input_iterates_records() {
        let data = "Name,MGRS\nA,18SUJ2337006519\nB,30UXC9983606474\n";
        let mut reader = CsvInput::new(Cursor::new(data)).unwrap();

        let rec1 = reader.next_record().unwrap().unwrap();
        assert_eq!(rec1.fields, vec![
            ("Name".to_string(), "A".to_string()),
            ("MGRS".to_string(), "18SUJ2337006519".to_string()),
        ]);
        assert!(rec1.latitude.is_none());

        let rec2 = reader.next_record().unwrap().unwrap();
        assert_eq!(rec2.fields[0].1, "B");
    }

    #[test]
    fn test_csv_input_exhaustion_returns_none() {
        let data = "Name\nA\n";
        let mut reader = CsvInput::new(Cursor::new(data)).unwrap();
        assert!(reader.next_record().unwrap().is_some());
        assert!(reader.next_record().unwrap().is_none());
        assert!(reader.next_record().unwrap().is_none());
    }
}
