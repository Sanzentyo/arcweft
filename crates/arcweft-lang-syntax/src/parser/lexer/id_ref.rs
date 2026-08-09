//! Typed semantic projection for lexer-owned entity-reference tokens.

use arcweft_source::SourceRange;

use crate::grammar::kinds::SyntaxKind;
use crate::id_ref::{
    AuthoredIdRef, AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefComponent, SyntaxIdRefIssue,
    SyntaxIdRefPart, SyntaxIdRefShape, SyntaxIdRefSyntax,
};
use crate::name::{SyntaxName, is_identifier_continue};

use super::{LexToken, take_while, token_local_range};

/// One source component emitted by the entity-reference token projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::parser) struct EntityReferenceLexemeComponent {
    part: SyntaxIdRefPart,
    range: SourceRange,
    local_start: usize,
    local_end: usize,
}

impl EntityReferenceLexemeComponent {
    pub(in crate::parser) const fn part(self) -> SyntaxIdRefPart {
        self.part
    }

    pub(in crate::parser) const fn range(self) -> SourceRange {
        self.range
    }

    fn spelling(self, spelling: &str) -> &str {
        spelling.get(self.local_start..self.local_end).unwrap_or("")
    }
}

/// Typed qualification of an empty entity-reference marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::parser) enum EntityReferenceEmptyMarker {
    Unqualified,
    Family(SyntaxName),
}

impl EntityReferenceEmptyMarker {
    pub(in crate::parser) fn matches_family(&self, family: &str) -> bool {
        match self {
            Self::Unqualified => true,
            Self::Family(authored) => authored.as_str() == family,
        }
    }

    pub(in crate::parser) fn into_family(self) -> Option<SyntaxName> {
        match self {
            Self::Unqualified => None,
            Self::Family(family) => Some(family),
        }
    }
}

/// One lexer-owned entity-reference projection with exact component ranges.
pub(in crate::parser) struct EntityReferenceLexemeProjection {
    syntax: SyntaxIdRefSyntax,
    components: Vec<SyntaxIdRefComponent>,
    empty_marker_family: Option<EntityReferenceEmptyMarker>,
    unclosed_delimited_absolute: bool,
}

impl EntityReferenceLexemeProjection {
    pub(in crate::parser) const fn syntax(&self) -> &SyntaxIdRefSyntax {
        &self.syntax
    }

    pub(in crate::parser) fn components(&self) -> &[SyntaxIdRefComponent] {
        &self.components
    }

    pub(in crate::parser) const fn empty_marker_family(
        &self,
    ) -> Option<&EntityReferenceEmptyMarker> {
        self.empty_marker_family.as_ref()
    }

    pub(in crate::parser) const fn has_unclosed_delimited_absolute(&self) -> bool {
        self.unclosed_delimited_absolute
    }

    pub(in crate::parser) fn into_syntax(self) -> SyntaxIdRefSyntax {
        self.syntax
    }
}

/// Projects one already-recognized entity-reference token without another
/// parser or source reader.
pub(in crate::parser) fn typed_entity_reference(
    token: LexToken,
    spelling: &str,
) -> EntityReferenceLexemeProjection {
    let components = entity_reference_components(token, spelling);
    let absolute = components
        .iter()
        .any(|component| component.part() == SyntaxIdRefPart::AbsoluteMarker);
    let family_component = components
        .iter()
        .find(|component| component.part() == SyntaxIdRefPart::Family)
        .copied();
    let parent_count = components
        .iter()
        .filter(|component| matches!(component.part(), SyntaxIdRefPart::ParentMarker { .. }))
        .count();
    let segment_components = components
        .iter()
        .filter(|component| matches!(component.part(), SyntaxIdRefPart::SuffixSegment { .. }))
        .copied()
        .collect::<Vec<_>>();
    let segment_count = u32::try_from(segment_components.len())
        .expect("entity token length bounds semantic segment ordinals");
    let shape = SyntaxIdRefShape::new(
        absolute,
        family_component.is_some(),
        parent_count,
        segment_count,
    );
    let value = (|| {
        let root = if absolute {
            Ok(AuthoredIdRoot::Absolute {
                delimited: spelling.starts_with("@<"),
            })
        } else if let Some(family) = family_component {
            SyntaxName::try_new(family.spelling(spelling))
                .map(|family| AuthoredIdRoot::FamilyRelative {
                    family,
                    parent_depth: parent_count,
                })
                .map_err(SyntaxIdRefIssue::InvalidFamily)
        } else {
            Ok(AuthoredIdRoot::Relative {
                parent_depth: parent_count,
            })
        }?;
        if segment_components.is_empty()
            || segment_components
                .iter()
                .all(|component| component.spelling(spelling).is_empty())
        {
            return Err(SyntaxIdRefIssue::MissingSuffix);
        }
        let mut segments = Vec::new();
        for (ordinal, component) in segment_components.iter().enumerate() {
            let segment =
                AuthoredIdSegment::try_new(component.spelling(spelling)).map_err(|()| {
                    SyntaxIdRefIssue::InvalidSegment {
                        ordinal: u32::try_from(ordinal)
                            .expect("entity token length bounds segment ordinals"),
                    }
                })?;
            segments.push(segment);
        }
        Ok(AuthoredIdRef::new(root, segments))
    })();
    let empty_marker_family = if matches!(value, Err(SyntaxIdRefIssue::MissingSuffix))
        && !absolute
        && parent_count == 0
    {
        match family_component {
            Some(family) => SyntaxName::try_new(family.spelling(spelling))
                .ok()
                .map(EntityReferenceEmptyMarker::Family),
            None => Some(EntityReferenceEmptyMarker::Unqualified),
        }
    } else {
        None
    };
    EntityReferenceLexemeProjection {
        syntax: SyntaxIdRefSyntax::new(value, shape),
        components: components
            .into_iter()
            .map(|component| SyntaxIdRefComponent::new(component.part(), component.range()))
            .collect(),
        empty_marker_family,
        unclosed_delimited_absolute: spelling.starts_with("@<") && !spelling.ends_with('>'),
    }
}

