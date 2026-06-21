use arcweft_lang_syntax::{
    ast::{
        dialogue::{DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultsItem},
        items::Item,
    },
    source::ParsedSource,
};

use crate::model::TextEdit;

pub(crate) fn dialogue_defaults_nested_assignment_edits(
    source: &str,
    parsed: &ParsedSource,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for defaults in parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::DialogueDefaults(defaults) => Some(defaults),
            _ => None,
        })
    {
        edits.extend(dialogue_defaults_nested_assignment_runs(source, defaults));
    }
    edits
}

fn dialogue_defaults_nested_assignment_runs(
    source: &str,
    defaults: &DialogueDefaultsItem,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut pending = NestedAssignmentRun::default();
    for assignment in defaults.assignments() {
        if let Some(dotted) = DottedAssignment::new(source, assignment) {
            if !pending.can_append(source, &dotted)
                && let Some(edit) = pending.finish()
            {
                edits.push(edit);
            }
            pending.push(&dotted);
        } else if let Some(edit) = pending.finish() {
            edits.push(edit);
        }
    }
    if let Some(edit) = pending.finish() {
        edits.push(edit);
    }
    edits
}

#[derive(Debug)]
struct DottedAssignment {
    line_start: usize,
    end: usize,
    indent: String,
    parts: Vec<String>,
    op: &'static str,
    value: String,
}

impl DottedAssignment {
    fn new(source: &str, assignment: &DialogueDefaultAssignment) -> Option<Self> {
        let path_source = source.get(assignment.path_range().as_range())?.trim();
        let parts = path_source
            .split('.')
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().to_owned())
            .collect::<Vec<_>>();
        if parts.len() <= 1 {
            return None;
        }
        let line_start = source[..assignment.range().start()]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        Some(Self {
            line_start,
            end: assignment.range().end(),
            indent: source
                .get(line_start..assignment.range().start())?
                .to_owned(),
            parts,
            op: assignment_op_label(assignment.op()),
            value: assignment.raw_value().to_owned(),
        })
    }
}

#[derive(Debug, Default)]
struct NestedAssignmentRun {
    start: Option<usize>,
    end: usize,
    indent: String,
    tree: Vec<NestedAssignmentNode>,
}

impl NestedAssignmentRun {
    fn can_append(&self, source: &str, assignment: &DottedAssignment) -> bool {
        let Some(start) = self.start else {
            return true;
        };
        start <= assignment.line_start
            && self.end <= assignment.line_start
            && source
                .get(self.end..assignment.line_start)
                .is_some_and(|between| between.trim().is_empty())
    }

    fn push(&mut self, assignment: &DottedAssignment) {
        if self.start.is_none() {
            self.start = Some(assignment.line_start);
            self.indent.clone_from(&assignment.indent);
        }
        self.end = assignment.end;
        insert_nested_assignment(
            &mut self.tree,
            &assignment.parts,
            assignment.op,
            &assignment.value,
        );
    }

    fn finish(&mut self) -> Option<TextEdit> {
        let start = self.start?;
        let replacement = nested_assignment_tree_text(&self.indent, &self.tree);
        let edit = TextEdit {
            start,
            end: self.end,
            replacement,
        };
        *self = Self::default();
        Some(edit)
    }
}

#[derive(Debug)]
enum NestedAssignmentNode {
    Block {
        name: String,
        children: Vec<NestedAssignmentNode>,
    },
    Leaf {
        name: String,
        op: &'static str,
        value: String,
    },
}

fn insert_nested_assignment(
    nodes: &mut Vec<NestedAssignmentNode>,
    parts: &[String],
    op: &'static str,
    value: &str,
) {
    if let Some((head, tail)) = parts.split_first() {
        if tail.is_empty() {
            nodes.push(NestedAssignmentNode::Leaf {
                name: head.clone(),
                op,
                value: value.to_owned(),
            });
            return;
        }
        if let Some(NestedAssignmentNode::Block { children, .. }) =
            nodes.iter_mut().find(|node| match node {
                NestedAssignmentNode::Block { name, .. } => name == head,
                NestedAssignmentNode::Leaf { .. } => false,
            })
        {
            insert_nested_assignment(children, tail, op, value);
            return;
        }
        let mut children = Vec::new();
        insert_nested_assignment(&mut children, tail, op, value);
        nodes.push(NestedAssignmentNode::Block {
            name: head.clone(),
            children,
        });
    }
}

fn nested_assignment_tree_text(indent: &str, nodes: &[NestedAssignmentNode]) -> String {
    let mut output = String::new();
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        write_nested_assignment_node(&mut output, indent, 0, node);
    }
    output
}

fn write_nested_assignment_node(
    output: &mut String,
    indent: &str,
    depth: usize,
    node: &NestedAssignmentNode,
) {
    output.push_str(indent);
    output.push_str(&"    ".repeat(depth));
    match node {
        NestedAssignmentNode::Block { name, children } => {
            output.push_str(name);
            output.push_str(" {");
            for child in children {
                output.push('\n');
                write_nested_assignment_node(output, indent, depth + 1, child);
            }
            output.push('\n');
            output.push_str(indent);
            output.push_str(&"    ".repeat(depth));
            output.push('}');
        }
        NestedAssignmentNode::Leaf { name, op, value } => {
            output.push_str(name);
            output.push(' ');
            output.push_str(op);
            output.push(' ');
            output.push_str(value);
        }
    }
}

fn assignment_op_label(op: DialogueDefaultAssignOp) -> &'static str {
    match op {
        DialogueDefaultAssignOp::Replace => "=",
        DialogueDefaultAssignOp::Append => "+=",
    }
}
