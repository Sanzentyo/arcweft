use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    PlatformTextInputContext, PlatformTextInputEvent, TextInputAdapterKind,
    TextInputFocusGeneration, TextInputOperation, TextInputSerial, TextInputSessionId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfEditAccess {
    Read,
    ReadWrite,
    CommandCallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsTsfSerialAllocator {
    next: TextInputSerial,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfEventContext {
    session: TextInputSessionId,
    generation: TextInputFocusGeneration,
    target: InteractionTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfEditSessionBuilder {
    base: WindowsTsfEventContext,
    serial: TextInputSerial,
    access: WindowsTsfEditAccess,
    operations: Vec<TextInputOperation>,
}

impl Default for WindowsTsfSerialAllocator {
    fn default() -> Self {
        Self {
            next: TextInputSerial(1),
        }
    }
}

impl WindowsTsfSerialAllocator {
    pub const fn new(first: TextInputSerial) -> Self {
        Self { next: first }
    }

    pub const fn peek(&self) -> TextInputSerial {
        self.next
    }

    pub fn begin_session(
        &mut self,
        base: WindowsTsfEventContext,
        access: WindowsTsfEditAccess,
    ) -> WindowsTsfEditSessionBuilder {
        let serial = self.next;
        self.next = TextInputSerial(self.next.0.saturating_add(1));
        WindowsTsfEditSessionBuilder::new(base, serial, access)
    }
}

impl WindowsTsfEventContext {
    pub fn new(
        session: TextInputSessionId,
        generation: TextInputFocusGeneration,
        target: InteractionTarget,
    ) -> Self {
        Self {
            session,
            generation,
            target,
        }
    }

    pub const fn session(&self) -> TextInputSessionId {
        self.session
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    fn platform_context(&self, serial: TextInputSerial) -> PlatformTextInputContext {
        PlatformTextInputContext::new(
            TextInputAdapterKind::WindowsTsf,
            self.session,
            self.generation,
            self.target.clone(),
            serial,
        )
    }
}

impl WindowsTsfEditSessionBuilder {
    fn new(
        base: WindowsTsfEventContext,
        serial: TextInputSerial,
        access: WindowsTsfEditAccess,
    ) -> Self {
        Self {
            base,
            serial,
            access,
            operations: Vec::new(),
        }
    }

    pub const fn serial(&self) -> TextInputSerial {
        self.serial
    }

    pub const fn access(&self) -> WindowsTsfEditAccess {
        self.access
    }

    pub fn push_operation(&mut self, operation: TextInputOperation) {
        self.operations.push(operation);
    }

    #[must_use]
    pub fn with_operation(mut self, operation: TextInputOperation) -> Self {
        self.push_operation(operation);
        self
    }

    pub fn finish(self) -> Option<PlatformTextInputEvent> {
        if self.operations.is_empty() || self.access == WindowsTsfEditAccess::Read {
            return None;
        }
        Some(PlatformTextInputEvent::Batch {
            context: self.base.platform_context(self.serial),
            operations: self.operations,
        })
    }
}
