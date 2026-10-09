//! Borrowed source-order codec children shared by encoders and validation.
use super::{RuntimeCodecUse, RuntimeFieldCodecUse, RuntimeVariantCodecUse};

pub(super) enum CodecEvent<'a> {
    Node {
        node: &'a RuntimeCodecUse,
        depth: usize,
    },
    Item {
        ordinal: usize,
    },
    Field {
        ordinal: usize,
        field: &'a RuntimeFieldCodecUse,
    },
    Case {
        ordinal: usize,
        case: &'a RuntimeVariantCodecUse,
    },
}
pub(super) enum CodecChild<'a> {
    Direct(&'a RuntimeCodecUse),
    Item {
        ordinal: usize,
        node: &'a RuntimeCodecUse,
    },
    Field {
        ordinal: usize,
        field: &'a RuntimeFieldCodecUse,
    },
    Case {
        ordinal: usize,
        case: &'a RuntimeVariantCodecUse,
    },
}
impl<'a> CodecChild<'a> {
    pub(super) fn node(&self) -> Option<&'a RuntimeCodecUse> {
        match self {
            Self::Direct(node) | Self::Item { node, .. } => Some(node),
            Self::Field { field, .. } => Some(&field.value),
            Self::Case { case, .. } => case.payload.as_ref(),
        }
    }
}
pub(super) enum CodecChildren<'a> {
    Empty,
    Direct(std::array::IntoIter<Option<&'a RuntimeCodecUse>, 2>),
    Nodes(std::iter::Enumerate<std::slice::Iter<'a, RuntimeCodecUse>>),
    Fields(std::iter::Enumerate<std::slice::Iter<'a, RuntimeFieldCodecUse>>),
    Cases(std::iter::Enumerate<std::slice::Iter<'a, RuntimeVariantCodecUse>>),
}
impl<'a> CodecChildren<'a> {
    pub(super) fn new(node: &'a RuntimeCodecUse) -> Self {
        use RuntimeCodecUse as C;
        match node {
            C::Unary { item } | C::Newtype { inner: item } => {
                Self::Direct([Some(item.as_ref()), None].into_iter())
            }
            C::Map { key, value } => {
                Self::Direct([Some(key.as_ref()), Some(value.as_ref())].into_iter())
            }
            C::Tuple { items }
            | C::RecordFields { fields: items }
            | C::Builtin { payloads: items }
            | C::Choice {
                alternatives: items,
            }
            | C::Opaque { arguments: items } => Self::Nodes(items.iter().enumerate()),
            C::Record { fields, .. } => Self::Fields(fields.iter().enumerate()),
            C::Enum { cases, .. } => Self::Cases(cases.iter().enumerate()),
            C::Plain | C::Bytes { .. } | C::NominalRef => Self::Empty,
        }
    }
}
impl<'a> Iterator for CodecChildren<'a> {
    type Item = CodecChild<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Direct(nodes) => nodes.find_map(|node| node.map(CodecChild::Direct)),
            Self::Nodes(nodes) => nodes
                .next()
                .map(|(ordinal, node)| CodecChild::Item { ordinal, node }),
            Self::Fields(fields) => fields
                .next()
                .map(|(ordinal, field)| CodecChild::Field { ordinal, field }),
            Self::Cases(cases) => cases
                .next()
                .map(|(ordinal, case)| CodecChild::Case { ordinal, case }),
        }
    }
}
enum Work<'a> {
    Node(&'a RuntimeCodecUse, usize),
    Children(CodecChildren<'a>, usize),
}
pub(super) struct CodecEvents<'a> {
    work: Vec<Work<'a>>,
}
impl<'a> CodecEvents<'a> {
    pub(super) fn new(node: &'a RuntimeCodecUse) -> Self {
        Self {
            work: vec![Work::Node(node, 0)],
        }
    }
}
impl<'a> Iterator for CodecEvents<'a> {
    type Item = CodecEvent<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(work) = self.work.pop() {
            let event = match work {
                Work::Node(node, depth) => {
                    self.work
                        .push(Work::Children(CodecChildren::new(node), depth + 1));
                    CodecEvent::Node { node, depth }
                }
                Work::Children(mut children, depth) => {
                    let Some(child) = children.next() else {
                        continue;
                    };
                    self.work.push(Work::Children(children, depth));
                    if let Some(node) = child.node() {
                        self.work.push(Work::Node(node, depth));
                    }
                    match child {
                        CodecChild::Direct(_) => continue,
                        CodecChild::Item { ordinal, .. } => CodecEvent::Item { ordinal },
                        CodecChild::Field { ordinal, field } => {
                            CodecEvent::Field { ordinal, field }
                        }
                        CodecChild::Case { ordinal, case } => CodecEvent::Case { ordinal, case },
                    }
                }
            };
            return Some(event);
        }
        None
    }
}

#[cfg(test)]
mod tests;
