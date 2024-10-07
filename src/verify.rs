//! WARC file verification.

use std::{collections::HashMap, path::Path, str::FromStr};

use data_encoding::HEXLOWER;
use redb::{backends::InMemoryBackend, Database, MultimapTableDefinition, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::{
    digest::{Algorithm, Digest, Hasher},
    error::StorageError,
    header::{fields::FieldsExt, WarcHeader},
};

// mapping of record ID => ()
const RECORDS_TABLE: TableDefinition<&str, ()> = TableDefinition::new("records");
// mapping of record ID => (reference target record ID, type of reference)
const ID_REFERENCES_TABLE: MultimapTableDefinition<&str, (&str, &str)> =
    MultimapTableDefinition::new("id_references");
// mapping of (origin record ID, segment number) => record block length
const SEGMENT_ID_TABLE: TableDefinition<(&str, u64), u64> = TableDefinition::new("segments");
// mapping of origin record ID => total length
const SEGMENT_LENGTH_TABLE: TableDefinition<&str, u64> = TableDefinition::new("segment_lengths");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProblemKind {
    UnsupportedRecordType(String),
    RequiredFieldMissing(String),
    ProhibitedField(String),
    TargetConcurrentToMissing(String),
    UnknownDigest(String),
    BadSpecUri(String),
    ParseInt(String),
    InvalidBeginSegment,
    InvalidEndSegment,
    DigestMismatch {
        algorithm: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    record_id: String,
    kind: ProblemKind,
}

impl Problem {
    pub fn new<I: Into<String>>(record_id: I, kind: ProblemKind) -> Self {
        Self {
            record_id: record_id.into(),
            kind,
        }
    }
}

/// Checks WARCs for specification conformance and integrity.
pub struct Verifier {
    db: Database,
    problems: Vec<Problem>,
    id_references_cursor: Option<String>,
    segment_length_cursor: Option<String>,
}

impl Verifier {
    pub fn new() -> Self {
        let db = Database::builder()
            .set_cache_size(8 * 1024 * 1024)
            .create_with_backend(InMemoryBackend::new())
            .unwrap();
        Self::new_impl(db).unwrap()
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = Database::builder()
            .set_cache_size(8 * 1024 * 1024)
            .create(path)?;
        Self::new_impl(db)
    }

    fn new_impl(db: Database) -> Result<Self, StorageError> {
        Ok(Self {
            db,
            problems: Vec::new(),
            id_references_cursor: Some(String::new()),
            segment_length_cursor: Some(String::new()),
        })
    }

    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    pub fn problems_mut(&mut self) -> &mut Vec<Problem> {
        &mut self.problems
    }

    pub fn verify_record<'a>(
        &'a mut self,
        header: &'a WarcHeader,
    ) -> Result<VerifierRecord<'a>, StorageError> {
        let mut record = VerifierRecord::new(&mut self.db, &mut self.problems, header);
        record.process_header()?;
        Ok(record)
    }

    pub fn verify_end(&mut self) -> Result<VerifyStatus, StorageError> {
        let txn = self.db.begin_read()?;
        let records_table = txn.open_table(RECORDS_TABLE)?;
        let id_references_table = txn.open_multimap_table(ID_REFERENCES_TABLE)?;
        let segment_id_table = txn.open_table(SEGMENT_ID_TABLE)?;
        let segment_length_table = txn.open_table(SEGMENT_LENGTH_TABLE)?;

        if let Some(cursor) = &self.id_references_cursor {
            todo!()
        }

        if let Some(cursor) = &self.segment_length_cursor {
            todo!()
        }

        if self.id_references_cursor.is_none() && self.segment_length_cursor.is_none() {
            Ok(VerifyStatus::Done)
        } else {
            Ok(VerifyStatus::HasMore)
        }
    }
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerifyStatus {
    HasMore,
    Done,
}

pub struct VerifierRecord<'a> {
    db: &'a Database,
    problems: &'a mut Vec<Problem>,
    header: &'a WarcHeader,
    digests: HashMap<Algorithm, Digest>,
    hashers: Vec<Hasher>,
}

impl<'a> VerifierRecord<'a> {
    pub(crate) fn new(
        database: &'a Database,
        problems: &'a mut Vec<Problem>,
        header: &'a WarcHeader,
    ) -> Self {
        Self {
            db: database,
            problems,
            header,
            digests: HashMap::new(),
            hashers: Vec::new(),
        }
    }

    fn record_id(&self) -> &str {
        self.header.fields.get_or_default("WARC-Record-ID")
    }

