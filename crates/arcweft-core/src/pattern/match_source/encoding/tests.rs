use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, Eq, PartialEq)]
enum OutputError {
    Limit,
    Length,
}
impl From<MatchSourceEncodingError> for OutputError {
    fn from(_: MatchSourceEncodingError) -> Self {
        Self::Length
    }
}
struct LimitedSink {
    calls: Rc<Cell<usize>>,
    bytes: Rc<RefCell<Vec<u8>>>,
}
impl MatchSourceSink for LimitedSink {
    type Error = OutputError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.calls.set(self.calls.get() + 1);
        if self.bytes.borrow().len() + bytes.len() > 2 {
            return Err(OutputError::Limit);
        }
        self.bytes.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }
}
#[test]
fn failed_parent_stops_output_and_preserves_its_first_error() {
    let calls = Rc::new(Cell::new(0));
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let mut writer = MatchSourceWriter::new(LimitedSink {
        calls: Rc::clone(&calls),
        bytes: Rc::clone(&bytes),
    });
    writer.extend_from_slice(&[1, 2]);
    writer.push(3);
    writer.extend_from_slice(&[4; 32]);
    assert!(!writer.is_active());
    assert_eq!(writer.byte_len(), 2);
    assert_eq!(calls.get(), 2);
    assert_eq!(*bytes.borrow(), [1, 2]);
    assert!(matches!(
        writer.finish(Err(MatchSourceEncodingError::LengthOverflow)),
        Err(OutputError::Limit)
    ));
}
#[test]
fn memory_output_retains_exact_canonical_bytes() {
    let mut writer = MatchSourceWriter::new(Vec::new());
    writer.push(1);
    writer.extend_from_slice(&7_u64.to_le_bytes());
    assert_eq!(writer.byte_len(), 9);
    assert_eq!(writer.finish(Ok(())).unwrap(), [1, 7, 0, 0, 0, 0, 0, 0, 0]);
}
