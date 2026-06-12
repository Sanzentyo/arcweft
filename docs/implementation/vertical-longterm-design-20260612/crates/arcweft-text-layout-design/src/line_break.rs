/// Item measured along the logical inline axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutItem {
    pub cluster_index: u32,
    pub advance: f32,
    pub penalty_after: i32,
    pub mandatory_break_after: bool,
    pub prohibited_break_after: bool,
}

impl LayoutItem {
    pub const fn glyph(cluster_index: u32, advance: f32) -> Self {
        Self {
            cluster_index,
            advance,
            penalty_after: 0,
            mandatory_break_after: false,
            prohibited_break_after: false,
        }
    }
}

/// A line break result over item indices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBreak {
    pub start: usize,
    pub end: usize,
    pub used_inline: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    cost: f32,
    previous: usize,
    used_inline: f32,
}

/// Dynamic-programming line breaker for the long-term design.
///
/// This simplified implementation models badness, overflow, and explicit break
/// penalties. Production should add UAX #14/JLREQ classes, stretch/shrink, ruby
/// collision feedback, and hanging punctuation.
pub fn break_lines_dp(items: &[LayoutItem], max_inline: f32) -> Vec<LineBreak> {
    if items.is_empty() {
        return Vec::new();
    }

    let n = items.len();
    let mut best: Vec<Option<Candidate>> = vec![None; n + 1];
    best[0] = Some(Candidate {
        cost: 0.0,
        previous: 0,
        used_inline: 0.0,
    });

    for start in 0..n {
        let Some(prefix) = best[start] else {
            continue;
        };
        let mut used = 0.0;
        for end in (start + 1)..=n {
            let item = items[end - 1];
            used += item.advance;
            let is_last = end == n;
            let can_break = is_last || item.mandatory_break_after || !item.prohibited_break_after;
            if !can_break {
                continue;
            }

            let cost = prefix.cost + line_cost(used, max_inline, item.penalty_after, is_last);
            let replace = best[end].map_or(true, |current| cost < current.cost);
            if replace {
                best[end] = Some(Candidate {
                    cost,
                    previous: start,
                    used_inline: used,
                });
            }

            if item.mandatory_break_after || used > max_inline * 1.5 {
                break;
            }
        }
    }

    reconstruct(items, &best)
}

fn line_cost(used: f32, max_inline: f32, penalty: i32, is_last: bool) -> f32 {
    let overflow = (used - max_inline).max(0.0);
    let remaining = (max_inline - used).max(0.0);
    let ragged = if is_last { 0.0 } else { remaining * remaining * 0.01 };
    overflow * overflow * 100.0 + ragged + penalty.max(0) as f32
}

fn reconstruct(items: &[LayoutItem], best: &[Option<Candidate>]) -> Vec<LineBreak> {
    let mut cursor = items.len();
    let mut lines = Vec::new();

    while cursor > 0 {
        let Some(candidate) = best[cursor] else {
            let start = cursor - 1;
            lines.push(LineBreak {
                start,
                end: cursor,
                used_inline: items[start].advance,
            });
            cursor = start;
            continue;
        };
        lines.push(LineBreak {
            start: candidate.previous,
            end: cursor,
            used_inline: candidate.used_inline,
        });
        cursor = candidate.previous;
    }

    lines.reverse();
    lines
}

#[cfg(test)]
mod tests {
    use super::{LayoutItem, break_lines_dp};

    #[test]
    fn breaks_when_measure_is_exceeded() {
        let items = (0_u32..5).map(|i| LayoutItem::glyph(i, 10.0)).collect::<Vec<_>>();
        let lines = break_lines_dp(&items, 25.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].start, 0);
        assert_eq!(lines[0].end, 2);
    }
}
