//! Borrowed key framing is fully admitted before producing the Id-owned key.

use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};

use super::{Counts, Error, cursor::Cursor};

pub(super) struct RegistryKeySource<'a> {
    bytes: &'a [u8],
}

impl<'a> RegistryKeySource<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn preflight(&self, counts: &mut Counts) -> Result<(), Error> {
        let mut cursor = Cursor::new(self.bytes);
        let scope = cursor.text()?;
        counts.storage(scope.len())?;
        let count = cursor.count()?;
        counts.array(count, size_of::<DeclarationName>())?;
        for _ in 0..count {
            counts.storage(cursor.text()?.len())?;
        }
        cursor.finish()?;
        // Finish resource admission of the entire key before grammar errors
        // may retain a bounded offending name in their structured diagnostic.
        DeclarationName::validate_source(scope)?;
        let mut cursor = Cursor::new(self.bytes);
        cursor.text()?;
        cursor.count()?;
        for _ in 0..count {
            DeclarationName::validate_source(cursor.text()?)?;
        }
        cursor.finish()
    }

    pub fn materialize(&self) -> Result<LifetimeRegistryKey, Error> {
        let mut cursor = Cursor::new(self.bytes);
        let scope = LifetimeScopeKind::parse(cursor.text()?)?;
        let count = cursor.count()?;
        let segments = (0..count)
            .map(|_| Ok(DeclarationName::try_new(cursor.text()?)?))
            .collect::<Result<Box<[_]>, Error>>()?;
        cursor.finish()?;
        Ok(LifetimeRegistryKey::new(scope, segments))
    }
}
