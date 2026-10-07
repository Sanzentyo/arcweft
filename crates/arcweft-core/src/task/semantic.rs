//! Checked version-one byte grammar for task semantic transcripts.
//!
//! The owner supplies closed tags and ordered fields. All intermediate/final
//! transcripts borrow one meter; the first error poisons the entire seal.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskSemanticEncodingError {
    CountOverflow,
    StringLengthOverflow,
    ArithmeticOverflow,
    SemanticWork,
    TranscriptBytes,
    OwnerRejected,
}

pub(crate) struct TaskSemanticMeter {
    max_work: u64,
    max_bytes: u64,
    work: u64,
    bytes: u64,
    error: Option<TaskSemanticEncodingError>,
}

impl TaskSemanticMeter {
    pub(crate) const fn new(max_work: u64, max_bytes: u64) -> Self {
        Self {
            max_work,
            max_bytes,
            work: 0,
            bytes: 0,
            error: None,
        }
    }

    fn reject(&mut self, error: TaskSemanticEncodingError) -> TaskSemanticEncodingError {
        *self.error.get_or_insert(error)
    }

    pub(crate) const fn status(&self) -> Result<(), TaskSemanticEncodingError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn charge_work(&mut self, units: u64) -> Result<(), TaskSemanticEncodingError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(next) = self.work.checked_add(units) else {
            return Err(self.reject(TaskSemanticEncodingError::ArithmeticOverflow));
        };
        if next > self.max_work {
            return Err(self.reject(TaskSemanticEncodingError::SemanticWork));
        }
        self.work = next;
        Ok(())
    }

    pub(crate) fn preflight_bytes(&mut self, bytes: u64) -> Result<(), TaskSemanticEncodingError> {
        self.checked_byte_total(bytes).map(|_| ())
    }

    fn checked_byte_total(&mut self, bytes: u64) -> Result<u64, TaskSemanticEncodingError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(next) = self.bytes.checked_add(bytes) else {
            return Err(self.reject(TaskSemanticEncodingError::ArithmeticOverflow));
        };
        if next > self.max_bytes {
            return Err(self.reject(TaskSemanticEncodingError::TranscriptBytes));
        }
        Ok(next)
    }

    fn charge_bytes(&mut self, bytes: usize) -> Result<(), TaskSemanticEncodingError> {
        let count = u64::try_from(bytes)
            .map_err(|_| self.reject(TaskSemanticEncodingError::ArithmeticOverflow))?;
        self.bytes = self.checked_byte_total(count)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn totals(&self) -> (u64, u64) {
        (self.work, self.bytes)
    }
}

pub(crate) struct TaskSemanticEncoder<'a> {
    hasher: blake3::Hasher,
    meter: &'a mut TaskSemanticMeter,
}

impl<'a> TaskSemanticEncoder<'a> {
    pub(crate) fn new(domain: &'static [u8], meter: &'a mut TaskSemanticMeter) -> Self {
        let mut encoder = Self {
            hasher: blake3::Hasher::new(),
            meter,
        };
        encoder.write(domain);
        encoder
    }

    /// Domain owners charge entry into a source-ordered list separately from
    /// emission of that element's integer/tag/digest fields.
    pub(crate) fn enter_element(&mut self) {
        self.enter_role();
    }

    pub(crate) fn enter_role(&mut self) {
        let _ = self.meter.charge_work(1);
    }

    pub(crate) const fn status(&self) -> Result<(), TaskSemanticEncodingError> {
        self.meter.status()
    }

    pub(crate) fn reject_owner(&mut self) {
        self.meter.reject(TaskSemanticEncodingError::OwnerRejected);
    }

    pub(crate) fn tag(&mut self, tag: u8) {
        self.atom(&[tag]);
    }

    pub(crate) fn identity_128(&mut self, identity: &[u8; 16]) {
        self.atom(identity);
    }

    pub(crate) fn scalar_u64(&mut self, value: u64) {
        self.atom(&value.to_le_bytes());
    }

    pub(crate) fn scalar_u128(&mut self, value: u128) {
        self.atom(&value.to_le_bytes());
    }

