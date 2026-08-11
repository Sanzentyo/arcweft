use crate::{
    ArgumentError, ArgumentProduct, ArgumentValue, Coordinate, DeclarationDigest, DefinitionId,
    GenerationId, GroupIndex, LiveGenerations, OwnershipClass, Signature, SignatureFingerprint,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluatedGroup {
    pub group: GroupIndex,
    pub coordinates: Vec<Coordinate>,
    pub values: Vec<ArgumentValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalStreamPartial {
    pub definition: DefinitionId,
    pub declaration: DeclarationDigest,
    pub generation: GenerationId,
    pub signature: SignatureFingerprint,
    pub next_group: GroupIndex,
    pub captured: ArgumentProduct,
    pub ownership: OwnershipClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRequest {
    pub instance: u64,
    pub arguments: ArgumentProduct,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyOutcome {
    Partial(ExternalStreamPartial),
    Open(OpenRequest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyError {
    Argument(ArgumentError),
    GroupNotNext { expected: u16, actual: u16 },
    InstanceIdOverflow,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeState {
    pub next_instance: u64,
    pub open_requests: Vec<OpenRequest>,
}

impl ExternalStreamPartial {
    pub fn initial(
        signature: &Signature,
        generation: GenerationId,
        live: &LiveGenerations,
    ) -> Result<Self, ArgumentError> {
        let captured = ArgumentProduct::empty(signature, generation);
        let ownership = captured.validate_prefix(signature, live)?;
        Ok(Self {
            definition: signature.definition,
            declaration: signature.declaration,
            generation,
            signature: signature.fingerprint,
            next_group: GroupIndex(0),
            captured,
            ownership,
        })
    }

    pub fn apply(
        &self,
        signature: &Signature,
        group: &EvaluatedGroup,
        live: &LiveGenerations,
        state: &mut RuntimeState,
    ) -> Result<ApplyOutcome, ApplyError> {
        self.captured
            .validate_prefix(signature, live)
            .map_err(ApplyError::Argument)?;
        if self.next_group.0 != self.captured.completed_groups {
            return Err(ApplyError::GroupNotNext {
                expected: self.captured.completed_groups,
                actual: self.next_group.0,
            });
        }
        if group.group != self.next_group {
            return Err(ApplyError::GroupNotNext {
                expected: self.next_group.0,
                actual: group.group.0,
            });
        }
        if group.coordinates.len() != group.values.len() {
            return Err(ApplyError::Argument(
                ArgumentError::CoordinateValueLengthMismatch,
            ));
        }

        let mut candidate = self.captured.clone();
        candidate.completed_groups = candidate
            .completed_groups
            .checked_add(1)
            .ok_or(ApplyError::Argument(
                ArgumentError::CompletedGroupsOutOfRange,
            ))?;
        candidate.coordinates.extend(group.coordinates.iter().copied());
        candidate.values.extend(group.values.iter().cloned());
        let ownership = candidate
            .validate_prefix(signature, live)
            .map_err(ApplyError::Argument)?;

        if candidate.completed_groups == signature.group_count() {
            candidate
                .validate_complete(signature, live)
                .map_err(ApplyError::Argument)?;
            if state.next_instance == u64::MAX {
                return Err(ApplyError::InstanceIdOverflow);
            }
            let request = OpenRequest {
                instance: state.next_instance,
                arguments: candidate,
            };
            state.next_instance += 1;
            state.open_requests.push(request.clone());
            Ok(ApplyOutcome::Open(request))
        } else {
            Ok(ApplyOutcome::Partial(Self {
                definition: self.definition,
                declaration: self.declaration,
                generation: self.generation,
                signature: self.signature,
                next_group: GroupIndex(candidate.completed_groups),
                captured: candidate,
                ownership,
            }))
        }
    }
}
