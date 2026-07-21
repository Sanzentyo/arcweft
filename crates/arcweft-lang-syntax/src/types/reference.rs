//! Reference-type prefix grammar and exact recovery ranges.

use super::{TypeParseError, parse_lifetime_name, parse_type_ref_value};
use crate::ast::common::TextRange;
use crate::cst::{split_leading_ident, split_leading_lifetime};
use crate::reference::{BorrowKind, ReferenceType, RegionSyntax};

pub(super) fn parse_reference_type(source: &str) -> Result<ReferenceType, TypeParseError> {
    debug_assert!(source.starts_with('&'));
    let amp_end = '&'.len_utf8();
    let mut cursor = skip_trivia(source, amp_end)?;
    let region = if let Some((lifetime, _)) = split_leading_lifetime(&source[cursor..]) {
        let range = TextRange::new(cursor, cursor + lifetime.len());
        cursor = skip_trivia(source, range.end())?;
        RegionSyntax::Named {
            name: parse_lifetime_name(lifetime, range),
            range,
        }
    } else {
        RegionSyntax::Elided {
            anchor: TextRange::new(amp_end, amp_end),
        }
    };

    let (kind, mut_range) = match split_leading_ident(&source[cursor..]) {
        Some(("mut", _)) => {
            let range = TextRange::new(cursor, cursor + "mut".len());
            cursor = skip_trivia(source, range.end())?;
            (BorrowKind::Mutable, Some(range))
        }
        _ => (BorrowKind::Shared, None),
    };
    if kind.is_mutable()
        && let Some((lifetime, _)) = split_leading_lifetime(&source[cursor..])
    {
        return Err(TypeParseError::at(
            "syntax.type.region_after_mut",
            "a reference lifetime must appear before `mut`",
            TextRange::new(cursor, cursor + lifetime.len()),
        ));
    }
    if cursor == source.len() {
        return Err(TypeParseError::at(
            "syntax.type.reference_missing_referent",
            "reference type requires a referent",
            TextRange::new(cursor, cursor),
        ));
    }

    let mut referent =
        parse_type_ref_value(&source[cursor..]).map_err(|error| error.rebased(cursor))?;
    referent.rebase_reference_ranges(cursor);
    Ok(ReferenceType::new(
        kind,
        region,
        Box::new(referent),
        TextRange::new(0, amp_end),
        mut_range,
        TextRange::new(0, source.len()),
    ))
}

pub(super) fn reference_referent_start(source: &str) -> Result<usize, TypeParseError> {
    debug_assert!(source.starts_with('&'));
    let amp_end = '&'.len_utf8();
    let mut cursor = skip_trivia(source, amp_end)?;
    if let Some((lifetime, _)) = split_leading_lifetime(&source[cursor..]) {
        cursor = skip_trivia(source, cursor + lifetime.len())?;
    }
    if matches!(split_leading_ident(&source[cursor..]), Some(("mut", _))) {
        cursor = skip_trivia(source, cursor + "mut".len())?;
    }
    Ok(cursor)
}