    pub(crate) fn ordinal(&mut self, ordinal: u32) {
        self.atom(&ordinal.to_le_bytes());
    }

    pub(crate) fn digest(&mut self, digest: &[u8; 32]) {
        self.atom(digest);
    }

    pub(crate) fn count(&mut self, count: usize) {
        self.length(count, TaskSemanticEncodingError::CountOverflow);
    }

    pub(crate) fn string(&mut self, value: &str) {
        let Ok(length) = u32::try_from(value.len()) else {
            self.meter
                .reject(TaskSemanticEncodingError::StringLengthOverflow);
            return;
        };
        let chunks = u64::from(length).div_ceil(32);
        if self.meter.charge_work(1).is_err() || self.meter.charge_work(chunks).is_err() {
            return;
        }
        self.atom(&length.to_le_bytes());
        self.write(value.as_bytes());
    }

    fn length(&mut self, value: usize, error: TaskSemanticEncodingError) {
        if self.meter.error.is_some() {
            return;
        }
        match u32::try_from(value) {
            Ok(value) => self.atom(&value.to_le_bytes()),
            Err(_) => {
                self.meter.reject(error);
            }
        }
    }

    fn atom(&mut self, bytes: &[u8]) {
        if self.meter.charge_work(1).is_ok() {
            self.write(bytes);
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.meter.charge_bytes(bytes.len()).is_ok() {
            self.hasher.update(bytes);
        }
    }

    pub(crate) fn finish(self) -> Result<blake3::Hash, TaskSemanticEncodingError> {
        match self.meter.error {
            Some(error) => Err(error),
            None => Ok(self.hasher.finalize()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter() -> TaskSemanticMeter {
        TaskSemanticMeter::new(4_194_304, 67_108_864)
    }

    #[test]
    fn source_ordinal_is_one_little_endian_atom_with_exact_limits() {
        let mut meter = TaskSemanticMeter::new(1, 5);
        let mut encoder = TaskSemanticEncoder::new(b"d", &mut meter);
        encoder.ordinal(0x0403_0201);
        assert_eq!(encoder.finish().unwrap(), blake3::hash(&[b'd', 1, 2, 3, 4]));
        assert_eq!(meter.totals(), (1, 5));
        let mut short = TaskSemanticMeter::new(0, 5);
        let mut encoder = TaskSemanticEncoder::new(b"d", &mut short);
        encoder.ordinal(0);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::SemanticWork)
        );
        assert_eq!(short.totals(), (0, 1));
    }

    #[test]
    fn semantic_string_lengths_count_utf8_bytes_with_u32_le() {
        let mut meter = meter();
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0", &mut meter);
        encoder.tag(7);
        encoder.string("é");
        encoder.count(1);
        encoder.digest(&[9; 32]);
        let mut expected = b"task-test.v1\0".to_vec();
        expected.push(7);
        expected.extend_from_slice(&2_u32.to_le_bytes());
        expected.extend_from_slice("é".as_bytes());
        expected.extend_from_slice(&1_u32.to_le_bytes());
        expected.extend_from_slice(&[9; 32]);
        assert_eq!(encoder.finish().unwrap(), blake3::hash(&expected));
    }

    #[test]
    fn invalid_count_prevents_digest_publication_after_later_writes() {
        if usize::BITS <= u32::BITS {
            return;
        }
        let mut meter = meter();
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0", &mut meter);
        encoder.count(usize::MAX);
        encoder.string("otherwise valid");
        encoder.digest(&[9; 32]);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::CountOverflow)
        );
    }

