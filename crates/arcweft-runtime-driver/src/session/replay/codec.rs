//! Bounded trace decoding through the existing strict typed JSON save codec.

use super::model::{
    ROOT_REPLAY_SAVE_SCHEMA_ID, ROOT_REPLAY_SCHEMA_VERSION, RootReplayError, RootReplayReportV1,
    RootReplayTraceV1,
};
use crate::session::{ArcweftBundle, BundleSession, BundleSessionOptions};
use arcweft_save::{SaveDecodeOptions, SaveSchemaId};

impl BundleSession {
    /// Decodes an inert trace under the caller's complete wire policy, then
    /// authenticates and replays it against the exact logical bundle.
    pub fn replay_root_trace_from_save_bytes(
        bundle: &ArcweftBundle,
        options: BundleSessionOptions,
        trace_bytes: &[u8],
        decode_options: &SaveDecodeOptions,
    ) -> Result<RootReplayReportV1, RootReplayError> {
        let trace = decode_trace(trace_bytes, decode_options)?;
        Self::replay_root_trace(bundle, options, &trace)
    }

    /// Binds independently decoded trace bytes to the exact AWFB artifact.
    pub fn replay_root_trace_from_awfb_and_trace_bytes(
        awfb_bytes: &[u8],
        options: BundleSessionOptions,
        trace_bytes: &[u8],
        decode_options: &SaveDecodeOptions,
    ) -> Result<RootReplayReportV1, RootReplayError> {
        let trace = decode_trace(trace_bytes, decode_options)?;
        Self::replay_root_trace_from_awfb_bytes(awfb_bytes, options, &trace)
    }
}

fn decode_trace(
    bytes: &[u8],
    options: &SaveDecodeOptions,
) -> Result<RootReplayTraceV1, RootReplayError> {
    arcweft_save::decode_strict_typed_json_save(
        bytes,
        &SaveSchemaId::new(ROOT_REPLAY_SAVE_SCHEMA_ID),
        ROOT_REPLAY_SCHEMA_VERSION,
        options,
    )
    .map_err(|error| RootReplayError::Codec(Box::new(error)))
}
