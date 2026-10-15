//! Entry metadata borrowed from the selected original executable.

use super::RuntimeProgramTypes;
use crate::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use crate::plan::{EntryRuntimeId, RuntimeEntryKind, RuntimeEntrySpec};

pub(crate) enum RuntimeProgramEntry<'a> {
    Plan(&'a RuntimeEntrySpec),
    Awbc {
        entry: &'a crate::awbc::schema::AwbcEntry,
        strings: &'a [String],
    },
}

impl<'a> RuntimeProgramTypes<'a> {
    pub(crate) fn entry(self, id: &EntryRuntimeId) -> Option<RuntimeProgramEntry<'a>> {
        match self {
            Self::Plan(plan) => plan
                .entries()
                .iter()
                .find(|entry| entry.id == *id)
                .map(RuntimeProgramEntry::Plan),
            Self::Awbc(program) => program
                .entries
                .iter()
                .find(|entry| entry.runtime_id == *id)
                .map(|entry| RuntimeProgramEntry::Awbc {
                    entry,
                    strings: &program.strings,
                }),
        }
    }
}

impl<'a> RuntimeProgramEntry<'a> {
    pub(crate) fn binding(&self) -> EntryBindingIdentity {
        match self {
            Self::Plan(entry) => entry.binding,
            Self::Awbc { entry, .. } => entry.binding,
        }
    }

    pub(crate) fn roles(&self) -> &'a RuntimeEntryRoles {
        match self {
            Self::Plan(entry) => &entry.roles,
            Self::Awbc { entry, .. } => &entry.roles,
        }
    }

    pub(crate) fn matches_kind(&self, kind: &RuntimeEntryKind) -> bool {
        match self {
            Self::Plan(entry) => &entry.kind == kind,
            Self::Awbc { entry, strings } => {
                entry.kind.runtime_kind(strings).as_ref() == Some(kind)
            }
        }
    }
}
