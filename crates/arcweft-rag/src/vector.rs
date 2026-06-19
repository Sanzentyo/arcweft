use arcweft_debug_model::{
    chunk::ChunkId,
    rag::{SearchChannel, SearchHit},
};
use thiserror::Error;

/// One candidate vector loaded from a storage adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorCandidate {
    pub chunk_id: ChunkId,
    pub values: Vec<f32>,
}

/// Invalid vector comparison input.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum VectorSearchError {
    #[error("vector dimensions differ: query={query}, candidate={candidate}")]
    DimensionMismatch { query: usize, candidate: usize },
    #[error("vector contains a non-finite value")]
    NonFinite,
    #[error("vector has zero norm")]
    ZeroNorm,
}

/// Computes cosine similarity without assuming either input is normalized.
pub fn cosine_similarity(query: &[f32], candidate: &[f32]) -> Result<f32, VectorSearchError> {
    if query.len() != candidate.len() {
        return Err(VectorSearchError::DimensionMismatch {
            query: query.len(),
            candidate: candidate.len(),
        });
    }
    if query
        .iter()
        .chain(candidate)
        .any(|value| !value.is_finite())
    {
        return Err(VectorSearchError::NonFinite);
    }

    let (dot, query_norm, candidate_norm) = query.iter().zip(candidate).fold(
        (0.0_f32, 0.0_f32, 0.0_f32),
        |(dot, query_norm, candidate_norm), (left, right)| {
            (
                dot + left * right,
                query_norm + left * left,
                candidate_norm + right * right,
            )
        },
    );

    if query_norm <= f32::EPSILON || candidate_norm <= f32::EPSILON {
        return Err(VectorSearchError::ZeroNorm);
    }
    Ok(dot / (query_norm.sqrt() * candidate_norm.sqrt()))
}

/// Ranks candidates by cosine score with a stable chunk-id tie break.
pub fn rank_vectors(
    query: &[f32],
    candidates: &[VectorCandidate],
    limit: usize,
) -> Result<Vec<SearchHit>, VectorSearchError> {
    let mut scored = candidates
        .iter()
        .map(|candidate| {
            cosine_similarity(query, &candidate.values)
                .map(|score| (candidate.chunk_id.clone(), score))
        })
        .collect::<Result<Vec<_>, _>>()?;

    scored.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });

    Ok(scored
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, (chunk_id, score))| SearchHit {
            chunk_id,
            channel: SearchChannel::Vector,
            rank: index + 1,
            score: Some(f64::from(score)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_ranking_is_stable() {
        let candidates = vec![
            VectorCandidate {
                chunk_id: ChunkId::new("b"),
                values: vec![1.0, 0.0],
            },
            VectorCandidate {
                chunk_id: ChunkId::new("a"),
                values: vec![1.0, 0.0],
            },
            VectorCandidate {
                chunk_id: ChunkId::new("c"),
                values: vec![0.0, 1.0],
            },
        ];
        let hits = rank_vectors(&[1.0, 0.0], &candidates, 3).expect("valid vectors");
        let ids = hits
            .iter()
            .map(|hit| hit.chunk_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }
}
