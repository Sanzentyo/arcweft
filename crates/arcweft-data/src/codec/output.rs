use std::{fmt, io};

use crate::{DataError, Result};

/// Bounded, append-only codec output. Construction and publication belong to
/// `EncodeOptions`; a codec only borrows this memory buffer while encoding.
/// The first failed append is sticky, including after a serializer catches it.
#[derive(Debug)]
pub struct EncodeBuffer {
    bytes: Vec<u8>,
    maximum_len: usize,
    failure: Option<DataError>,
}

impl EncodeBuffer {
    pub(super) const fn new(maximum_len: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_len,
            failure: None,
        }
    }

    /// Checks the complete append before reserving or copying any of its bytes.
    pub fn append(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.append_checked(bytes);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    pub fn push(&mut self, byte: u8) -> Result<()> {
        self.append(&[byte])
    }

    fn append_checked(&mut self, bytes: &[u8]) -> Result<()> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| DataError::limit("encoded output length overflow"))?;
        if length > self.maximum_len {
            return Err(DataError::limit(format!(
                "encoded output length {length} exceeds {}",
                self.maximum_len
            )));
        }
        if length > self.bytes.capacity() {
            let capacity = length
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.maximum_len);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|error| {
                    DataError::limit(format!("codec output allocation failed: {error}"))
                })?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub(super) fn finish(self, result: Result<()>) -> Result<Vec<u8>> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        result?;
        Ok(self.bytes)
    }
}

impl io::Write for EncodeBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.append(bytes).map_err(io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match &self.failure {
            Some(error) => Err(io::Error::other(error.clone())),
            None => Ok(()),
        }
    }
}

impl fmt::Write for EncodeBuffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.append(text.as_bytes()).map_err(|_| fmt::Error)
    }
}

#[cfg(test)]
mod tests;