    #[test]
    fn invalid_string_length_retains_the_first_error() {
        if usize::BITS <= u32::BITS {
            return;
        }
        let mut meter = meter();
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0", &mut meter);
        encoder.length(usize::MAX, TaskSemanticEncodingError::StringLengthOverflow);
        encoder.count(usize::MAX);
        encoder.tag(0);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::StringLengthOverflow)
        );
    }

    #[test]
    fn exact_atom_work_and_domain_bytes_pass_at_limit_and_reject_the_next_atom() {
        for (work, bytes, expected) in [
            (2, 34, None),
            (1, 34, Some(TaskSemanticEncodingError::SemanticWork)),
            (2, 33, Some(TaskSemanticEncodingError::TranscriptBytes)),
        ] {
            let mut meter = TaskSemanticMeter::new(work, bytes);
            let mut encoder = TaskSemanticEncoder::new(b"d", &mut meter);
            encoder.tag(7);
            encoder.digest(&[9; 32]);
            match expected {
                None => {
                    let mut bytes = vec![b'd', 7];
                    bytes.extend_from_slice(&[9; 32]);
                    assert_eq!(encoder.finish().unwrap(), blake3::hash(&bytes));
                    assert_eq!(meter.totals(), (2, 34));
                }
                Some(error) => assert_eq!(encoder.finish(), Err(error)),
            }
        }
    }

    #[test]
    fn utf8_work_counts_string_entry_length_atom_and_rounded_byte_chunks() {
        for text in [
            "",
            "a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "あああああああああああ",
        ] {
            let bytes = 1 + 4 + u64::try_from(text.len()).unwrap();
            let work = 2 + u64::try_from(text.len()).unwrap().div_ceil(32);
            let mut exact = TaskSemanticMeter::new(work, bytes);
            let mut encoder = TaskSemanticEncoder::new(b"d", &mut exact);
            encoder.string(text);
            let mut expected = vec![b'd'];
            expected.extend_from_slice(&u32::try_from(text.len()).unwrap().to_le_bytes());
            expected.extend_from_slice(text.as_bytes());
            assert_eq!(encoder.finish().unwrap(), blake3::hash(&expected));
            assert_eq!(exact.totals(), (work, bytes));
            let mut short = TaskSemanticMeter::new(work - 1, bytes);
            let mut encoder = TaskSemanticEncoder::new(b"d", &mut short);
            encoder.string(text);
            assert_eq!(
                encoder.finish(),
                Err(TaskSemanticEncodingError::SemanticWork)
            );
        }
    }

    #[test]
    fn intermediate_transcripts_share_cumulative_limits_and_the_first_error() {
        let mut meter = TaskSemanticMeter::new(2, 3);
        let mut first = TaskSemanticEncoder::new(b"d", &mut meter);
        first.tag(1);
        first.finish().unwrap();
        let mut second = TaskSemanticEncoder::new(b"d", &mut meter);
        second.tag(2);
        assert_eq!(
            second.finish(),
            Err(TaskSemanticEncodingError::TranscriptBytes)
        );
        let before = meter.totals();
        let mut third = TaskSemanticEncoder::new(b"later domain", &mut meter);
        third.tag(3); // would otherwise also exceed the work limit
        assert_eq!(
            third.finish(),
            Err(TaskSemanticEncodingError::TranscriptBytes)
        );
        assert_eq!(meter.totals(), before);
    }

    #[test]
    fn arithmetic_overflow_precedes_limit_errors_without_counter_mutation() {
        let mut work = TaskSemanticMeter::new(u64::MAX, u64::MAX);
        work.charge_work(u64::MAX).unwrap();
        assert_eq!(
            work.charge_work(1),
            Err(TaskSemanticEncodingError::ArithmeticOverflow)
        );
        assert_eq!(work.totals(), (u64::MAX, 0));
        let mut bytes = TaskSemanticMeter::new(u64::MAX, u64::MAX);
        // Synthetic scalar state tests checked arithmetic without allocating
        // a u64::MAX-byte transcript.
        bytes.bytes = u64::MAX;
        assert_eq!(
            bytes.preflight_bytes(1),
            Err(TaskSemanticEncodingError::ArithmeticOverflow)
        );
        assert_eq!(bytes.totals(), (0, u64::MAX));
    }

    #[test]
    fn count_conversion_precedes_atom_work_and_preserves_the_rejection() {
        if usize::BITS <= u32::BITS {
            return;
        }
        let mut meter = TaskSemanticMeter::new(0, 1);
        let mut encoder = TaskSemanticEncoder::new(b"d", &mut meter);
        encoder.count(usize::MAX);
        encoder.tag(0);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::CountOverflow)
        );
        assert_eq!(meter.totals(), (0, 1));
    }
}