    fn record_type(&self) -> &str {
        self.header.fields.get_or_default("WARC-Record-Type")
    }

    fn add_problem(&mut self, kind: ProblemKind) {
        self.problems.push(Problem::new(self.record_id(), kind));
    }

    fn require_field(&mut self, name: &str) {
        if !self.header.fields.contains_name(name) {
            self.add_problem(ProblemKind::RequiredFieldMissing(name.to_string()));
        }
    }

    fn require_fields(&mut self, names: &[&str]) {
        for name in names {
            self.require_field(name);
        }
    }

    fn prohibit_field(&mut self, name: &str) -> bool {
        if self.header.fields.contains_name(name) {
            self.add_problem(ProblemKind::ProhibitedField(name.to_string()));
            true
        } else {
            false
        }
    }

    fn is_any_record_type(&mut self, types: &[&str]) -> bool {
        types.contains(&self.record_type())
    }

    pub(crate) fn process_header(&mut self) -> Result<(), StorageError> {
        self.mandatory_fields();
        self.concurrent_to()?;
        self.refers_to()?;
        self.refers_to_target_uri();
        self.refers_to_date();
        self.target_uri();
        self.warcinfo_id()?;
        self.filename();
        self.profile();
        self.segment()?;
        self.block_digest();

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(RECORDS_TABLE)?;
            table.insert(self.record_id(), ())?;
        }
        txn.commit()?;
        Ok(())
    }

    fn mandatory_fields(&mut self) {
        self.require_fields(&["WARC-Record-ID", "Content-Length", "WARC-Date", "WARC-Type"]);

        if !self.is_any_record_type(&[
            "warcinfo",
            "response",
            "resource",
            "request",
            "metadata",
            "revisit",
            "conversion",
            "continuation",
        ]) {
            self.add_problem(ProblemKind::UnsupportedRecordType(
                self.record_type().to_string(),
            ));
        }
    }

    fn concurrent_to(&mut self) -> Result<(), StorageError> {
        if self.is_any_record_type(&["warcinfo", "conversion", "continuation"])
            && self.prohibit_field("WARC-Concurrent-To")
        {
            return Ok(());
        }

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_multimap_table(ID_REFERENCES_TABLE)?;

            for target in self.header.fields.get_all("WARC-Concurrent-To") {
                table.insert(self.record_id(), (target.as_str(), "Concurrent-To"))?;
            }
        }

        txn.commit()?;

        Ok(())
    }

    fn refers_to(&mut self) -> Result<(), StorageError> {
        if self.is_any_record_type(&[
            "warcinfo",
            "response",
            "resource",
            "request",
            "continuation",
        ]) && self.prohibit_field("WARC-Refers-To")
        {
            return Ok(());
        }

        if let Some(target) = self.header.fields.get("WARC-Refers-To") {
            let txn = self.db.begin_write()?;
            {
                let mut table = txn.open_multimap_table(ID_REFERENCES_TABLE)?;

                table.insert(self.record_id(), (target.as_str(), "Refers-To"))?;
            }

            txn.commit()?;
        }

        Ok(())
    }

    fn refers_to_target_uri(&mut self) {
        if self.is_any_record_type(&[
            "warcinfo",
            "response",
            "metadata",
            "conversion",
            "resource",
            "request",
            "continuation",
        ]) {
            self.prohibit_field("WARC-Refers-To-Target-URI");
        }
    }

    fn refers_to_date(&mut self) {
        if self.is_any_record_type(&[
            "warcinfo",
            "response",
            "metadata",
            "conversion",
            "resource",
            "request",
            "continuation",
        ]) {
            self.prohibit_field("WARC-Refers-To-Date");
        }
    }

    fn target_uri(&mut self) {
        if self.header.fields.contains_name("WARC-Target-URI")
            && self.is_any_record_type(&["warcinfo"])
        {
            self.prohibit_field("WARC-Target-URI");
        } else if self.is_any_record_type(&[
            "response",
            "resource",
            "request",
            "revisit",
            "conversion",
            "continuation",
        ]) {
            self.require_field("WARC-Target-URI");
        }

        if self
            .header
            .fields
            .is_formatted_bad_spec_url("WARC-Target-URI")
        {
            self.add_problem(ProblemKind::BadSpecUri("WARC-Target-URI".to_string()));
        }
    }

    fn warcinfo_id(&mut self) -> Result<(), StorageError> {
        if self.is_any_record_type(&["warcinfo"]) && self.prohibit_field("WARC-Target-URI") {
            return Ok(());
        }

        if let Some(target) = self.header.fields.get("WARC-Warcinfo-ID") {
            let txn = self.db.begin_write()?;
            {
                let mut table = txn.open_multimap_table(ID_REFERENCES_TABLE)?;
                table.insert(self.record_id(), (target.as_str(), "Warcinfo-ID"))?;
            }
            txn.commit()?;
        }

        Ok(())
    }

    fn filename(&mut self) {
        if !self.is_any_record_type(&["warcinfo"]) {
            self.prohibit_field("WARC-Filename");
        }
    }

    fn profile(&mut self) {
        if self.is_any_record_type(&["profile"]) {
            self.require_field("WARC-Profile");
        }
        if self.header.fields.is_formatted_bad_spec_url("WARC-Profile") {
            self.add_problem(ProblemKind::BadSpecUri("WARC-Target-URI".to_string()));
        }
    }

    fn segment(&mut self) -> Result<(), StorageError> {
        if self.is_any_record_type(&["continuation"]) {
            self.require_field("WARC-Segment-Origin-ID");
            self.require_field("WARC-Segment-Total-Length");
            self.require_field("WARC-Segment-Number");
        } else {
            self.prohibit_field("WARC-Segment-Origin-ID");
            self.prohibit_field("WARC-Segment-Total-Length");
        }

        let number = if let Some(number) = self.header.fields.get_u64_strict("WARC-Segment-Number")
        {
            number
        } else {
            return Ok(());
        };

        let number = match number {
            Ok(number) => number,
            Err(_) => {
                self.add_problem(ProblemKind::ParseInt("WARC-Segment-Number".to_string()));
                return Ok(());
            }
        };

        if number == 1 {
            self.segment_begin()?;
        } else if self
            .header
            .fields
            .contains_name("WARC-Segment-Total-Length")
        {
            self.segment_end(number)?;
        } else {
            self.segment_middle(number)?;
        }

        Ok(())
    }

    fn segment_begin(&mut self) -> Result<(), StorageError> {
        if self.is_any_record_type(&["continuation"]) {
            self.add_problem(ProblemKind::InvalidBeginSegment);
        }

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(SEGMENT_ID_TABLE)?;
            table.insert((self.record_id(), 1), self.header.content_length().unwrap())?;
        }
        txn.commit()?;

        Ok(())
    }

    fn segment_middle(&mut self, number: u64) -> Result<(), StorageError> {
        let origin_id = self.header.fields.get_or_default("WARC-Segment-Origin-ID");

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(SEGMENT_ID_TABLE)?;
            table.insert((origin_id, number), self.header.content_length().unwrap())?;
        }
        txn.commit()?;

        Ok(())
    }

    fn segment_end(&mut self, number: u64) -> Result<(), StorageError> {
        self.segment_middle(number)?;

        let origin_id = self.header.fields.get_or_default("WARC-Segment-Origin-ID");

        if let Some(total_length) = self
            .header
            .fields
            .get_u64_strict("WARC-Segment-Total-Length")
        {
            match total_length {
                Ok(total_length) => {
                    let txn = self.db.begin_write()?;
                    {
                        let mut table = txn.open_table(SEGMENT_LENGTH_TABLE)?;
                        table.insert(origin_id, total_length)?;
                    }
                    txn.commit()?;
                }
                Err(_) => self.add_problem(ProblemKind::ParseInt(
                    "WARC-Segment-Total-Length".to_string(),
                )),
            }
        }
        Ok(())
    }

    fn block_digest(&mut self) {
        for value in self.header.fields.get_all("WARC-Block-Digest") {
            if let Ok(digest) = Digest::from_str(value) {
                self.digests.insert(digest.algorithm(), digest);
            } else {
                self.add_problem(ProblemKind::UnknownDigest(value.to_string()));
            }
        }
    }

    pub fn block_data(&mut self, data: &[u8]) {
        for hasher in &mut self.hashers {
            hasher.update(data);
        }
    }

    pub fn finish_block(&mut self) {
        let mut hashers = std::mem::take(&mut self.hashers);

        for hasher in &mut hashers {
            let value = hasher.finish();

            let digest = self.digests.get(&hasher.algorithm()).unwrap();

            if digest.value() != &value {
                self.add_problem(ProblemKind::DigestMismatch {
                    algorithm: hasher.algorithm().to_string(),
                    expected: HEXLOWER.encode(digest.value()),
                    actual: HEXLOWER.encode(&value),
                });
            }
        }

        self.hashers = hashers;
    }
}
