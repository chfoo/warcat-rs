//! WARC file verification.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    str::FromStr,
};

use data_encoding::HEXLOWER;
use redb::{backends::InMemoryBackend, Database, MultimapTableDefinition, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::{
    digest::{AlgorithmName, Digest, Hasher},
    error::StorageError,
    extract::WarcExtractor,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Check {
    MandatoryFields,
    KnownRecordType,
    ContentType,
    ConcurrentTo,
    BlockDigest,
    PayloadDigest,
    IpAddress,
    RefersTo,
    RefersToTargetUri,
    RefersToDate,
    TargetUri,
    Truncated,
    WarcinfoId,
    Filename,
    Profile,
    // IdentifiedPayloadType,
    Segment,
    RecordAtTimeCompression,
}

impl Check {
    pub fn all() -> &'static [Self] {
        &[
            Self::MandatoryFields,
            Self::KnownRecordType,
            Self::ContentType,
            Self::ConcurrentTo,
            Self::BlockDigest,
            Self::PayloadDigest,
            Self::IpAddress,
            Self::RefersTo,
            Self::RefersToTargetUri,
            Self::RefersToDate,
            Self::TargetUri,
            Self::Truncated,
            Self::WarcinfoId,
            Self::Filename,
            Self::Profile,
            // Self::IdentifiedPayloadType,
            Self::Segment,
            Self::RecordAtTimeCompression,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProblemKind {
    UnknownRecordType(String),
    RequiredFieldMissing(String),
    ProhibitedField(String),
    ReferencedRecordMissing(String),
    UnknownDigest(String),
    BadSpecUri(String),
    ParseInt(String),
    InvalidDate(String),
    InvalidUrl(String),
    InvalidIpAddress(String),
    InvalidMediaType(String),
    InvalidTruncatedReason,
    InvalidSegment,
    MissingSegment(u64),
    MismatchedSegmentLength {
        expect: u64,
        actual: u64,
    },
    DigestMismatch {
        algorithm: String,
        expected: String,
        actual: String,
    },
    PayloadDigestMismatch {
        algorithm: String,
        expected: String,
        actual: String,
    },
    ParsePayload(String),
    NotRecordAtTimeCompression,
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
    checks: HashSet<Check>,
    db: Database,
    problems: Vec<Problem>,
    id_references_cursor: Option<String>,
    segment_length_cursor: Option<String>,
    header: WarcHeader,
    digests: HashMap<AlgorithmName, Digest>,
    hashers: Vec<Hasher>,
    payload_extractor: Option<WarcExtractor>,
    payload_extractor_buf: Vec<u8>,
    payload_digests: HashMap<AlgorithmName, Digest>,
    payload_hashers: Vec<Hasher>,
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
        let txn = db.begin_write()?;
        txn.open_table(RECORDS_TABLE)?;
        txn.open_multimap_table(ID_REFERENCES_TABLE)?;
        txn.open_table(SEGMENT_ID_TABLE)?;
        txn.open_table(SEGMENT_LENGTH_TABLE)?;
        txn.commit()?;

        Ok(Self {
            checks: HashSet::from_iter(Check::all().iter().cloned()),
            db,
            problems: Vec::new(),
            id_references_cursor: Some(String::new()),
            segment_length_cursor: Some(String::new()),
            header: WarcHeader::empty(),
            digests: HashMap::new(),
            hashers: Vec::new(),
            payload_extractor: None,
            payload_extractor_buf: Vec::new(),
            payload_digests: HashMap::new(),
            payload_hashers: Vec::new(),
        })
    }

    pub fn checks(&self) -> &HashSet<Check> {
        &self.checks
    }

    pub fn checks_mut(&mut self) -> &mut HashSet<Check> {
        &mut self.checks
    }

    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    pub fn problems_mut(&mut self) -> &mut Vec<Problem> {
        &mut self.problems
    }

    /// Starts verifying a record.
    ///
    /// After calling this function, call [`block_data()`](Self::block_data).
    pub fn begin_record(&mut self, header: &WarcHeader) -> Result<(), StorageError> {
        self.header = header.clone();
        self.digests.clear();
        self.hashers.clear();
        self.payload_extractor = None;
        self.payload_digests.clear();
        self.payload_hashers.clear();

        self.process_header()?;

        Ok(())
    }

    /// Finish processing any remaining verification.
    ///
    /// This function should be repeated called until [`VerifyStatus::Done`]
    /// is returned.
    pub fn verify_end(&mut self) -> Result<VerifyStatus, StorageError> {
        self.check_references()?;
        self.check_segments()?;

        if self.id_references_cursor.is_none() && self.segment_length_cursor.is_none() {
            Ok(VerifyStatus::Done)
        } else {
            Ok(VerifyStatus::HasMore)
        }
    }

    fn check_references(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check references");

        let txn = self.db.begin_read()?;
        let records_table = txn.open_table(RECORDS_TABLE)?;
        let id_references_table = txn.open_multimap_table(ID_REFERENCES_TABLE)?;

        if let Some(cursor) = self.id_references_cursor.take() {
            let cursor = cursor.as_str();

            for (index, item) in id_references_table.range(cursor..)?.enumerate() {
                let (key, values) = item?;
                let record_id = key.value();

                if index == 1025 {
                    self.id_references_cursor = Some(record_id.to_string());
                    break;
                }

                for item in values {
                    let item = item?;
                    let (target_id, _target_type) = item.value();

                    if records_table.get(target_id)?.is_none() {
                        self.problems.push(Problem::new(
                            record_id.to_string(),
                            ProblemKind::ReferencedRecordMissing(target_id.to_string()),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn check_segments(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check segments");

        let txn = self.db.begin_read()?;

        let segment_id_table = txn.open_table(SEGMENT_ID_TABLE)?;
        let segment_length_table = txn.open_table(SEGMENT_LENGTH_TABLE)?;

        if let Some(cursor) = self.segment_length_cursor.take() {
            let cursor = cursor.as_str();

            for (index, item) in segment_length_table.range(cursor..)?.enumerate() {
                let (key, value) = item?;
                let origin_id = key.value();
                let expected_total_length = value.value();

                if index == 1025 {
                    self.segment_length_cursor = Some(origin_id.to_string());
                }

                let mut expected_number = 1u64;
                let mut current_total_length = 0u64;

                for item in segment_id_table.range((origin_id, 1)..(origin_id, u64::MAX))? {
                    let (key, value) = item?;
                    let (origin_id, number) = key.value();
                    let block_length = value.value();

                    if number != expected_number {
                        self.problems.push(Problem::new(
                            origin_id.to_string(),
                            ProblemKind::MissingSegment(expected_number),
                        ));
                        expected_number = number;
                    }

                    expected_number += 1;
                    current_total_length += block_length;
                }

                if expected_total_length != current_total_length {
                    self.problems.push(Problem::new(
                        origin_id.to_string(),
                        ProblemKind::MismatchedSegmentLength {
                            expect: expected_total_length,
                            actual: current_total_length,
                        },
                    ));
                }
            }
        }

        Ok(())
    }

    fn record_id(&self) -> &str {
        self.header.fields.get_or_default("WARC-Record-ID")
    }

    fn record_type(&self) -> &str {
        self.header.fields.get_or_default("WARC-Type")
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

    pub fn process_header(&mut self) -> Result<(), StorageError> {
        if self.checks.contains(&Check::MandatoryFields) {
            self.mandatory_fields();
        }
        if self.checks.contains(&Check::ContentType) {
            self.content_type();
        }
        if self.checks.contains(&Check::ConcurrentTo) {
            self.concurrent_to()?;
        }
        if self.checks.contains(&Check::IpAddress) {
            self.ip_address();
        }
        if self.checks.contains(&Check::RefersTo) {
            self.refers_to()?;
        }
        if self.checks.contains(&Check::RefersToTargetUri) {
            self.refers_to_target_uri();
        }
        if self.checks.contains(&Check::RefersToDate) {
            self.refers_to_date();
        }
        if self.checks.contains(&Check::TargetUri) {
            self.target_uri();
        }
        if self.checks.contains(&Check::Truncated) {
            self.truncated();
        }
        if self.checks.contains(&Check::WarcinfoId) {
            self.warcinfo_id()?;
        }
        if self.checks.contains(&Check::Filename) {
            self.filename();
        }
        if self.checks.contains(&Check::Profile) {
            self.profile();
        }
        if self.checks.contains(&Check::Segment) {
            self.segment()?;
        }
        if self.checks.contains(&Check::BlockDigest) {
            self.block_digest();
        }
        if self.checks.contains(&Check::PayloadDigest) {
            self.payload_digest();
        }

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(RECORDS_TABLE)?;
            table.insert(self.record_id(), ())?;
        }
        txn.commit()?;
        Ok(())
    }

    fn mandatory_fields(&mut self) {
        tracing::trace!("check mandatory fields");

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
            self.add_problem(ProblemKind::UnknownRecordType(
                self.record_type().to_string(),
            ));
        }

        if let Some(Err(_error)) = self.header.fields.get_date("WARC-Date") {
            self.add_problem(ProblemKind::InvalidDate("WARC-Date".to_string()));
        }
    }

    fn content_type(&mut self) {
        tracing::trace!("check content-type");

        if let Some(Err(_error)) = self.header.fields.get_media_type("Content-Type") {
            self.add_problem(ProblemKind::InvalidMediaType("Content-Type".to_string()));
        }
    }

    fn concurrent_to(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check concurrent-to");

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

    fn ip_address(&mut self) {
        tracing::trace!("check ip-address");

        if self.is_any_record_type(&["warcinfo", "conversion", "continuation"]) {
            self.prohibit_field("WARC-IP-Address");
        }

        if let Some(Err(_error)) = self.header.fields.get_ip_addr("WARC-IP-Address") {
            self.add_problem(ProblemKind::InvalidIpAddress("WARC-IP-Address".to_string()));
        }
    }

    fn refers_to(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check refers-to");

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
        tracing::trace!("check refers-to-target-uri");

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

        if let Some(Err(_error)) = self.header.fields.get_url("WARC-Refers-To-Target-URI") {
            self.add_problem(ProblemKind::InvalidUrl(
                "WARC-Refers-To-Target-URI".to_string(),
            ));
        }
    }

    fn refers_to_date(&mut self) {
        tracing::trace!("check refers-to-date");

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

        if let Some(Err(_error)) = self.header.fields.get_date("WARC-Refers-To-Date") {
            self.add_problem(ProblemKind::InvalidDate("WARC-Refers-To-Date".to_string()));
        }
    }

    fn target_uri(&mut self) {
        tracing::trace!("check target-uri");

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

        if let Some(Err(_error)) = self.header.fields.get_url("WARC-Target-URI") {
            self.add_problem(ProblemKind::InvalidUrl("WARC-Target-URI".to_string()));
        }
    }

    fn truncated(&mut self) {
        tracing::trace!("check truncated");

        if let Some(reason) = self.header.fields.get("WARC-Truncated") {
            if !["length", "time", "disconnect", "unspecified"].contains(&reason.as_str()) {
                self.add_problem(ProblemKind::InvalidTruncatedReason);
            }
        }
    }

    fn warcinfo_id(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check warcinfo-id");

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
        tracing::trace!("check filename");

        if !self.is_any_record_type(&["warcinfo"]) {
            self.prohibit_field("WARC-Filename");
        }
    }

    fn profile(&mut self) {
        tracing::trace!("check profile");

        if self.is_any_record_type(&["profile"]) {
            self.require_field("WARC-Profile");
        }
        if self.header.fields.is_formatted_bad_spec_url("WARC-Profile") {
            self.add_problem(ProblemKind::BadSpecUri("WARC-Target-URI".to_string()));
        }
    }

    fn segment(&mut self) -> Result<(), StorageError> {
        tracing::trace!("check segment");

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
            self.add_problem(ProblemKind::InvalidSegment);
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
        tracing::trace!("check block-digest");

        let mut pending_problems = Vec::new();

        for value in self.header.fields.get_all("WARC-Block-Digest") {
            if let Ok(digest) = Digest::from_str(value) {
                self.hashers.push(Hasher::new(digest.algorithm()));
                self.digests.insert(digest.algorithm(), digest);
            } else {
                pending_problems.push(ProblemKind::UnknownDigest(value.to_string()));
            }
        }

        for kind in pending_problems.into_iter() {
            self.add_problem(kind);
        }
    }

    fn payload_digest(&mut self) {
        tracing::trace!("check payload-digest");

        if self.header.fields.contains_name("WARC-Payload-Digest") {
            let mut extractor = WarcExtractor::new();
            if let Err(error) = extractor.read_header(&self.header) {
                self.add_problem(ProblemKind::ParsePayload(error.to_string()));

                return;
            }

            if extractor.has_content() {
                self.payload_extractor = Some(extractor);
            } else {
                return;
            }
        }

        let mut pending_problems = Vec::new();

        for value in self.header.fields.get_all("WARC-Payload-Digest") {
            if let Ok(digest) = Digest::from_str(value) {
                self.payload_hashers.push(Hasher::new(digest.algorithm()));
                self.payload_digests.insert(digest.algorithm(), digest);
            } else {
                pending_problems.push(ProblemKind::UnknownDigest(value.to_string()));
            }
        }

        for kind in pending_problems.into_iter() {
            self.add_problem(kind);
        }
    }

    /// Process the block data of a record.
    ///
    /// This function should be called until there is no more block data.
    /// Then, call [`end_record()`](Self::end_record).
    pub fn block_data(&mut self, data: &[u8]) {
        for hasher in &mut self.hashers {
            hasher.update(data);
        }

        let mut payload_extractor_error = false;
        if let Some(extractor) = &mut self.payload_extractor {
            let result = extractor.extract_data(data, &mut self.payload_extractor_buf);

            if let Err(error) = result {
                self.add_problem(ProblemKind::ParsePayload(error.to_string()));
                payload_extractor_error = true;
            }

            for hasher in &mut self.payload_hashers {
                hasher.update(&self.payload_extractor_buf);
            }
            self.payload_extractor_buf.clear();
        }

        if payload_extractor_error {
            self.payload_extractor = None
        }
    }

    /// Indicate the end of the record.
    ///
    /// Call [`begin_record()`](Self::begin_record) or [`verify_end()`](Self::verify_end) next.
    pub fn end_record(&mut self) {
        let mut hashers = std::mem::take(&mut self.hashers);

        tracing::trace!(hashers_len = hashers.len(), "verify block digests");

        for hasher in &mut hashers {
            let value = hasher.finish();

            let digest = self.digests.get(&hasher.algorithm()).unwrap();

            if digest.value() != value {
                self.add_problem(ProblemKind::DigestMismatch {
                    algorithm: hasher.algorithm().to_string(),
                    expected: HEXLOWER.encode(digest.value()),
                    actual: HEXLOWER.encode(&value),
                });
            }
        }

        self.hashers = hashers;

        let mut payload_hashers = std::mem::take(&mut self.payload_hashers);

        tracing::trace!(hashers_len = payload_hashers.len(), "verify payload digests");

        for hasher in &mut payload_hashers {
            let value = hasher.finish();

            let digest = self.payload_digests.get(&hasher.algorithm()).unwrap();

            if digest.value() != value {
                self.add_problem(ProblemKind::PayloadDigestMismatch {
                    algorithm: hasher.algorithm().to_string(),
                    expected: HEXLOWER.encode(digest.value()),
                    actual: HEXLOWER.encode(&value),
                });
            }
        }

        self.payload_hashers = payload_hashers;
    }

    pub fn add_not_record_at_time_compression(&mut self) {
        self.add_problem(ProblemKind::NotRecordAtTimeCompression);
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
