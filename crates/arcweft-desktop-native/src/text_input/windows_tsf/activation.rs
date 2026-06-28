use crate::text_input::windows_tsf::capabilities::{
    WindowsTsfCapabilityEntry, WindowsTsfCapabilityReport, WindowsTsfRuntimeFacts,
};
use crate::text_input::windows_tsf::edit_session::{
    WindowsTsfEditAccess, WindowsTsfEditSessionBuilder, WindowsTsfEventContext,
    WindowsTsfSerialAllocator,
};
use arcweft_presentation::text_input::{
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputSerial,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfActivation {
    capabilities: WindowsTsfCapabilityReport,
    diagnostics: Vec<WindowsTsfActivationDiagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsTsfActivationDiagnostic {
    entry: WindowsTsfCapabilityEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfAdapter {
    facts: WindowsTsfRuntimeFacts,
    capabilities: WindowsTsfCapabilityReport,
    serials: WindowsTsfSerialAllocator,
}

impl WindowsTsfActivationDiagnostic {
    pub const fn new(entry: WindowsTsfCapabilityEntry) -> Self {
        Self { entry }
    }

    pub const fn entry(self) -> WindowsTsfCapabilityEntry {
        self.entry
    }

    pub const fn code(self) -> &'static str {
        self.entry.feature().code()
    }

    pub const fn reason_code(self) -> &'static str {
        self.entry.status().diagnostic_code()
    }
}

impl WindowsTsfActivation {
    pub fn from_facts(facts: WindowsTsfRuntimeFacts) -> Self {
        let capabilities = WindowsTsfCapabilityReport::from_facts(facts);
        let diagnostics = capabilities
            .diagnostics()
            .into_iter()
            .map(WindowsTsfActivationDiagnostic::new)
            .collect();
        Self {
            capabilities,
            diagnostics,
        }
    }

    pub const fn capabilities(&self) -> &WindowsTsfCapabilityReport {
        &self.capabilities
    }

    pub fn diagnostics(&self) -> &[WindowsTsfActivationDiagnostic] {
        &self.diagnostics
    }
}

impl WindowsTsfAdapter {
    pub fn activate(facts: WindowsTsfRuntimeFacts) -> (Self, WindowsTsfActivation) {
        let activation = WindowsTsfActivation::from_facts(facts);
        let adapter = Self {
            facts,
            capabilities: activation.capabilities().clone(),
            serials: WindowsTsfSerialAllocator::default(),
        };
        (adapter, activation)
    }

    #[must_use]
    pub fn with_first_serial(mut self, first: TextInputSerial) -> Self {
        self.serials = WindowsTsfSerialAllocator::new(first);
        self
    }

    pub const fn facts(&self) -> WindowsTsfRuntimeFacts {
        self.facts
    }

    pub const fn capabilities(&self) -> &WindowsTsfCapabilityReport {
        &self.capabilities
    }

    pub fn begin_edit_session(
        &mut self,
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        access: WindowsTsfEditAccess,
    ) -> WindowsTsfEditSessionBuilder {
        self.serials.begin_session(
            WindowsTsfEventContext::new(snapshot.session(), generation, snapshot.target().clone()),
            access,
        )
    }
}
