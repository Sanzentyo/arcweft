use arcweft_lang_sema::checked_rich_text::CheckedDialogueMark;
use arcweft_lang_sema::final_analysis::{
    CheckedIncludeFlowTarget, CheckedSelectStatement, CheckedStatement, CheckedTrigger,
    CheckedUnsafeAudit,
};
use arcweft_lang_sema::semantic_coordinate::StableCheckedDialogueMarkCoordinate;

fn any<T>() -> T {
    panic!("fixture must not run")
}

fn forge_mark_coordinate() -> StableCheckedDialogueMarkCoordinate {
    StableCheckedDialogueMarkCoordinate {
        application: any(),
        ordinal: any(),
    }
}

fn forge_dialogue_mark() -> CheckedDialogueMark {
    CheckedDialogueMark {
        coordinate: any(),
        diagnostic_name: any(),
    }
}

fn forge_trigger() -> CheckedTrigger {
    CheckedTrigger { kind: any() }
}

fn forge_select() -> CheckedSelectStatement {
    CheckedSelectStatement { kind: any() }
}

fn forge_unsafe_audit() -> CheckedUnsafeAudit {
    CheckedUnsafeAudit {
        id: any(),
        has_safety_doc: false,
    }
}

fn forge_include() -> CheckedIncludeFlowTarget {
    CheckedIncludeFlowTarget { declaration: any() }
}

fn forge_statement() -> CheckedStatement {
    CheckedStatement {
        effects: any(),
        payload: any(),
    }
}

fn main() {}
