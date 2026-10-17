//! Borrowed admission of the canonical asset payloads, before materialization.

use super::{
    RUNTIME_BUNDLE_ASSET_VALUE_VERSION, RuntimeAssetContentDigest,
    RuntimeBundleAssetArtifactDigest, RuntimeBundleAssetBinding, RuntimeBundleAssetContext,
    RuntimeBundleAssetFailure, RuntimeBundleAssetFailureReason, RuntimeBundleAssetResourceId,
    RuntimeBundleAssetValueError, decode_digest, decode_option_digest, decode_string, decode_u8,
    decode_u64, invalid_payload,
};
use crate::task::GenerationId;
use crate::value::RuntimeValueView;

/// Fixed metadata and a borrowed identity from one admitted payload. Validation
/// cannot accidentally allocate an owned copy of its untrusted identity text.
pub(super) struct ParsedAssetBinding<'a> {
    context: RuntimeBundleAssetContext,
    id: &'a str,
    content_digest: RuntimeAssetContentDigest,
}

impl<'a> ParsedAssetBinding<'a> {
    pub(super) fn parse(value: RuntimeValueView<'a>) -> Result<Self, RuntimeBundleAssetValueError> {
        let RuntimeValueView::Tuple(fields) = value else {
            return Err(invalid_payload("asset handle"));
        };
        if fields.len() != 5 {
            return Err(invalid_payload("asset handle"));
        }
        let [
            Some(version),
            Some(generation),
            Some(artifact),
            Some(id),
            Some(content_digest),
        ] = [
            fields.get(0),
            fields.get(1),
            fields.get(2),
            fields.get(3),
            fields.get(4),
        ]
        else {
            return Err(invalid_payload("asset handle"));
        };
        if decode_u8(version)? != RUNTIME_BUNDLE_ASSET_VALUE_VERSION {
            return Err(invalid_payload("asset handle"));
        }
        let context = RuntimeBundleAssetContext::new(
            GenerationId::new(decode_u64(generation)?),
            RuntimeBundleAssetArtifactDigest::try_from_bytes(decode_digest(artifact)?)?,
        );
        let id = decode_string(id)?;
        RuntimeBundleAssetResourceId::validate_text(id)?;
        Ok(Self {
            context,
            id,
            content_digest: RuntimeAssetContentDigest::try_from_bytes(decode_digest(
                content_digest,
            )?)?,
        })
    }

    pub(super) fn materialize(
        self,
    ) -> Result<RuntimeBundleAssetBinding, RuntimeBundleAssetValueError> {
        RuntimeBundleAssetBinding::try_new(
            self.context,
            RuntimeBundleAssetResourceId::try_new(self.id)?,
            self.content_digest,
        )
    }

    #[cfg(test)]
    pub(super) const fn id(&self) -> &'a str {
        self.id
    }
}

/// Borrowed failure evidence obeys the same reason/digest relation as the owned
/// carrier, without constructing a temporary `RuntimeBundleAssetFailure`.
pub(super) struct ParsedAssetFailure<'a> {
    context: RuntimeBundleAssetContext,
    id: &'a str,
    reason: RuntimeBundleAssetFailureReason,
    content_digest: Option<RuntimeAssetContentDigest>,
}

impl<'a> ParsedAssetFailure<'a> {
    pub(super) fn parse(value: RuntimeValueView<'a>) -> Result<Self, RuntimeBundleAssetValueError> {
        let RuntimeValueView::Tuple(fields) = value else {
            return Err(invalid_payload("asset error"));
        };
        if fields.len() != 6 {
            return Err(invalid_payload("asset error"));
        }
        let [
            Some(version),
            Some(generation),
            Some(artifact),
            Some(id),
            Some(reason),
            Some(digest),
        ] = [
            fields.get(0),
            fields.get(1),
            fields.get(2),
            fields.get(3),
            fields.get(4),
            fields.get(5),
        ]
        else {
            return Err(invalid_payload("asset error"));
        };
        if decode_u8(version)? != RUNTIME_BUNDLE_ASSET_VALUE_VERSION {
            return Err(invalid_payload("asset error"));
        }
        let context = RuntimeBundleAssetContext::new(
            GenerationId::new(decode_u64(generation)?),
            RuntimeBundleAssetArtifactDigest::try_from_bytes(decode_digest(artifact)?)?,
        );
        let id = decode_string(id)?;
        RuntimeBundleAssetResourceId::validate_text(id)?;
        let reason = RuntimeBundleAssetFailureReason::from_encoded(decode_u8(reason)?)
            .ok_or_else(|| invalid_payload("asset error"))?;
        let content_digest = decode_option_digest(digest)?;
        RuntimeBundleAssetFailure::validate_evidence(reason, content_digest)?;
        Ok(Self {
            context,
            id,
            reason,
            content_digest,
        })
    }

    pub(super) fn materialize(
        self,
    ) -> Result<RuntimeBundleAssetFailure, RuntimeBundleAssetValueError> {
        RuntimeBundleAssetFailure::try_new(
            self.context,
            RuntimeBundleAssetResourceId::try_new(self.id)?,
            self.reason,
            self.content_digest,
        )
    }

    #[cfg(test)]
    pub(super) const fn id(&self) -> &'a str {
        self.id
    }
}
