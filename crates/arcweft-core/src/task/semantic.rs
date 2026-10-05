//! Checked version-one byte grammar for task semantic transcripts.
//!
//! The owner supplies its closed tags and ordered fields. A rejected length
//! poisons the transcript, so later writes cannot publish a partial digest.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskSemanticEncodingError {
    CountOverflow,
    StringLengthOverflow,
}

pub(super) struct TaskSemanticEncoder {
    hasher: blake3::Hasher,
    error: Option<TaskSemanticEncodingError>,
}

impl TaskSemanticEncoder {
    pub(super) fn new(domain: &'static [u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        Self {
            hasher,
            error: None,
        }
    }

    pub(super) fn tag(&mut self, tag: u8) {
        self.write(&[tag]);
    }

    pub(super) fn digest(&mut self, digest: &[u8; 32]) {
        self.write(digest);
    }

    pub(super) fn count(&mut self, count: usize) {
        self.length(count, TaskSemanticEncodingError::CountOverflow);
    }

    pub(super) fn string(&mut self, value: &str) {
        self.length(value.len(), TaskSemanticEncodingError::StringLengthOverflow);
        self.write(value.as_bytes());
    }

    fn length(&mut self, value: usize, error: TaskSemanticEncodingError) {
        if self.error.is_some() {
            return;
        }
        match u32::try_from(value) {
            Ok(value) => self.write(&value.to_le_bytes()),
            Err(_) => self.error = Some(error),
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.error.is_none() {
            self.hasher.update(bytes);
        }
    }

    pub(super) fn finish(self) -> Result<blake3::Hash, TaskSemanticEncodingError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.hasher.finalize()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_string_lengths_count_utf8_bytes_with_u32_le() {
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0");
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
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0");
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
        let mut encoder = TaskSemanticEncoder::new(b"task-test.v1\0");
        encoder.length(usize::MAX, TaskSemanticEncodingError::StringLengthOverflow);
        encoder.count(usize::MAX);
        encoder.tag(0);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::StringLengthOverflow)
        );
    }
}
