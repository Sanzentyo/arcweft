//! Bounded lexical traversal of the complete retained instruction regions.

use super::{SectionCodecError, ViewProgramInstruction};
use crate::resource_codec::view::model::ViewInstructionSpan;
use arcweft_view::{ViewBranchRanges, ViewInstructionRange};

pub(super) enum ScopeEvent<'a> {
    Enter,
    Exit,
    Instruction {
        index: u32,
        instruction: &'a ViewProgramInstruction,
    },
}

enum Work {
    Cursor(ViewInstructionRange),
    Region(ViewInstructionRange),
    Exit,
    Finish,
}

pub(super) struct ScopedInstructions<'a> {
    instructions: &'a [ViewProgramInstruction],
    work: Vec<Work>,
    floors: Vec<usize>,
    explicit_depth: usize,
    failed: bool,
}

impl<'a> ScopedInstructions<'a> {
    pub(super) fn new(
        instructions: &'a [ViewProgramInstruction],
        span: ViewInstructionSpan,
    ) -> Result<Self, SectionCodecError> {
        if span.start_instruction > span.end_instruction
            || span.end_instruction as usize > instructions.len()
        {
            return Err(invalid_span());
        }
        Ok(Self {
            instructions,
            work: vec![
                Work::Finish,
                Work::Cursor(ViewInstructionRange::new(
                    span.start_instruction,
                    span.end_instruction,
                )),
            ],
            floors: vec![0],
            explicit_depth: 0,
            failed: false,
        })
    }

    fn advance(&mut self) -> Result<Option<ScopeEvent<'a>>, SectionCodecError> {
        while let Some(work) = self.work.pop() {
            match work {
                Work::Region(range) => {
                    self.floors.push(self.explicit_depth);
                    self.work.extend([Work::Exit, Work::Cursor(range)]);
                    return Ok(Some(ScopeEvent::Enter));
                }
                Work::Exit => {
                    if self.floors.pop() != Some(self.explicit_depth) {
                        return Err(invalid_scope());
                    }
                    return Ok(Some(ScopeEvent::Exit));
                }
                Work::Finish => {
                    if self.explicit_depth != 0 || self.floors.len() != 1 {
                        return Err(invalid_scope());
                    }
                }
                Work::Cursor(range) => {
                    if range.start == range.end {
                        continue;
                    }
                    let index = range.start;
                    let instruction = self
                        .instructions
                        .get(index as usize)
                        .ok_or_else(invalid_span)?;
                    let start = index.checked_add(1).ok_or_else(invalid_span)?;
                    let mut children = Vec::new();
                    let continuation = match instruction {
                        ViewProgramInstruction::Branch {
                            then_span,
                            else_span,
                            ..
                        } => {
                            let ranges = ViewBranchRanges::try_from_spans(
                                index, *then_span, *else_span, range.end,
                            )
                            .ok_or_else(invalid_span)?;
                            children.push(ranges.then_range());
                            children.extend(ranges.else_range());
                            ranges.continuation()
                        }
                        ViewProgramInstruction::RepeatKeyed { body_span, .. } => {
                            let body = subrange(start, 0, *body_span, range.end)?;
                            children.push(body);
                            body.end
                        }
                        ViewProgramInstruction::Await {
                            pending_branch,
                            ready_branch,
                            error_branch,
                            denied_branch,
                            ..
                        } => {
                            for branch in
                                [pending_branch, ready_branch, error_branch, denied_branch]
                                    .into_iter()
                                    .flatten()
                            {
                                children.push(subrange(
                                    start,
                                    branch.start_offset,
                                    branch.body_span,
                                    range.end,
                                )?);
                            }
                            children.sort_by_key(|range| (range.start, range.end));
                            if children.windows(2).any(|pair| pair[0].end > pair[1].start) {
                                return Err(invalid_span());
                            }
                            children.last().map_or(start, |range| range.end)
                        }
                        _ => start,
                    };
                    match instruction {
                        ViewProgramInstruction::BeginScope => self.explicit_depth += 1,
                        ViewProgramInstruction::EndScope => {
                            if self.explicit_depth
                                <= *self.floors.last().ok_or_else(invalid_scope)?
                            {
                                return Err(invalid_scope());
                            }
                            self.explicit_depth -= 1;
                        }
                        _ => {}
                    }
                    self.work.push(Work::Cursor(ViewInstructionRange::new(
                        continuation,
                        range.end,
                    )));
                    self.work
                        .extend(children.into_iter().rev().map(Work::Region));
                    return Ok(Some(ScopeEvent::Instruction { index, instruction }));
                }
            }
        }
        Ok(None)
    }
}

impl<'a> Iterator for ScopedInstructions<'a> {
    type Item = Result<ScopeEvent<'a>, SectionCodecError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        match self.advance() {
            Ok(event) => event.map(Ok),
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }
}

fn subrange(
    start: u32,
    offset: u32,
    span: u32,
    enclosing_end: u32,
) -> Result<ViewInstructionRange, SectionCodecError> {
    let start = start.checked_add(offset).ok_or_else(invalid_span)?;
    let end = start
        .checked_add(span)
        .filter(|end| *end <= enclosing_end)
        .ok_or_else(invalid_span)?;
    Ok(ViewInstructionRange::new(start, end))
}

fn invalid_span() -> SectionCodecError {
    SectionCodecError::NonCanonicalTable("view_control_flow_spans")
}
fn invalid_scope() -> SectionCodecError {
    SectionCodecError::NonCanonicalTable("view_local_scope")
}
