use std::io::Cursor;

use super::loader::read_bytes_bounded;

#[test]
fn topology_disk_reader_retains_only_remaining_plus_one_evidence() {
    let source = [b'x'; 64];
    let mut reader = Cursor::new(source);

    let bytes = read_bytes_bounded(&mut reader, 7).expect("bounded read");

    assert_eq!(bytes.len(), 8);
    assert_eq!(reader.position(), 8);
}

#[test]
fn topology_disk_reader_accepts_the_exact_remaining_bytes() {
    let source = [b'x'; 7];
    let mut reader = Cursor::new(source);

    let bytes = read_bytes_bounded(&mut reader, 7).expect("exact bounded read");

    assert_eq!(bytes.len(), 7);
    assert_eq!(reader.position(), 7);
}
