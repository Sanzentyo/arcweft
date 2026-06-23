use arcweft_bundle::container::{BundleView, ReadBudget, SectionDescriptor};
use clap::Args;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Args)]
pub(in crate::app) struct InspectOptions {
    /// AWFB bundle to inspect.
    pub(in crate::app) path: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(in crate::app) json: bool,
    /// Include the canonical manifest bytes as UTF-8 text when possible.
    #[arg(long)]
    pub(in crate::app) manifest: bool,
}

#[derive(Debug, Serialize)]
struct InspectReport {
    path: String,
    kind: String,
    content_root: String,
    manifest_bytes: usize,
    sections: Vec<InspectSectionReport>,
    skipped_optional_sections: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_text: Option<String>,
}

#[derive(Debug, Serialize)]
struct InspectSectionReport {
    id: String,
    kind: String,
    schema_version: u32,
    residency: String,
    placement: String,
    compression: String,
    required: bool,
    offset: u64,
    stored_size: u64,
    decoded_size: u64,
    stored_digest: String,
    content_digest: String,
}

pub(super) fn inspect_command(options: &InspectOptions) -> Result<(), ExitCode> {
    let bytes = fs::read(&options.path).map_err(|error| {
        eprintln!(
            "error: failed to read bundle {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let view = BundleView::parse(&bytes, ReadBudget::default()).map_err(|error| {
        eprintln!(
            "error: failed to inspect AWFB bundle {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = inspect_report(&options.path, &view, options.manifest);
    if options.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report).map_err(|error| {
            eprintln!("error: failed to write inspect JSON: {error}");
            ExitCode::FAILURE
        })?;
        println!();
    } else {
        print_text_report(&report);
    }
    Ok(())
}

fn inspect_report(path: &Path, view: &BundleView<'_>, include_manifest: bool) -> InspectReport {
    InspectReport {
        path: path.display().to_string(),
        kind: format!("{:?}", view.kind()),
        content_root: view.content_root().to_string(),
        manifest_bytes: view.manifest().len(),
        sections: view.sections().iter().map(section_report).collect(),
        skipped_optional_sections: view.skipped_optional_sections(),
        manifest_text: include_manifest
            .then(|| String::from_utf8_lossy(view.manifest()).into_owned()),
    }
}

fn section_report(section: &SectionDescriptor) -> InspectSectionReport {
    InspectSectionReport {
        id: section.id().to_string(),
        kind: format!("{:?}", section.kind()),
        schema_version: section.schema_version(),
        residency: section.residency().to_string(),
        placement: section.placement().to_string(),
        compression: section.compression().to_string(),
        required: section.required(),
        offset: section.offset(),
        stored_size: section.stored_size(),
        decoded_size: section.decoded_size(),
        stored_digest: section.stored_digest().to_string(),
        content_digest: section.content_digest().to_string(),
    }
}

fn print_text_report(report: &InspectReport) {
    println!("path: {}", report.path);
    println!("kind: {}", report.kind);
    println!("content root: {}", report.content_root);
    println!("manifest bytes: {}", report.manifest_bytes);
    println!("sections: {}", report.sections.len());
    if report.skipped_optional_sections != 0 {
        println!(
            "skipped optional sections: {}",
            report.skipped_optional_sections
        );
    }
    for section in &report.sections {
        println!(
            "- {} {} schema={} residency={} placement={} required={} stored={} decoded={} content={}",
            section.id,
            section.kind,
            section.schema_version,
            section.residency,
            section.placement,
            section.required,
            section.stored_size,
            section.decoded_size,
            section.content_digest
        );
    }
    if let Some(manifest) = &report.manifest_text {
        println!("manifest:");
        println!("{manifest}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::container::{
        BundleKind, BundleSectionKind, ContentResidency, SectionId, SectionInput, encode_bundle,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inspect_report_describes_awfb_sections() {
        let bytes = encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([7; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                b"asset-bytes",
            )],
        )
        .expect("bundle encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("bundle parses");

        let report = inspect_report(Path::new("content.awfb"), &view, true);

        assert_eq!(report.kind, "ContentPack");
        assert_eq!(
            report.manifest_text.as_deref(),
            Some(r#"{"kind":"content"}"#)
        );
        assert_eq!(report.sections.len(), 1);
        assert_eq!(report.sections[0].kind, "AssetBlob");
        assert_eq!(report.sections[0].placement, "Embedded");
        assert_eq!(report.sections[0].decoded_size, 11);
    }

    #[test]
    fn inspect_command_rejects_non_awfb_input() {
        let path = temp_path("not-awfb");
        fs::write(&path, b"not an awfb").expect("fixture writes");

        let result = inspect_command(&InspectOptions {
            path: path.clone(),
            json: true,
            manifest: false,
        });
        let _ = fs::remove_file(path);

        assert_eq!(result, Err(ExitCode::FAILURE));
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "arcweft-inspect-{label}-{}-{nanos}.awfb",
            std::process::id()
        ))
    }
}
