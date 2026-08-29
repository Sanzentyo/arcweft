use arcweft_lang_sema::checked_rich_text::CheckedDialogueMark;
use arcweft_lang_sema::final_analysis::{
    CheckedIncludeFlowTarget, CheckedSelectBranchHead, CheckedSelectStatement,
    CheckedSelectStatementView, CheckedStatement, CheckedStatementPayload, CheckedTrigger,
    CheckedTriggerView, CheckedUnsafeAudit,
};

fn consume_trigger(trigger: &CheckedTrigger) {
    match trigger.view() {
        CheckedTriggerView::Input
        | CheckedTriggerView::Event
        | CheckedTriggerView::Signal
        | CheckedTriggerView::Timeout
        | CheckedTriggerView::Select
        | CheckedTriggerView::Task
        | CheckedTriggerView::Scope
        | CheckedTriggerView::Expression => {}
        CheckedTriggerView::Mark(coordinate) => {
            let _ = (coordinate.application(), coordinate.ordinal().get());
        }
    }
}

fn consume_select(select: &CheckedSelectStatement) {
    match select.view() {
        CheckedSelectStatementView::Operand => {}
        CheckedSelectStatementView::Branches(branches) => {
            for branch in branches {
                match branch {
                    CheckedSelectBranchHead::Bind
                    | CheckedSelectBranchHead::Frame
                    | CheckedSelectBranchHead::Event => {}
                }
            }
        }
    }
}

fn consume_unsafe_audit(audit: &CheckedUnsafeAudit) {
    let _ = (audit.id(), audit.has_safety_doc(), audit.semantic_id());
}

fn consume_include(target: &CheckedIncludeFlowTarget) {
    let _ = target.declaration();
}

fn consume_mark(mark: &CheckedDialogueMark) {
    let _ = (mark.coordinate(), mark.diagnostic_name());
}

fn consume_statement(statement: &CheckedStatement) {
    let _ = (statement.effects(), statement.payload());
    if let CheckedStatementPayload::Trigger(trigger) = statement.payload() {
        consume_trigger(trigger);
    }
}

#[test]
fn checked_consumers_need_only_borrowed_read_only_accessors() {
    let _ = consume_trigger as fn(&CheckedTrigger);
    let _ = consume_select as fn(&CheckedSelectStatement);
    let _ = consume_unsafe_audit as fn(&CheckedUnsafeAudit);
    let _ = consume_include as fn(&CheckedIncludeFlowTarget);
    let _ = consume_mark as fn(&CheckedDialogueMark);
    let _ = consume_statement as fn(&CheckedStatement);
}
