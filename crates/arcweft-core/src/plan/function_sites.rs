//! Plan-owned structured function bodies.

use std::num::NonZeroU32;

use thiserror::Error;

use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::value::RuntimeExpr;

/// One typed structured function body owned by its complete runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunctionSite {
    params: Box<[RuntimeLocalDeclarationId]>,
    captures: Box<[RuntimeLocalDeclarationId]>,
    body: RuntimeExpr,
}

impl RuntimeFunctionSite {
    #[must_use]
    pub const fn params(&self) -> &[RuntimeLocalDeclarationId] {
        &self.params
    }

    #[must_use]
    pub const fn body(&self) -> &RuntimeExpr {
        &self.body
    }

    #[must_use]
    pub const fn captures(&self) -> &[RuntimeLocalDeclarationId] {
        &self.captures
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeFunctionSiteTable {
    sites: Box<[RuntimeFunctionSite]>,
}

impl RuntimeFunctionSiteTable {
    #[must_use]
    pub fn get(&self, id: RuntimeFunctionSiteId) -> Option<&RuntimeFunctionSite> {
        usize::try_from(id.get().get() - 1)
            .ok()
            .and_then(|index| self.sites.get(index))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeFunctionSiteTableBuilder {
    sites: Vec<RuntimeFunctionSite>,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeFunctionSiteError {
    #[error("runtime function-site identity space is exhausted")]
    IdentityExhausted,
}

impl RuntimeFunctionSiteTableBuilder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { sites: Vec::new() }
    }

    pub(crate) fn push(
        &mut self,
        params: Box<[RuntimeLocalDeclarationId]>,
        captures: Box<[RuntimeLocalDeclarationId]>,
        body: RuntimeExpr,
    ) -> Result<RuntimeFunctionSiteId, RuntimeFunctionSiteError> {
        let ordinal = self
            .sites
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeFunctionSiteError::IdentityExhausted)?;
        self.sites.push(RuntimeFunctionSite {
            params,
            captures,
            body,
        });
        Ok(RuntimeFunctionSiteId::from_accepted_ordinal(ordinal))
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeFunctionSiteTable {
        RuntimeFunctionSiteTable {
            sites: self.sites.into_boxed_slice(),
        }
    }
}
