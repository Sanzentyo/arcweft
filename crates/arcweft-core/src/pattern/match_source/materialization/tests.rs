use super::*;

fn empty_source<C>(
    coordinate: C,
) -> MatchSourceDefinition<
    C,
    MatchSourceDataIdentity,
    MatchSourceDataIdentity,
    MatchSourceDataArm,
    MatchSourceDataCoverage,
> {
    MatchSourceDefinition::new(
        coordinate,
        MatchSourceDataIdentity([1; 32]),
        MatchSourceDataIdentity([2; 32]),
        Box::new([]),
        MatchSourceCoverage::new(true, MatchSourceDataIdentity([3; 32]), Box::new([])),
    )
}

struct FailingCoordinate;
impl CanonicalMatchSource for FailingCoordinate {
    fn write_source<W: MatchSourceSink>(
        &self,
        output: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError> {
        output.push(1);
        Err(MatchSourceEncodingError::LengthOverflow)
    }
}

#[test]
fn first_parent_failure_survives_a_later_coordinate_encoding_error() {
    let source = empty_source(FailingCoordinate);
    let limit = u64::try_from(size_of::<MatchSourceData>()).unwrap();
    let mut budget = MatchSourceMaterializationBudget::new(limit);
    let error = source.materialize_source(&mut budget).unwrap_err();
    assert_eq!(
        error,
        MatchSourceMaterializationError::LimitExceeded {
            limit,
            attempted: limit + 1
        }
    );
    assert_eq!(source.preflight_source(&mut budget), Err(error));
    assert_eq!(budget.used_bytes(), limit);
}

struct GrowingCoordinate(std::cell::Cell<usize>);
impl CanonicalMatchSource for GrowingCoordinate {
    fn write_source<W: MatchSourceSink>(
        &self,
        output: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError> {
        let call = self.0.get();
        self.0.set(call + 1);
        output.extend_from_slice(if call == 0 { &[1] } else { &[1, 2] });
        Ok(())
    }
}

#[test]
fn changed_coordinate_length_cannot_expand_its_original_copy_reservation() {
    let source = empty_source(GrowingCoordinate(std::cell::Cell::new(0)));
    let mut budget = MatchSourceMaterializationBudget::new(u64::MAX);
    let error = source.materialize_source(&mut budget).unwrap_err();
    assert_eq!(
        error,
        MatchSourceMaterializationError::ChangedCoordinateEncoding
    );
    assert_eq!(source.coordinate().0.get(), 2);
    assert_eq!(source.preflight_source(&mut budget), Err(error));
    assert_eq!(source.coordinate().0.get(), 2);
}

fn definition() -> MatchSourceData {
    let coordinate = || MatchSourceDataCoordinate(vec![1, 2, 3].into_boxed_slice());
    let id = |tag| MatchSourceDataIdentity([tag; 32]);
    MatchSourceDefinition::new(
        coordinate(),
        id(1),
        id(2),
        vec![MatchSourceArm::new(
            MatchSourceArmCoordinate::new(coordinate(), 0),
            id(3),
            MatchSourceGuard::Dynamic(id(4)),
            MatchSourceArmResult::Body(id(5)),
            vec![MatchSourceBinding::new(coordinate(), id(6))].into_boxed_slice(),
        )]
        .into_boxed_slice(),
        MatchSourceCoverage::new(
            true,
            id(7),
            vec![MatchSourceUnreachable::new(
                MatchSourceArmCoordinate::new(coordinate(), 0),
                Some(coordinate()),
                MatchSourceUnreachableReason::CoveredByEarlierOrAlternative,
            )]
            .into_boxed_slice(),
        ),
    )
}

fn storage_bytes(source: &MatchSourceData) -> u64 {
    let mut budget = MatchSourceMaterializationBudget::new(u64::MAX);
    source.preflight_source(&mut budget).unwrap();
    budget.used
}

#[test]
fn materialization_preserves_complete_definition_and_canonical_transcript() {
    let original = definition();
    let mut budget = MatchSourceMaterializationBudget::new(storage_bytes(&original));
    let retained = original.materialize_source(&mut budget).unwrap();
    assert_eq!(retained, original);
    let encode = |source: &MatchSourceData| {
        let mut writer = MatchSourceWriter::new(Vec::new());
        let encoded = source.write_semantic(&mut writer);
        writer.finish(encoded).unwrap()
    };
    assert_eq!(encode(&original), encode(&retained));
}

#[test]
fn source_budget_is_shared_and_a_failure_cannot_reset_it() {
    let original = definition();
    let bytes = storage_bytes(&original);
    let mut exact = MatchSourceMaterializationBudget::new(bytes * 2);
    original.materialize_source(&mut exact).unwrap();
    original.materialize_source(&mut exact).unwrap();
    assert_eq!(exact.used, bytes * 2);
    let first_error = original.materialize_source(&mut exact).unwrap_err();
    assert!(matches!(
        first_error,
        MatchSourceMaterializationError::LimitExceeded { .. }
    ));
    assert_eq!(
        original.materialize_source(&mut exact).unwrap_err(),
        first_error
    );
    assert_eq!(exact.used, bytes * 2);
}

#[test]
fn source_budget_rejects_the_last_coordinate_byte_before_materialization() {
    let original = definition();
    let bytes = storage_bytes(&original);
    let mut budget = MatchSourceMaterializationBudget::new(bytes - 1);
    assert!(matches!(original.materialize_source(&mut budget),
        Err(MatchSourceMaterializationError::LimitExceeded { attempted, limit })
        if attempted == bytes && limit == bytes - 1));
    assert!(budget.used < bytes);
}