fn skip_trivia(source: &str, mut cursor: usize) -> Result<usize, TypeParseError> {
    loop {
        while let Some(ch) = source[cursor..].chars().next()
            && ch.is_whitespace()
        {
            cursor += ch.len_utf8();
        }
        if source[cursor..].starts_with("/*") {
            let comment_start = cursor;
            let Some(close) = source[cursor + 2..].find("*/") else {
                return Err(TypeParseError::at(
                    "syntax.type.invalid",
                    "unclosed comment in reference type",
                    TextRange::new(comment_start, source.len()),
                ));
            };
            cursor += 2 + close + 2;
            continue;
        }
        if source[cursor..].starts_with("//") {
            cursor = source[cursor..]
                .find('\n')
                .map_or(source.len(), |newline| cursor + newline + 1);
            continue;
        }
        return Ok(cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{TypeRef, parse_type_ref};
    use crate::ast::common::TextRange;
    use crate::reference::{BorrowKind, RegionSyntax};

    #[test]
    fn reference_forms_preserve_kind_region_and_operator_ranges() {
        let fixtures = [
            ("&T", BorrowKind::Shared, None, None),
            (
                "&mut T",
                BorrowKind::Mutable,
                None,
                Some(TextRange::new(1, 4)),
            ),
            ("&'a T", BorrowKind::Shared, Some("a"), None),
            (
                "&'a mut T",
                BorrowKind::Mutable,
                Some("a"),
                Some(TextRange::new(4, 7)),
            ),
        ];
        for (source, kind, lifetime, mut_range) in fixtures {
            let TypeRef::Reference(reference) = parse_type_ref(source)
                .expect("reference parses")
                .into_value()
            else {
                panic!("expected reference type");
            };
            assert_eq!(reference.kind(), kind);
            assert_eq!(reference.amp_range(), TextRange::new(0, 1));
            assert_eq!(reference.mut_range(), mut_range);
            assert_eq!(
                reference
                    .region()
                    .name()
                    .map(super::super::LifetimeName::name),
                lifetime
            );
            assert_eq!(reference.range(), TextRange::new(0, source.len()));
        }
    }

    #[test]
    fn trivia_does_not_change_reference_mutability() {
        for source in ["& mut T", "&/* ownership */mut T", "&\nmut T"] {
            let TypeRef::Reference(reference) = parse_type_ref(source)
                .expect("reference parses")
                .into_value()
            else {
                panic!("expected reference type");
            };
            assert_eq!(reference.kind(), BorrowKind::Mutable);
        }
        let TypeRef::Reference(reference) = parse_type_ref("&mutable")
            .expect("reference parses")
            .into_value()
        else {
            panic!("expected reference type");
        };
        assert_eq!(reference.kind(), BorrowKind::Shared);
    }

    #[test]
    fn nested_reference_ranges_use_original_type_offsets() {
        let TypeRef::Reference(outer) = parse_type_ref("  &&mut T  ")
            .expect("nested reference")
            .into_value()
        else {
            panic!("expected outer reference");
        };
        assert_eq!(outer.amp_range(), TextRange::new(2, 3));
        assert_eq!(outer.range(), TextRange::new(2, 9));

        let TypeRef::Reference(inner) = outer.referent() else {
            panic!("expected inner reference");
        };
        assert_eq!(inner.amp_range(), TextRange::new(3, 4));
        assert_eq!(inner.mut_range(), Some(TextRange::new(4, 7)));
        assert_eq!(inner.range(), TextRange::new(3, 9));
    }

    #[test]
    fn references_inside_composite_types_keep_parent_offsets() {
        let TypeRef::Generic { args, .. } =
            parse_type_ref("Vec<&mut T>").expect("generic").into_value()
        else {
            panic!("expected generic");
        };
        let TypeRef::Reference(generic_reference) = &args[0] else {
            panic!("expected generic reference argument");
        };
        assert_eq!(generic_reference.amp_range(), TextRange::new(4, 5));
        assert_eq!(generic_reference.range(), TextRange::new(4, 10));

        let TypeRef::Tuple(items) = parse_type_ref("(&A, &mut B)").expect("tuple").into_value()
        else {
            panic!("expected tuple");
        };
        let TypeRef::Reference(first) = &items[0] else {
            panic!("expected first tuple reference");
        };
        let TypeRef::Reference(second) = &items[1] else {
            panic!("expected second tuple reference");
        };
        assert_eq!(first.range(), TextRange::new(1, 3));
        assert_eq!(second.range(), TextRange::new(5, 11));

        let TypeRef::Function {
            params,
            return_type,
            ..
        } = parse_type_ref("&A -> &mut B")
            .expect("function type")
            .into_value()
        else {
            panic!("expected function type");
        };
        let TypeRef::Reference(param) = &params[0] else {
            panic!("expected reference parameter");
        };
        let TypeRef::Reference(result) = return_type.as_ref() else {
            panic!("expected reference return");
        };
        assert_eq!(param.range(), TextRange::new(0, 2));
        assert_eq!(result.range(), TextRange::new(6, 12));
    }

    #[test]
    fn invalid_region_order_and_missing_referent_are_typed() {
        let order = parse_type_ref("&mut 'a T").expect_err("invalid order must fail");
        assert_eq!(order.code(), "syntax.type.region_after_mut");
        assert_eq!(order.range(), Some(TextRange::new(5, 7)));

        for source in ["&", "&mut", "&'a"] {
            let error = parse_type_ref(source).expect_err("missing referent must fail");
            assert_eq!(error.code(), "syntax.type.reference_missing_referent");
            assert_eq!(
                error.range(),
                Some(TextRange::new(source.len(), source.len()))
            );
        }
    }

    #[test]
    fn reference_prefix_binds_tighter_than_type_choice() {
        let TypeRef::Choice(items) = parse_type_ref("&T | U")
            .expect("choice parses")
            .into_value()
        else {
            panic!("expected outer type choice");
        };
        assert!(
            matches!(items.as_slice(), [TypeRef::Reference(_), TypeRef::Path(path)] if path.canonical_string() == "U")
        );

        let TypeRef::Reference(reference) = parse_type_ref("&(T | U)")
            .expect("reference parses")
            .into_value()
        else {
            panic!("expected outer reference");
        };
        assert!(matches!(reference.referent(), TypeRef::Choice(_)));
        assert!(
            matches!(reference.region(), RegionSyntax::Elided { anchor } if *anchor == TextRange::new(1, 1))
        );
    }
}
