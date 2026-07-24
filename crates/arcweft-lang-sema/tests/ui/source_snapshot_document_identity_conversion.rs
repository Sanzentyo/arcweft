use arcweft_source::{SourceDocumentIdentity, SourceName, identity::SourceSnapshotId};

fn main() {
    let snapshot = SourceSnapshotId::initial(SourceName::path("story.arcw"));
    let _: SourceDocumentIdentity = snapshot.into();
}
