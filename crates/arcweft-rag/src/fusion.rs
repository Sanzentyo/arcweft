use arcweft_debug_model::{
    chunk::ChunkId,
    rag::{FusedSearchHit, SearchChannel, SearchHit},
};
use std::collections::{BTreeMap, BTreeSet};

/// Per-channel weights and the reciprocal-rank constant.
#[derive(Clone, Debug, PartialEq)]
pub struct FusionConfig {
    pub rrf_k: f64,
    pub weights: BTreeMap<SearchChannel, f64>,
}

/// Deterministically fuses ranked lists with weighted reciprocal rank fusion.
pub fn reciprocal_rank_fusion(
    ranked_lists: &[Vec<SearchHit>],
    config: &FusionConfig,
    limit: usize,
) -> Vec<FusedSearchHit> {
    let mut scores = BTreeMap::<ChunkId, (f64, BTreeSet<SearchChannel>)>::new();

    for hit in ranked_lists.iter().flat_map(|list| list.iter()) {
        let weight = config.weights.get(&hit.channel).copied().unwrap_or(1.0);
        let rank = u32::try_from(hit.rank).unwrap_or(u32::MAX);
        let contribution = weight / (config.rrf_k + f64::from(rank));
        let entry = scores
            .entry(hit.chunk_id.clone())
            .or_insert_with(|| (0.0, BTreeSet::new()));
        entry.0 += contribution;
        entry.1.insert(hit.channel);
    }

    let mut fused = scores
        .into_iter()
        .map(|(chunk_id, (fused_score, channels))| FusedSearchHit {
            chunk_id,
            fused_score,
            channels,
        })
        .collect::<Vec<_>>();

    fused.sort_by(|left, right| {
        right
            .fused_score
            .total_cmp(&left.fused_score)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    fused.truncate(limit);
    fused
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            rrf_k: 60.0,
            weights: BTreeMap::from([
                (SearchChannel::ExactEntity, 1.4),
                (SearchChannel::Lexical, 1.0),
                (SearchChannel::Vector, 0.9),
                (SearchChannel::Graph, 1.1),
                (SearchChannel::History, 0.65),
                (SearchChannel::Diagnostics, 1.2),
                (SearchChannel::Trace, 1.0),
                (SearchChannel::Summary, 0.45),
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: &str, channel: SearchChannel, rank: usize) -> SearchHit {
        SearchHit {
            chunk_id: ChunkId::new(id),
            channel,
            rank,
            score: None,
        }
    }

    #[test]
    fn multiple_channels_raise_a_candidate() {
        let lists = vec![
            vec![
                hit("source", SearchChannel::Lexical, 1),
                hit("combined", SearchChannel::Lexical, 2),
            ],
            vec![
                hit("combined", SearchChannel::Graph, 1),
                hit("graph", SearchChannel::Graph, 2),
            ],
        ];
        let fused = reciprocal_rank_fusion(&lists, &FusionConfig::default(), 3);
        assert_eq!(fused[0].chunk_id.as_str(), "combined");
    }
}
