mod action_grammar;
#[cfg(test)]
mod action_grammar_tests;
mod activity_grammar;
#[cfg(test)]
mod activity_grammar_tests;
mod character_grammar;
#[cfg(test)]
mod character_grammar_tests;
mod cursor;
mod declaration;
#[cfg(test)]
mod dialogue_expression_tests;
mod document;
pub(crate) use document::parse_document;
#[cfg(test)]
pub(crate) use document::parse_document_with_global_count;
mod entry_grammar;
#[cfg(test)]
mod entry_grammar_tests;
mod expression;
mod extern_capability_grammar;
#[cfg(test)]
mod extern_capability_grammar_tests;
pub mod fragment;
mod function_grammar;
#[cfg(test)]
mod function_grammar_tests;
mod item;
#[cfg(test)]
mod item_tests;
mod layer_grammar;
#[cfg(test)]
mod layer_grammar_tests;
mod lexer;
mod metric_grammar;
#[cfg(test)]
mod metric_grammar_tests;
mod module_use_grammar;
#[cfg(test)]
mod module_use_grammar_tests;
mod path;
mod pattern;
mod pattern_projection;
mod predicate_proof;
#[cfg(test)]
mod predicate_proof_tests;
pub mod recovery;
mod resource_grammar;
#[cfg(test)]
mod resource_grammar_tests;
#[cfg(test)]
mod retained_grammar_tests;
#[cfg(test)]
mod retained_header_tests;
mod rich_text_grammar;
mod shadow_flow;
#[cfg(test)]
mod shadow_flow_tests;
mod shadow_recovery;
mod signal_grammar;
#[cfg(test)]
mod signal_grammar_tests;
mod statement;
mod style_grammar;
#[cfg(test)]
mod style_grammar_tests;
mod test_bench_grammar;
#[cfg(test)]
mod test_bench_grammar_tests;
mod trait_impl_grammar;
#[cfg(test)]
mod trait_impl_grammar_tests;
mod type_declaration_grammar;
#[cfg(test)]
mod type_declaration_grammar_tests;
mod type_ref;
pub(crate) mod unbound_fragment;
mod view_grammar;
#[cfg(test)]
mod view_grammar_tests;

pub use fragment::{ExpectedToken, ParseCompletion, ParseOptions};
pub use unbound_fragment::{
    AttachedFragment, ExpressionFragment, FragmentDiagnostic, FragmentKind, PatternFragment,
    StatementFragment, TypeFragment, UnboundFragment, parse_expression_fragment,
    parse_pattern_fragment, parse_statement_fragment, parse_type_fragment,
};