fn entity_reference_components(
    token: LexToken,
    spelling: &str,
) -> Vec<EntityReferenceLexemeComponent> {
    if token.kind() != SyntaxKind::EntityReferenceToken
        || spelling.len() != token.range().end().saturating_sub(token.range().start())
        || !spelling.starts_with('@')
    {
        return Vec::new();
    }

    let mut output = Vec::new();
    push_component(
        &mut output,
        token,
        SyntaxIdRefPart::Whole,
        0,
        spelling.len(),
    );
    if spelling.starts_with("@<") {
        push_component(&mut output, token, SyntaxIdRefPart::AbsoluteMarker, 0, 2);
        let body_end = if spelling.ends_with('>') {
            spelling.len().saturating_sub(1)
        } else {
            spelling.len()
        };
        push_suffix_segments(&mut output, token, spelling, 2, body_end);
        return output;
    }

    let after_at = &spelling['@'.len_utf8()..];
    let family_len = take_while(after_at, is_identifier_continue);
    let family_end = '@'.len_utf8() + family_len;
    if family_len > 0
        && spelling
            .get(family_end..)
            .is_some_and(|tail| tail.starts_with(':'))
        && spelling.as_bytes().get(family_end + ':'.len_utf8()) == Some(&b'.')
    {
        push_component(
            &mut output,
            token,
            SyntaxIdRefPart::Family,
            '@'.len_utf8(),
            family_end,
        );
        push_component(
            &mut output,
            token,
            SyntaxIdRefPart::FamilySeparator,
            family_end,
            family_end + ':'.len_utf8(),
        );
        let dots_start = family_end + ':'.len_utf8();
        let dots_end = dots_start + take_while(&spelling[dots_start..], |ch| ch == '.');
        push_dot_parent_markers(&mut output, token, dots_start, dots_end);
        push_suffix_segments(&mut output, token, spelling, dots_end, spelling.len());
        return output;
    }

    if after_at.starts_with('.') {
        let dots_start = '@'.len_utf8();
        let dots_end = dots_start + take_while(after_at, |ch| ch == '.');
        push_dot_parent_markers(&mut output, token, dots_start, dots_end);
        push_suffix_segments(&mut output, token, spelling, dots_end, spelling.len());
        return output;
    }

    if after_at.starts_with("super.") {
        let mut cursor = '@'.len_utf8();
        let mut ordinal = 0_usize;
        while spelling
            .get(cursor..)
            .is_some_and(|tail| tail.starts_with("super."))
        {
            let marker_end = cursor + "super".len();
            push_component(
                &mut output,
                token,
                SyntaxIdRefPart::ParentMarker {
                    ordinal: checked_ordinal(ordinal),
                },
                cursor,
                marker_end,
            );
            ordinal = ordinal
                .checked_add(1)
                .expect("source token length bounds entity parent ordinals");
            cursor = marker_end + '.'.len_utf8();
        }
        push_suffix_segments(&mut output, token, spelling, cursor, spelling.len());
        return output;
    }

    push_component(
        &mut output,
        token,
        SyntaxIdRefPart::AbsoluteMarker,
        0,
        '@'.len_utf8(),
    );
    push_suffix_segments(&mut output, token, spelling, '@'.len_utf8(), spelling.len());
    output
}

fn push_component(
    output: &mut Vec<EntityReferenceLexemeComponent>,
    token: LexToken,
    part: SyntaxIdRefPart,
    start: usize,
    end: usize,
) {
    output.push(EntityReferenceLexemeComponent {
        part,
        range: token_local_range(token, start, end),
        local_start: start,
        local_end: end,
    });
}

fn push_dot_parent_markers(
    output: &mut Vec<EntityReferenceLexemeComponent>,
    token: LexToken,
    dots_start: usize,
    dots_end: usize,
) {
    for (ordinal, marker) in (dots_start + '.'.len_utf8()..dots_end).enumerate() {
        push_component(
            output,
            token,
            SyntaxIdRefPart::ParentMarker {
                ordinal: checked_ordinal(ordinal),
            },
            marker,
            marker + '.'.len_utf8(),
        );
    }
}

fn push_suffix_segments(
    output: &mut Vec<EntityReferenceLexemeComponent>,
    token: LexToken,
    spelling: &str,
    start: usize,
    end: usize,
) {
    let mut segment_start = start;
    let mut ordinal = 0_usize;
    for (relative, character) in spelling[start..end].char_indices() {
        if character != '.' {
            continue;
        }
        let separator = start + relative;
        push_component(
            output,
            token,
            SyntaxIdRefPart::SuffixSegment {
                ordinal: checked_ordinal(ordinal),
            },
            segment_start,
            separator,
        );
        ordinal = ordinal
            .checked_add(1)
            .expect("source token length bounds entity suffix ordinals");
        segment_start = separator + '.'.len_utf8();
    }
    push_component(
        output,
        token,
        SyntaxIdRefPart::SuffixSegment {
            ordinal: checked_ordinal(ordinal),
        },
        segment_start,
        end,
    );
}

fn checked_ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("source document byte limits fit lexical component ordinals")
}
