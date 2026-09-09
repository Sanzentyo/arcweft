use super::{HirDatabase, key, parsed_revisions_with_source, publish_attached_project};
use crate::dialogue_application::{HirAttachedContentApplicationFamily, HirDialogueNodeKind};
use crate::expr::{HirCallArgument, HirCallValue, HirExprKind};
use crate::leaf::{HirLiteral, HirStringLiteral};
use crate::source_index::{
    HirDialogueNodeSourcePart, HirExprSourceRole, HirSourcePresence, HirSourceQuery,
    HirSourceQueryError, HirSourceSite,
};

fn check_ruby_source(fragment: &str, retained: bool) {
    let source = format!(
        "pub character alice {{ display = \"Alice\" }}\n\
         flow main {{\n    alice: Before {fragment}[p]\n}}\n"
    );
    let (parsed, revised) =
        parsed_revisions_with_source("arcweft-test://dialogue-source/ruby", &source);
    let mut database = HirDatabase::try_new().unwrap();
    let output = publish_attached_project(&mut database, &parsed, &key(&parsed));
    let module = output.module();
    assert_eq!(module.status(), crate::module::HirModuleStatus::Clean);
    let expressions = module.arenas().expressions();
    let mut ruby_calls = 0;
    for (_, expression) in expressions.try_iter(module.slots()).unwrap() {
        if let HirExprKind::AttachedContentApplication(application) = expression.kind()
            && let HirAttachedContentApplicationFamily::ContentCall { invocation, .. } =
                application.family()
        {
            ruby_calls += 1;
            let [base] = application.content().nodes() else {
                panic!("one canonical Ruby base")
            };
            assert!(
                matches!(base.kind(), HirDialogueNodeKind::Text(text) if text.as_str() == "夢")
            );
            let [
                HirCallArgument::Positional {
                    value: HirCallValue::Present { value: reading },
                },
            ] = invocation.arguments()
            else {
                panic!("one canonical Ruby reading")
            };
            assert!(matches!(
                expressions.resolve(module.slots(), *reading).unwrap().kind(),
                HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(text))) if text.as_ref() == "ゆめ"
            ));
        }
    }
    assert_eq!(ruby_calls, 1);
    let (owner, application) = expressions
        .try_iter(module.slots())
        .unwrap()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::AttachedContentApplication(application)
                if application.is_dialogue_line() =>
            {
                Some((owner, application))
            }
            _ => None,
        })
        .unwrap();
    let ordinal = application
        .content()
        .nodes()
        .iter()
        .position(|node| matches!(node.kind(), HirDialogueNodeKind::ContentApplication(_)))
        .unwrap();
    let query = |part| HirSourceQuery::Expr {
        owner,
        role: HirExprSourceRole::DialogueNode {
            ordinal: u32::try_from(ordinal).unwrap(),
            part,
        },
    };
    let authored_part = if retained {
        (HirDialogueNodeSourcePart::Ruby, fragment)
    } else {
        (HirDialogueNodeSourcePart::Expression, "ruby(\"ゆめ\")")
    };
    for (part, expected) in [(HirDialogueNodeSourcePart::Whole, fragment), authored_part] {
        let lookup = module
            .source_site(parsed.document().identity(), query(part))
            .unwrap();
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
            panic!("authored node has a source span")
        };
        let text = &parsed.document().text()[span.range().start()..span.range().end()];
        assert_eq!(text, expected);
    }
    let absent_part = if retained {
        HirDialogueNodeSourcePart::Hash
    } else {
        HirDialogueNodeSourcePart::Ruby
    };
    assert!(matches!(
        module.source_site(parsed.document().identity(), query(absent_part)),
        Err(HirSourceQueryError::ExprRoleNotApplicable { .. })
    ));
    assert!(matches!(
        module.source_site(
            revised.document().identity(),
            query(HirDialogueNodeSourcePart::Whole)
        ),
        Err(HirSourceQueryError::StaleSourceRevision { .. })
    ));
}

#[test]
fn retained_ruby_sources_publish_the_canonical_call_and_exact_authored_roles() {
    for fragment in ["|[夢](ゆめ)", "｜夢《ゆめ》"] {
        check_ruby_source(fragment, true);
    }
}

#[test]
fn typed_ruby_source_retains_its_distinct_authored_roles() {
    check_ruby_source("#ruby(\"ゆめ\")[夢]", false);
}
