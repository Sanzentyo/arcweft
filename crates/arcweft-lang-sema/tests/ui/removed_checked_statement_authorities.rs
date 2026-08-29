use arcweft_lang_sema::checked_rich_text::PreparedCheckedDialogueMark;
use arcweft_lang_sema::final_analysis::analyzer::statement_scrutinee::StatementScrutineeTypeAuthority;
use arcweft_lang_sema::final_analysis::{
    CheckedDialogueMarkHandler, CheckedDialogueMarkOrdinal, CheckedStatementRole,
    PreparedEventScrutineeProof,
};

fn publish_event(proof: &PreparedEventScrutineeProof) -> PreparedEventScrutineeProof {
    proof.clone()
}

fn publish_mark(proof: &PreparedCheckedDialogueMark) -> PreparedCheckedDialogueMark {
    proof.clone()
}

fn publish_scrutinee(authority: &StatementScrutineeTypeAuthority<'static>) {
    let _ = authority;
}

fn old_authorities(
    _: Option<CheckedStatementRole>,
    _: Option<CheckedDialogueMarkOrdinal>,
    _: Option<CheckedDialogueMarkHandler>,
) {
}

fn main() {}
