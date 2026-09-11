use super::*;
use crate::awbc::codec::AwbcDecodeBudget;

#[test]
fn deep_unary_schemas_fail_at_the_selected_depth_bound() {
    let budget = AwbcDecodeBudget {
        nesting_depth: 8,
        ..AwbcDecodeBudget::default()
    };
    for tag in [19, 20] {
        let mut bytes = vec![tag; 20_000];
        bytes.push(0);
        let mut reader = Reader::new(&bytes, &budget);
        assert!(matches!(
            RuntimeTypeSchema::read_wire(&mut reader),
            Err(AwbcCodecError::NestingDepthExceeded { limit: 8 })
        ));
        assert_eq!(reader.offset(), 8);
    }
}

#[test]
fn schema_depth_counts_root_and_children_inclusively() {
    let bytes = [19, 20, 1];
    let budget = AwbcDecodeBudget {
        nesting_depth: 3,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&bytes, &budget);
    let schema = RuntimeTypeSchema::read_wire(&mut reader).unwrap();
    reader.finish().unwrap();
    assert_eq!(
        schema,
        RuntimeTypeSchema::Option(Box::new(RuntimeTypeSchema::Seq(Box::new(
            RuntimeTypeSchema::Bool
        ))))
    );
    let mut writer = Writer::default();
    schema.write_wire(&mut writer).unwrap();
    assert_eq!(writer.into_bytes(), bytes);

    let budget = AwbcDecodeBudget {
        nesting_depth: 2,
        ..budget
    };
    let mut reader = Reader::new(&bytes, &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 2 })
    ));
}

#[test]
fn failed_schema_and_collection_reads_restore_the_nesting_scope() {
    let budget = AwbcDecodeBudget {
        nesting_depth: 2,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&[19, 19, 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 2 })
    ));
    assert_eq!(
        RuntimeTypeSchema::read_wire(&mut reader).unwrap(),
        RuntimeTypeSchema::Bool
    );
    reader.finish().unwrap();

    let mut reader = Reader::new(&[19, 1, 0], &budget);
    assert!(matches!(
        reader.read_items::<RuntimeTypeSchema>(2),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 2 })
    ));
    assert_eq!(
        RuntimeTypeSchema::read_wire(&mut reader).unwrap(),
        RuntimeTypeSchema::Bool
    );
    assert_eq!(
        RuntimeTypeSchema::read_wire(&mut reader).unwrap(),
        RuntimeTypeSchema::Unit
    );
    reader.finish().unwrap();
}
