fn imq_is_available() -> bool {
    Command::new("imq")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

struct NativeCliFixture {
    path: PathBuf,
    entry: EntryRuntimeId,
}

impl NativeCliFixture {
    fn new(path: PathBuf, entry: EntryRuntimeId) -> Self {
        Self { path, entry }
    }

    fn checked_in(path: PathBuf, entry: &str) -> Self {
        Self::new(
            path,
            EntryRuntimeId::from_source_entity_body(entry).expect("fixture entry ID"),
        )
    }

    fn agent_command(&self, operation: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
        command
            .arg("agent")
            .arg(operation)
            .arg(&self.path)
            .arg("--entry")
            .arg(self.entry.public_label().as_str());
        command
    }

    fn text_capture_command(&self, target: NativeTextCaptureTarget) -> NativeTextCaptureCommand {
        NativeTextCaptureCommand {
            command: self.agent_command("observe"),
            target,
        }
    }

    fn text_capture_command_at_entry(
        &self,
        entry: &EntryRuntimeId,
        target: NativeTextCaptureTarget,
    ) -> NativeTextCaptureCommand {
        assert_eq!(
            &self.entry, entry,
            "capture uses the caller-authored fixture launch"
        );
        self.text_capture_command(target)
    }
}

/// Caller-authored text identity. The command resolves this against an issued
/// observation with the same explicit launch, step, viewport and sample inputs.
/// Object descriptors are returned by that observation and never decoded here.
#[derive(Clone, Copy)]
struct NativeTextCaptureTarget {
    dialogue: u64,
    entry: u64,
    element: Option<NativeTextElementTarget>,
}

#[derive(Clone, Copy)]
enum NativeTextElementTarget {
    Page(usize),
    Line(usize),
    Run(usize),
    Ruby(usize),
    Glyph {
        index: usize,
        range: arcweft_text_model::RichTextRange,
    },
    Cluster {
        index: usize,
        range: arcweft_text_model::RichTextRange,
    },
}

impl NativeTextCaptureTarget {
    const fn content(dialogue: u64, entry: u64) -> Self {
        Self {
            dialogue,
            entry,
            element: None,
        }
    }
    const fn element(dialogue: u64, entry: u64, element: NativeTextElementTarget) -> Self {
        Self {
            dialogue,
            entry,
            element: Some(element),
        }
    }

    fn matches(
        self,
        catalog: &arcweft_agent_protocol::text_context::AgentTextContextCatalog,
        object: &arcweft_agent_protocol::object::AgentObservedObject,
    ) -> bool {
        let Some(reference) = &object.text_context else {
            return false;
        };
        let entry = catalog
            .resolve(reference)
            .expect("issued fixture reference resolves");
        if !matches!(entry.owner.kind, arcweft_text_model::PreparedTextOwnerKind::DialogueView { dialogue, entry, role: arcweft_text_model::DialoguePreparedTextRole::Content, .. } if dialogue == self.dialogue && entry == self.entry)
        {
            return false;
        }
        match (self.element, object.rich_text_ref.as_ref()) {
            (None, None) => true,
            (Some(expected), Some(actual)) => expected.matches(actual),
            _ => false,
        }
    }
}

impl NativeTextElementTarget {
    fn matches(self, actual: &arcweft_agent_protocol::rich_text::AgentRichTextElementRef) -> bool {
        use arcweft_agent_protocol::rich_text::AgentRichTextElementKind as Kind;
        match self {
            Self::Page(page) => {
                actual.kind == Kind::TextPage && actual.page == page && actual.index == page
            }
            Self::Line(index) => actual.kind == Kind::TextLine && actual.index == index,
            Self::Run(index) => actual.kind == Kind::TextRun && actual.index == index,
            Self::Ruby(index) => actual.kind == Kind::Ruby && actual.index == index,
            Self::Glyph { index, range } => {
                actual.kind == Kind::TextGlyph && actual.index == index && actual.range == range
            }
            Self::Cluster { index, range } => {
                actual.kind == Kind::GlyphCluster && actual.index == index && actual.range == range
            }
        }
    }
}

struct NativeTextCaptureCommand {
    command: Command,
    target: NativeTextCaptureTarget,
}

impl NativeTextCaptureCommand {
    fn arg(&mut self, argument: impl AsRef<std::ffi::OsStr>) -> &mut Self {
        self.command.arg(argument);
        self
    }

    fn output(&mut self) -> std::io::Result<std::process::Output> {
        let mut inspect = Command::new(self.command.get_program());
        let mut capture = Command::new(self.command.get_program());
        if let Some(directory) = self.command.get_current_dir() {
            inspect.current_dir(directory);
        }
        if let Some(directory) = self.command.get_current_dir() {
            capture.current_dir(directory);
        }
        for (name, value) in self.command.get_envs() {
            if let Some(value) = value {
                inspect.env(name, value);
            } else {
                inspect.env_remove(name);
            }
        }
        for (name, value) in self.command.get_envs() {
            if let Some(value) = value {
                capture.env(name, value);
            } else {
                capture.env_remove(name);
            }
        }
        capture.args(self.command.get_args());
        let mut arguments = self.command.get_args();
        while let Some(argument) = arguments.next() {
            if ["--image", "--capture", "--out"]
                .iter()
                .any(|flag| argument == std::ffi::OsStr::new(flag))
            {
                arguments
                    .next()
                    .expect("typed capture option has its value");
            } else {
                assert_ne!(
                    argument,
                    std::ffi::OsStr::new("--object"),
                    "the caller-authored typed target owns object selection"
                );
                inspect.arg(argument);
            }
        }
        let observed = inspect.output()?;
        if !observed.status.success() {
            return Err(std::io::Error::other(format!(
                "typed capture observation failed: {}",
                String::from_utf8_lossy(&observed.stderr)
            )));
        }
        let report = arcweft_agent_protocol::observation::AgentObservationReport::decode_json(
            &observed.stdout,
        )
        .map_err(std::io::Error::other)?;
        let candidates = report
            .objects
            .iter()
            .filter(|object| self.target.matches(&report.text_contexts, object))
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(std::io::Error::other(format!(
                "caller-authored capture target requires one exact issued object; found {}: {}",
                candidates.len(),
                String::from_utf8_lossy(&observed.stdout)
            )));
        }
        capture.arg("--object").arg(&candidates[0].id).output()
    }
}

impl AsRef<Path> for NativeCliFixture {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<std::ffi::OsStr> for NativeCliFixture {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path.as_os_str()
    }
}

impl std::ops::Deref for NativeCliFixture {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

/// The caller authors a flow.main fixture; the helper authors its typed launch.
fn temp_native_cli_fixture(name: &str, source: &str) -> NativeCliFixture {
    let entry = EntryRuntimeId::from_source_entity_body("entry.native_test")
        .expect("native fixture entry ID");
    let source = format!("entry cli @entry.native_test {{ goto @flow.main }}\n{source}");
    NativeCliFixture::new(temp_arcw(name, &source), entry)
}

fn temp_authored_native_cli_fixture(name: &str, source: &str, entry: &str) -> NativeCliFixture {
    NativeCliFixture::checked_in(temp_arcw(name, source), entry)
}

fn temp_main_cli_fixture(name: &str, source: &str) -> (NativeCliFixture, EntryRuntimeId) {
    let fixture = temp_native_cli_fixture(name, source);
    let entry = fixture.entry.clone();
    (fixture, entry)
}

fn capture_native_png_report(
    source_path: &NativeCliFixture,
    entry: &EntryRuntimeId,
    png_path: &Path,
) -> serde_json::Value {
    assert_eq!(&source_path.entry, entry, "capture uses the fixture launch");
    let output = source_path
        .agent_command("observe")
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--out")
        .arg(png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native PNG");

    assert!(
        output.status.success(),
        "native PNG capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(png_path).expect("read native PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    serde_json::from_slice(&output.stdout).expect("native capture report is JSON")
}

fn observe_native_dialogue_view_object_raw_report(
    source_path: &NativeCliFixture,
    raw_path: &Path,
    capture_kind: &str,
) -> serde_json::Value {
    let output = source_path
        .text_capture_command(NativeTextCaptureTarget::content(0, 0))
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native dialogue_view object raw crop");

    assert!(
        output.status.success(),
        "native dialogue_view object {capture_kind} capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native dialogue_view object raw report is JSON");
    let width = json["images"][0]["width"]
        .as_u64()
        .expect("native dialogue_view object raw width is reported");
    let height = json["images"][0]["height"]
        .as_u64()
        .expect("native dialogue_view object raw height is reported");
    assert_eq!(
        fs::read(raw_path)
            .expect("read native dialogue_view object raw crop")
            .len(),
        4 * usize::try_from(width).expect("raw width fits usize")
            * usize::try_from(height).expect("raw height fits usize")
    );
    json
}

fn observe_native_rich_text_layer_report(source_path: &NativeCliFixture) -> serde_json::Value {
    observe_native_rich_text_layer_report_with_viewport(source_path, 1280, 720)
}

fn observe_native_rich_text_layer_report_at_entry(
    source_path: &NativeCliFixture,
    entry: &EntryRuntimeId,
) -> serde_json::Value {
    observe_native_rich_text_layer_report_at_entry_with_viewport(source_path, entry, 1280, 720)
}

fn observe_native_rich_text_layer_report_at_entry_with_viewport(
    source_path: &NativeCliFixture,
    entry: &EntryRuntimeId,
    viewport_width: u32,
    viewport_height: u32,
) -> serde_json::Value {
    assert_eq!(&source_path.entry, entry, "observe uses the fixture launch");
    let command =
        native_rich_text_layer_observe_command(source_path, viewport_width, viewport_height);
    run_native_rich_text_layer_observe(command)
}

fn observe_native_rich_text_layer_report_with_viewport(
    source_path: &NativeCliFixture,
    viewport_width: u32,
    viewport_height: u32,
) -> serde_json::Value {
    run_native_rich_text_layer_observe(native_rich_text_layer_observe_command(
        source_path,
        viewport_width,
        viewport_height,
    ))
}

fn native_rich_text_layer_observe_command(
    source_path: &NativeCliFixture,
    viewport_width: u32,
    viewport_height: u32,
) -> Command {
    let mut command = source_path.agent_command("observe");
    command
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--viewport-width")
        .arg(viewport_width.to_string())
        .arg("--viewport-height")
        .arg(viewport_height.to_string());
    command
}

fn run_native_rich_text_layer_observe(mut command: Command) -> serde_json::Value {
    let output = command
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reports native rich-text layer");

    assert!(
        output.status.success(),
        "native rich-text layer observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("native rich-text layer report is JSON")
}

fn find_dialogue_view_object(report: &serde_json::Value) -> &serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "dialogue_view"
                && object["content"]["kind"] == "rich_text"
                && object["rich_text_ref"].is_null()
        })
        .unwrap_or_else(|| panic!("dialogue_view object should be observed: {report}"))
}

fn find_native_text_target_object(
    report: &serde_json::Value,
    target: NativeTextCaptureTarget,
) -> &serde_json::Value {
    let typed: arcweft_agent_protocol::observation::AgentObservationReport =
        serde_json::from_value(report.clone())
            .expect("the report has an admitted text context catalog");
    let matches = typed
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| target.matches(&typed.text_contexts, object))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "the caller-authored target has one exact issued object: {report}"
    );
    &report["objects"][matches[0]]
}

fn issued_text_capture_uri<'a>(
    report: &'a serde_json::Value,
    target: NativeTextCaptureTarget,
    kind: &str,
    mime_type: &str,
) -> &'a str {
    let object = find_native_text_target_object(report, target);
    let captures = object["capture_refs"]["captures"]
        .as_array()
        .expect("the issued object carries capture refs");
    let matches = captures
        .iter()
        .filter(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "the issued object has one capture of the requested closed format: {object}"
    );
    matches[0]["uri"].as_str().expect("issued capture URI")
}

fn find_dialogue_view_object_by_rich_text_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "dialogue_view"
                && object["content"]["kind"] == "rich_text"
                && observed_object_rich_text_line(report, object) == fixture_runtime_line(line)
        })
        .unwrap_or_else(|| {
            panic!("dialogue_view object for rich-text line `{line}` should be observed: {report}")
        })
}

fn observed_object_rich_text_frame<'a>(
    report: &'a serde_json::Value,
    object: &serde_json::Value,
) -> &'a serde_json::Value {
    assert_eq!(object["content"]["kind"], "rich_text");
    assert!(
        object["content"].get("frame").is_none(),
        "the owning catalog carries the frame once: {object}"
    );
    let catalog = &report["text_contexts"];
    assert_eq!(catalog["version"], 1);
    assert_eq!(catalog["scope"]["session_id"], report["session_id"]);
    assert_eq!(catalog["scope"]["frame_id"], report["frame_id"]);
    assert_eq!(catalog["scope"]["tick"], report["tick"]);
    assert_eq!(
        catalog["scope"]["capture_time_millis"],
        report["capture_time_millis"]
    );
    assert_eq!(object["text_context"]["scope"], catalog["scope"]);
    let owner = object["text_context"]["owner"]
        .as_u64()
        .expect("issued text context owner ordinal");
    let row = catalog["entries"]
        .as_array()
        .expect("issued text context rows")
        .iter()
        .find(|row| row["owner_ordinal"].as_u64() == Some(owner))
        .expect("the issued text context row is present");
    assert_eq!(row["content"]["kind"], "display_stage");
    &row["content"]["context"]["frame"]
}

fn fixture_runtime_line(label: &str) -> arcweft_core::plan::RuntimeLineId {
    arcweft_core::plan::RuntimeLineId::from_runtime_line_value(label)
        .expect("caller-authored runtime line identity")
}

fn observed_object_rich_text_line(
    report: &serde_json::Value,
    object: &serde_json::Value,
) -> arcweft_core::plan::RuntimeLineId {
    serde_json::from_value(observed_object_rich_text_frame(report, object)["line"].clone())
        .expect("issued frame preserves typed runtime line identity")
}

fn rich_text_text_runs<'a>(
    report: &'a serde_json::Value,
    dialogue_view: &serde_json::Value,
) -> &'a [serde_json::Value] {
    observed_object_rich_text_frame(report, dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("dialogue_view display_map should expose text_runs: {dialogue_view}")
        })
}

fn rich_text_text_run_has_effect(
    report: &serde_json::Value,
    dialogue_view: &serde_json::Value,
    id: &str,
) -> bool {
    rich_text_text_run_effect_count(report, dialogue_view, id) > 0
}

fn rich_text_text_run_effect_count(
    report: &serde_json::Value,
    dialogue_view: &serde_json::Value,
    id: &str,
) -> usize {
    rich_text_text_runs(report, dialogue_view)
        .iter()
        .flat_map(|run| run["presentation"]["fx"].as_array().into_iter().flatten())
        .filter(|effect| effect["definition"]["function"] == id)
        .count()
}

fn rich_text_text_run_has_shader(
    report: &serde_json::Value,
    dialogue_view: &serde_json::Value,
    id: &str,
) -> bool {
    rich_text_text_runs(report, dialogue_view)
        .iter()
        .any(|run| {
            run["presentation"]["fx"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|shader| {
                    shader["definition"]["function"] == "shader"
                        && shader["static_"].as_array().is_some_and(|arguments| {
                            arguments.iter().any(|argument| {
                                argument["kind"] == "resource" && argument["value"] == id
                            })
                        })
                })
        })
}

fn rich_text_text_run_has_object_proxy(
    report: &serde_json::Value,
    dialogue_view: &serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    rich_text_text_runs(report, dialogue_view)
        .iter()
        .any(|run| {
            run["presentation"]["object_proxies"]
                .as_array()
                .into_iter()
                .flatten()
                .any(&predicate)
        })
}

fn assert_full_grammar_text_object_proxy_hit_region(json: &serde_json::Value) {
    let proxy_run = find_rich_text_run_object(json, "proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 4000);
    let proxy_hit = rich_text_hit_region(
        proxy_run,
        "text_object_proxy",
        proxy_run["rich_text_ref"]["range"]["start"]
            .as_u64()
            .expect("proxy range start"),
        proxy_run["rich_text_ref"]["range"]["end"]
            .as_u64()
            .expect("proxy range end"),
    );
    assert_eq!(proxy_hit["proxy_id"], "hotspot");
    assert_eq!(proxy_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&proxy_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(proxy_hit["proxy_role"], "keyword");
    assert!(proxy_hit["proxy_layer"].is_null());
    assert_eq!(proxy_hit["depth"], 4000);
}

fn assert_full_grammar_text_object_proxy_observed_object(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
) {
    let proxy_object = find_rich_text_proxy_object(json, "hotspot", "proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    let parent_id = rich_text_proxy_parent_id(proxy_object);
    assert_eq!(proxy_object["parent_id"], parent_id);
    let proxy_object_id = proxy_object["id"].as_str().expect("proxy object id");
    assert_rich_text_proxy_presentation_tree_node(
        json,
        proxy_object_id,
        &parent_id,
        "hotspot",
        4000,
    );
    assert_eq!(proxy_object["text"], "proxy");
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["index"], 0);
    assert_eq!(proxy_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], 4000);
    assert_eq!(
        proxy_object["rich_text_ref"]["presentation"]["object_proxies"]
            .as_array()
            .expect("proxy object presentation should expose object_proxies")
            .len(),
        1
    );
    let proxy = &proxy_object["rich_text_ref"]["presentation"]["object_proxies"][0];
    assert_eq!(proxy["id"], "hotspot");
    assert_eq!(proxy["type_name"], "KeywordHit");
    assert_text_proxy_declaration(&proxy["declaration"], "KeywordHit");
    assert_eq!(proxy["role"], "keyword");
    assert!(proxy["layer"].is_null());
    assert_eq!(proxy["depth"], 4000);
    assert_eq!(proxy["hit_test"], true);
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("proxy object bbox height");
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["parent_id"],
        parent_id
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["declaration"]["struct_name"],
        "KeywordHit"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"]["channel"]["value"],
        "choice"
    );
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_hit = rich_text_hit_region(
        proxy_object,
        "text_object_proxy",
        proxy_object["rich_text_ref"]["range"]["start"]
            .as_u64()
            .expect("proxy range start"),
        proxy_object["rich_text_ref"]["range"]["end"]
            .as_u64()
            .expect("proxy range end"),
    );
    assert_eq!(proxy_hit["proxy_id"], "hotspot");
    assert_eq!(proxy_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&proxy_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(proxy_hit["proxy_role"], "keyword");
    assert!(proxy_hit["proxy_layer"].is_null());
    assert_eq!(proxy_hit["depth"], 4000);
}

fn assert_full_grammar_nested_text_object_proxies(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
) {
    let proxy_run = find_rich_text_run_object(json, "multi proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 7000);
    let proxies = proxy_run["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("nested proxy run should expose object_proxies");
    assert_eq!(
        proxies.len(),
        2,
        "nested proxy run should carry both authored proxy objects: {proxy_run}"
    );
    let keyword = proxies
        .iter()
        .find(|proxy| proxy["id"] == "hotspot2")
        .expect("nested proxy run should keep outer keyword proxy");
    assert_eq!(keyword["type_name"], "KeywordHit");
    assert_text_proxy_declaration(&keyword["declaration"], "KeywordHit");
    assert_eq!(keyword["role"], "keyword");
    assert!(keyword["layer"].is_null());
    assert_eq!(keyword["depth"], 4000);
    assert_eq!(keyword["hit_test"], true);
    assert_eq!(text_proxy_field(keyword, "channel")["value"], "inventory");
    let hover = proxies
        .iter()
        .find(|proxy| proxy["id"] == "hover")
        .expect("nested proxy run should keep inner hover proxy");
    assert_eq!(hover["type_name"], "HoverHit");
    assert_text_proxy_declaration(&hover["declaration"], "HoverHit");
    assert_eq!(hover["role"], "hover");
    assert_eq!(hover["layer"], "view");
    assert_eq!(hover["depth"], 7000);
    assert_eq!(hover["hit_test"], true);
    assert_eq!(text_proxy_field(hover, "tone")["value"], "alert");
    assert!(hover["params"]["layer"].is_null());

    let range_start = proxy_run["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("nested proxy range start");
    let range_end = proxy_run["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("nested proxy range end");
    let keyword_hit = rich_text_proxy_hit_region(proxy_run, "hotspot2", range_start, range_end);
    assert_eq!(keyword_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&keyword_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(keyword_hit["proxy_role"], "keyword");
    assert_eq!(keyword_hit["depth"], 4000);
    let hover_hit = rich_text_proxy_hit_region(proxy_run, "hover", range_start, range_end);
    assert_eq!(hover_hit["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(&hover_hit["proxy_declaration"], "HoverHit");
    assert_eq!(hover_hit["proxy_role"], "hover");
    assert_eq!(hover_hit["proxy_layer"], "view");
    assert_eq!(hover_hit["depth"], 7000);

    assert_full_grammar_nested_proxy_observed_object(
        source_path,
        json,
        "hotspot2",
        "KeywordHit",
        "keyword",
        4000,
        ("channel", "inventory"),
    );
    assert_full_grammar_nested_proxy_observed_object(
        source_path,
        json,
        "hover",
        "HoverHit",
        "hover",
        7000,
        ("tone", "alert"),
    );
}

fn assert_full_grammar_nested_proxy_observed_object(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
    proxy_id: &str,
    proxy_type: &str,
    proxy_role: &str,
    depth: i64,
    expected_param: (&str, &str),
) {
    let proxy_object = find_rich_text_proxy_object(json, proxy_id, "multi proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    let parent_id = rich_text_proxy_parent_id(proxy_object);
    assert_eq!(proxy_object["parent_id"], parent_id);
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], depth);
    let proxies = proxy_object["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("proxy object presentation should expose object_proxies");
    let [proxy] = proxies.as_slice() else {
        panic!("observed proxy object should carry only its selected proxy: {proxy_object}");
    };
    assert_eq!(proxy["id"], proxy_id);
    assert_eq!(proxy["type_name"], proxy_type);
    assert_text_proxy_declaration(&proxy["declaration"], proxy_type);
    assert_eq!(proxy["role"], proxy_role);
    if proxy_id == "hover" {
        assert_eq!(proxy["layer"], "view");
        assert_eq!(proxy_object["rich_text_ref"]["object_layer"], "view");
        assert_eq!(
            proxy_object["rich_text_ref"]["hit_regions"][0]["proxy_layer"],
            "view"
        );
    } else {
        assert!(proxy["layer"].is_null());
        assert!(proxy_object["rich_text_ref"]["object_layer"].is_null());
    }
    assert_eq!(proxy["depth"], depth);
    assert_eq!(proxy["hit_test"], true);
    assert_eq!(
        proxy["params"][expected_param.0]["value"], expected_param.1,
        "observed proxy object should keep typed params: {proxy_object}"
    );
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_id = proxy_object["id"].as_str().expect("nested proxy object id");
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("nested proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("nested proxy object bbox height");
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["parent_id"],
        parent_id
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"][expected_param.0]["value"],
        expected_param.1
    );
}

fn assert_full_grammar_typed_text_object_proxy(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
) {
    let proxy_run = find_rich_text_run_object(json, "typed proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 4000);
    let proxies = proxy_run["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("typed proxy run should expose object_proxies");
    let [proxy] = proxies.as_slice() else {
        panic!("typed proxy run should carry one proxy: {proxy_run}");
    };
    assert_eq!(proxy["id"], "KeywordHit");
    assert_eq!(proxy["type_name"], "KeywordHit");
    assert_eq!(proxy["role"], "keyword");
    assert_eq!(proxy["depth"], 4000);
    assert_eq!(proxy["hit_test"], true);
    assert_eq!(text_proxy_field(proxy, "channel")["value"], "typed");

    let proxy_object = find_rich_text_proxy_object(json, "KeywordHit", "typed proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], 4000);
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_id = proxy_object["id"].as_str().expect("typed proxy object id");
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("typed proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("typed proxy object bbox height");
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"]["channel"]["value"],
        "typed"
    );
}

fn assert_full_grammar_presentation_scalar_depth(json: &serde_json::Value) {
    let run = find_rich_text_run_object(json, "z depth");
    assert_eq!(run["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(run["rich_text_ref"]["presentation"]["z_index"], 3);
    assert_eq!(run["rich_text_ref"]["presentation"]["opacity"], 800);
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["hover"]["value"],
        true
    );
    assert_eq!(run["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(run["rich_text_ref"]["object_depth"], 3000);
}

fn assert_full_grammar_text_page_object_readback(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
) {
    let page_object = find_rich_text_page_object_by_line(json, "say.full.006", 0);
    assert_eq!(page_object["role"], "rich_text_page");
    assert_eq!(page_object["rich_text_ref"]["kind"], "text_page");
    assert_eq!(page_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        page_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(page_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(page_object["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        page_object["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(page_object["rich_text_ref"]["object_layer"], "view");
    assert_eq!(page_object["rich_text_ref"]["object_depth"], 7000);
    assert!(
        page_object["text"]
            .as_str()
            .is_some_and(|text| text.contains("明示family") && text.contains("proxy")),
        "full grammar text page object should expose the page text: {page_object}"
    );
    let range_start = page_object["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("text page range start");
    let range_end = page_object["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("text page range end");
    assert!(range_end > range_start);
    assert_rich_text_hit_region_matches_bbox(page_object, "text_page", range_start, range_end);
    assert_agent_observe_object_capture_refs(page_object);

    let page_object_id = page_object["id"].as_str().expect("text page object id");
    let page_object_width = page_object["bbox"]["width"]
        .as_u64()
        .expect("text page object bbox width");
    let page_object_height = page_object["bbox"]["height"]
        .as_u64()
        .expect("text page object bbox height");
    let page_mask_uri =
        rich_text_object_capture_uri(page_object, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        source_path,
        page_mask_uri,
        page_object_id,
        page_object_width,
        page_object_height,
    );
    let page_object_id_uri =
        rich_text_object_capture_uri(page_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        page_object_id_uri,
        page_object,
        page_object_width,
        page_object_height,
    );
}

fn assert_full_grammar_text_line_object_readback(
    source_path: &NativeCliFixture,
    json: &serde_json::Value,
) {
    let line_object = find_rich_text_line_object_by_line(json, "say.full.006", 0);
    assert_eq!(line_object["role"], "rich_text_line");
    assert_eq!(line_object["rich_text_ref"]["kind"], "text_line");
    assert_eq!(line_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        line_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(line_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(line_object["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        line_object["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(line_object["rich_text_ref"]["object_layer"], "view");
    assert_eq!(line_object["rich_text_ref"]["object_depth"], 7000);
    assert!(
        line_object["text"]
            .as_str()
            .is_some_and(|text| text.contains("明示family") && text.contains("proxy")),
        "full grammar text line object should expose the authored line text: {line_object}"
    );
    let range_start = line_object["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("text line range start");
    let range_end = line_object["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("text line range end");
    assert!(range_end > range_start);
    assert_rich_text_hit_region_matches_bbox(line_object, "text_line", range_start, range_end);
    assert_agent_observe_object_capture_refs(line_object);

    let line_object_id = line_object["id"].as_str().expect("text line object id");
    let line_object_width = line_object["bbox"]["width"]
        .as_u64()
        .expect("text line object bbox width");
    let line_object_height = line_object["bbox"]["height"]
        .as_u64()
        .expect("text line object bbox height");
    let line_mask_uri =
        rich_text_object_capture_uri(line_object, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        source_path,
        line_mask_uri,
        line_object_id,
        line_object_width,
        line_object_height,
    );
    let line_object_id_uri =
        rich_text_object_capture_uri(line_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        line_object_id_uri,
        line_object,
        line_object_width,
        line_object_height,
    );
}

fn rich_text_text_run_has_transform(
    report: &serde_json::Value,
    dialogue_view: &serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    rich_text_text_runs(report, dialogue_view)
        .iter()
        .any(|run| {
            let transform = &run["presentation"]["transform"];
            !transform.is_null() && predicate(transform)
        })
}

fn assert_object_capture_ref_matches_image(
    object: &serde_json::Value,
    image: &serde_json::Value,
    kind: &str,
    mime_type: &str,
) {
    let capture = object["capture_refs"]["captures"]
        .as_array()
        .expect("object capture refs are reported")
        .iter()
        .find(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .unwrap_or_else(|| panic!("object should expose {kind} {mime_type} capture ref: {object}"));
    assert_eq!(
        capture["width"], image["width"],
        "{kind} capture ref width should match actual image metadata"
    );
    assert_eq!(
        capture["height"], image["height"],
        "{kind} capture ref height should match actual image metadata"
    );
}

fn assert_native_rich_text_layer_image_has_content(report: &serde_json::Value) {
    let image = &report["images"][0];
    assert_eq!(image["kind"], "color");
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "layer");
    assert_eq!(image["scope"]["id"], "dialogue.rich_text");
    assert_eq!(image["composition"], "masked_framebuffer_crop");
    assert_eq!(image["mime_type"], "image/png");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);
}

fn find_rich_text_run_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "rich_text_run" && object["text"] == text)
        .unwrap_or_else(|| panic!("rich-text run `{text}` should be observed: {report}"))
}

fn find_rich_text_proxy_object<'a>(
    report: &'a serde_json::Value,
    proxy_id: &str,
    text: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_proxy"
                && object["text"] == text
                && object["rich_text_ref"]["presentation"]["object_proxies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|proxy| proxy["id"] == proxy_id)
        })
        .unwrap_or_else(|| {
            panic!("rich-text proxy `{proxy_id}` for `{text}` should be observed: {report}")
        })
}

fn assert_rich_text_run_object_has_effect<'a>(
    run: &'a serde_json::Value,
    id: &str,
) -> &'a serde_json::Value {
    run["rich_text_ref"]["presentation"]["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|effect| effect["id"] == id)
        .unwrap_or_else(|| panic!("rich-text run object should carry effect `{id}`: {run}"))
}

fn assert_rich_text_run_object_has_shader<'a>(
    run: &'a serde_json::Value,
    id: &str,
) -> &'a serde_json::Value {
    run["rich_text_ref"]["presentation"]["shaders"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|shader| shader["id"] == id)
        .unwrap_or_else(|| panic!("rich-text run object should carry shader `{id}`: {run}"))
}

fn find_rich_text_cluster_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_cluster"
                && object["text"] == text
                && object["rich_text_ref"]["range"]["start"].as_u64() == Some(range_start)
                && object["rich_text_ref"]["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text cluster `{text}` {range_start}..{range_end} should be observed: {report}"
            )
        })
}

/// The issued layout owns word clustering; exact scalar/grapheme requests use
/// its Glyph elements while the complete sideways word remains one cluster.
fn checked_observed_text_element(
    catalog: &arcweft_agent_protocol::text_context::AgentTextContextCatalog,
    object: &serde_json::Value,
) -> (
    arcweft_agent_protocol::text_context::AgentTextContextReference,
    arcweft_agent_protocol::rich_text::AgentRichTextElementRef,
) {
    let context = serde_json::from_value(object["text_context"].clone())
        .expect("issued text context reference");
    let element =
        serde_json::from_value(object["rich_text_ref"].clone()).expect("issued text element");
    catalog
        .validate_element(
            object["id"].as_str().unwrap(),
            &context,
            Some(&element),
            object["text"].as_str(),
        )
        .expect("observed text/range belongs to its exact issued element");
    let owner = catalog.resolve(&context).expect("issued text owner");
    assert_eq!(
        owner.element_parent_id(Some(&element)).unwrap().as_deref(),
        object["parent_id"].as_str()
    );
    (context, element)
}

fn assert_atomic_sideways_word<'report>(
    report: &'report serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
    expected_glyphs: &[(&str, u64, u64)],
) -> &'report serde_json::Value {
    use arcweft_agent_protocol::rich_text::{AgentGlyphOrientation, AgentRichTextElementKind};
    use arcweft_agent_protocol::text_context::AgentTextContextCatalog;

    let word = find_rich_text_cluster_object(report, text, range_start, range_end);
    let catalog: AgentTextContextCatalog =
        serde_json::from_value(report["text_contexts"].clone()).expect("issued word catalog");
    let (context, element) = checked_observed_text_element(&catalog, word);
    assert_eq!(element.kind, AgentRichTextElementKind::GlyphCluster);
    assert_eq!(element.orientation, Some(AgentGlyphOrientation::SidewaysCw));
    let entry = catalog.resolve(&context).expect("word owner resolves");
    let layout = entry.content.layout();
    let cluster_index = u32::try_from(element.index).expect("word cluster index");
    let cluster_glyphs = layout
        .cluster_glyphs(cluster_index)
        .expect("canonical word glyphs");
    assert!(!cluster_glyphs.is_empty());

    let mut previous_ink_center = None;
    for &(glyph_text, start, end) in expected_glyphs {
        let glyph = find_rich_text_glyph_object(report, glyph_text, start, end);
        let (glyph_context, glyph_element) = checked_observed_text_element(&catalog, glyph);
        assert_eq!(glyph_context, context, "word glyph keeps its exact owner");
        assert_eq!(glyph_element.kind, AgentRichTextElementKind::TextGlyph);
        assert_eq!(
            glyph_element.orientation,
            Some(AgentGlyphOrientation::SidewaysCw)
        );
        let shaped = layout
            .glyphs
            .get(glyph_element.index)
            .expect("canonical glyph index");
        assert_eq!(
            shaped.cluster_index, cluster_index,
            "word glyph stays in the atomic cluster"
        );
        assert_eq!(
            entry
                .content
                .ranges()
                .source_to_display(shaped.source_range)
                .unwrap(),
            glyph_element.range
        );
        assert_eq!(
            glyph["bbox"], word["bbox"],
            "sideways glyph uses its owning word cell"
        );
        let ink_center = shaped.ink_bounds.y + shaped.ink_bounds.height * 0.5;
        if let Some(previous) = previous_ink_center {
            assert!(
                ink_center > previous,
                "{text}: successive glyph ink advances inside the word cell"
            );
        }
        previous_ink_center = Some(ink_center);
        assert_rich_text_object_has_mask_capture(glyph, "atomic sideways word glyph");
    }
    for glyph in cluster_glyphs {
        let range = entry
            .content
            .ranges()
            .source_to_display(glyph.source_range)
            .unwrap();
        assert!(
            expected_glyphs.iter().any(|&(_, start, end)| {
                u64::try_from(range.start).ok() == Some(start)
                    && u64::try_from(range.end).ok() == Some(end)
            }),
            "{text}: every shaped glyph retains an authored scalar/grapheme range"
        );
    }
    assert_rich_text_object_has_mask_capture(word, "atomic sideways word cluster");
    word
}

fn find_rich_text_glyph_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_glyph"
                && object["text"] == text
                && object["rich_text_ref"]["range"]["start"].as_u64() == Some(range_start)
                && object["rich_text_ref"]["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text glyph `{text}` {range_start}..{range_end} should be observed: {report}"
            )
        })
}

fn assert_sideways_cluster_glyph_cells(cluster: &serde_json::Value, glyphs: &[&serde_json::Value]) {
    assert_eq!(cluster["rich_text_ref"]["orientation"], "sideways_cw");
    let mut end = cluster["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("cluster source start");
    for glyph in glyphs {
        assert_eq!(glyph["role"], "rich_text_glyph");
        assert_eq!(glyph["rich_text_ref"]["orientation"], "sideways_cw");
        assert_eq!(
            glyph["bbox"], cluster["bbox"],
            "glyph retains its cluster cell"
        );
        assert_eq!(glyph["rich_text_ref"]["range"]["start"], end);
        end = glyph["rich_text_ref"]["range"]["end"]
            .as_u64()
            .expect("glyph source end");
    }
    assert_eq!(cluster["rich_text_ref"]["range"]["end"], end);
}

fn find_rich_text_ruby_object(report: &serde_json::Value, index: u64) -> &serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_ruby"
                && object["rich_text_ref"]["index"].as_u64() == Some(index)
        })
        .unwrap_or_else(|| panic!("rich-text ruby `{index}` should be observed: {report}"))
}

fn find_rich_text_page_object_by_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
    page: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_page"
                && observed_object_rich_text_line(report, object) == fixture_runtime_line(line)
                && object["rich_text_ref"]["page"].as_u64().unwrap_or(0) == page
        })
        .unwrap_or_else(|| {
            panic!("rich-text page `{line}` page {page} should be observed: {report}")
        })
}

fn find_rich_text_line_object_by_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
    index: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_line"
                && observed_object_rich_text_line(report, object) == fixture_runtime_line(line)
                && object["rich_text_ref"]["index"].as_u64() == Some(index)
        })
        .unwrap_or_else(|| {
            panic!("rich-text line `{line}` index {index} should be observed: {report}")
        })
}

fn first_text_run_presentation_layout(report: &serde_json::Value) -> &serde_json::Value {
    let dialogue_view = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "dialogue_view"
                && object["content"]["kind"] == "rich_text"
                && object["rich_text_ref"].is_null()
        })
        .unwrap_or_else(|| panic!("dialogue_view object should be observed: {report}"));
    let run = observed_object_rich_text_frame(report, dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are reported")
        .first()
        .unwrap_or_else(|| panic!("first text run should be observed: {report}"));
    &run["presentation"]["layout"]
}

fn assert_rich_text_cluster_metadata(
    report: &serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
    orientation: &str,
    vertical_form: &str,
) {
    let cluster = find_rich_text_cluster_object(report, text, range_start, range_end);
    assert_eq!(cluster["rich_text_ref"]["orientation"], orientation);
    assert_eq!(cluster["rich_text_ref"]["vertical_form"], vertical_form);
}

fn assert_rich_text_hit_region_matches_bbox(
    object: &serde_json::Value,
    kind: &str,
    range_start: u64,
    range_end: u64,
) {
    let region = rich_text_hit_region(object, kind, range_start, range_end);
    assert_eq!(
        region["bbox"], object["bbox"],
        "{kind} hit region should match the observed object bbox: {object}"
    );
}

fn assert_rich_text_hit_region_matches_ref_bbox(
    object: &serde_json::Value,
    kind: &str,
    ref_bbox_key: &str,
    range_start: u64,
    range_end: u64,
) {
    let region = rich_text_hit_region(object, kind, range_start, range_end);
    assert_eq!(
        region["bbox"], object["rich_text_ref"][ref_bbox_key],
        "{kind} hit region should match {ref_bbox_key}: {object}"
    );
}

fn rich_text_hit_region<'a>(
    object: &'a serde_json::Value,
    kind: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    object["rich_text_ref"]["hit_regions"]
        .as_array()
        .unwrap_or_else(|| panic!("rich-text object should expose hit_regions: {object}"))
        .iter()
        .find(|region| {
            region["kind"] == kind
                && region["range"]["start"].as_u64() == Some(range_start)
                && region["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!("rich-text object should expose {kind} hit region {range_start}..{range_end}: {object}")
        })
}

fn rich_text_proxy_hit_region<'a>(
    object: &'a serde_json::Value,
    proxy_id: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    object["rich_text_ref"]["hit_regions"]
        .as_array()
        .unwrap_or_else(|| panic!("rich-text object should expose hit_regions: {object}"))
        .iter()
        .find(|region| {
            region["kind"] == "text_object_proxy"
                && region["proxy_id"] == proxy_id
                && region["range"]["start"].as_u64() == Some(range_start)
                && region["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text object should expose text_object_proxy hit region for {proxy_id} {range_start}..{range_end}: {object}"
            )
        })
}

fn rich_text_proxy_parent_id(object: &serde_json::Value) -> String {
    let object_id = object["id"]
        .as_str()
        .unwrap_or_else(|| panic!("rich-text object id is reported: {object}"));
    let (dialogue_view_id, suffix) = object_id
        .split_once(".proxy.")
        .unwrap_or_else(|| panic!("rich-text proxy object id should include .proxy.: {object_id}"));
    let run_index = suffix.split('.').next().unwrap_or_else(|| {
        panic!("rich-text proxy object id should include run index: {object_id}")
    });
    format!("{dialogue_view_id}.run.{run_index}")
}

fn presentation_tree_node<'a>(
    report: &'a serde_json::Value,
    node_id: &str,
) -> &'a serde_json::Value {
    report["presentation_tree"]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("presentation_tree nodes are reported: {report}"))
        .iter()
        .find(|node| node["id"] == node_id)
        .unwrap_or_else(|| panic!("presentation_tree should contain node {node_id}: {report}"))
}

fn assert_rich_text_proxy_presentation_tree_node(
    report: &serde_json::Value,
    proxy_object_id: &str,
    parent_id: &str,
    proxy_id: &str,
    depth: i64,
) {
    let parent_node = presentation_tree_node(report, parent_id);
    assert!(
        parent_node["children"]
            .as_array()
            .expect("parent tree node should expose children")
            .iter()
            .any(|child| child == proxy_object_id),
        "presentation tree parent should list proxy child {proxy_object_id}: {parent_node}"
    );
    let proxy_node = presentation_tree_node(report, proxy_object_id);
    assert_eq!(proxy_node["kind"], "object");
    assert_eq!(proxy_node["parent_id"], parent_id);
    assert_eq!(proxy_node["object_id"], proxy_object_id);
    assert_eq!(proxy_node["role"], "rich_text_proxy");
    assert_eq!(proxy_node["rich_text_kind"], "text_object_proxy");
    assert_eq!(proxy_node["object_depth"], depth);
    assert_eq!(proxy_node["object_proxy_ids"][0], proxy_id);
}

fn rich_text_cluster_column_count(report: &serde_json::Value) -> usize {
    let mut columns = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .map(|object| agent_json_bbox_x(&object["bbox"]))
        .collect::<Vec<_>>();
    columns.sort_unstable();
    columns.dedup();
    columns.len()
}

fn assert_rich_text_object_has_mask_capture(object: &serde_json::Value, context: &str) {
    assert!(
        object["capture_refs"]["captures"]
            .as_array()
            .expect("rich-text object captures are reported")
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "{context} should expose native mask capture refs: {object}"
    );
}

fn agent_json_bboxes_intersect(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    agent_json_bbox_x(left) < agent_json_bbox_right(right)
        && agent_json_bbox_x(right) < agent_json_bbox_right(left)
        && agent_json_bbox_y(left) < agent_json_bbox_bottom(right)
        && agent_json_bbox_y(right) < agent_json_bbox_bottom(left)
}

fn agent_json_bbox_x(bbox: &serde_json::Value) -> u64 {
    bbox["x"].as_u64().expect("bbox x is reported")
}

fn agent_json_bbox_y(bbox: &serde_json::Value) -> u64 {
    bbox["y"].as_u64().expect("bbox y is reported")
}

fn agent_json_bbox_height(bbox: &serde_json::Value) -> u64 {
    bbox["height"].as_u64().expect("bbox height is reported")
}

fn agent_json_bbox_width(bbox: &serde_json::Value) -> u64 {
    bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_center_x_twice(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_x(bbox)
        .saturating_mul(2)
        .saturating_add(agent_json_bbox_width(bbox))
}

fn agent_json_bbox_right(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_x(bbox) + bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_bottom(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_y(bbox) + bbox["height"].as_u64().expect("bbox height is reported")
}

fn assert_vertical_cluster_after(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    context: &str,
) {
    assert_eq!(
        previous["bbox"]["x"], next["bbox"]["x"],
        "{context}: clusters should share the same vertical column"
    );
    let previous_y = previous["bbox"]["y"]
        .as_i64()
        .expect("previous cluster y is numeric");
    let next_y = next["bbox"]["y"]
        .as_i64()
        .expect("next cluster y is numeric");
    assert!(
        next_y > previous_y,
        "{context}: next cluster should advance downward within the column"
    );
}

fn rich_text_object_capture_uri<'a>(
    object: &'a serde_json::Value,
    kind: &str,
    mime_type: &str,
) -> &'a str {
    object["capture_refs"]["captures"]
        .as_array()
        .expect("rich-text object has capture refs")
        .iter()
        .find(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .and_then(|capture| capture["uri"].as_str())
        .unwrap_or_else(|| {
            panic!("rich-text object should have {kind}/{mime_type} capture URI: {object}")
        })
}

fn assert_native_capture_has_content(report: &serde_json::Value, written_name: &str) {
    assert_eq!(report["images"][0]["kind"], "color");
    assert_eq!(report["images"][0]["renderer"], "native");
    assert_eq!(report["images"][0]["composition"], "framebuffer");
    assert_eq!(report["images"][0]["mime_type"], "image/png");
    assert_eq!(report["images"][0]["width"], 1280);
    assert_eq!(report["images"][0]["height"], 720);
    assert!(report["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(report["images"][0]["written"], written_name);
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    (bytes.len() >= 24 && &bytes[..8] == PNG_SIGNATURE && &bytes[12..16] == b"IHDR").then(|| {
        let width = u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width bytes"));
        let height = u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height bytes"));
        (width, height)
    })
}

fn metric_score(report: &serde_json::Value, metric_name: &str) -> f64 {
    metric_entry(report, metric_name)["score"]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name} score should be numeric: {report}"))
}

fn metric_detail(report: &serde_json::Value, metric_name: &str, detail_name: &str) -> f64 {
    metric_entry(report, metric_name)["details"][detail_name]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name}.{detail_name} should be numeric: {report}"))
}

fn metric_entry<'a>(report: &'a serde_json::Value, metric_name: &str) -> &'a serde_json::Value {
    report["metrics"]
        .as_array()
        .and_then(|metrics| {
            metrics
                .iter()
                .find(|metric| metric["name"].as_str() == Some(metric_name))
        })
        .unwrap_or_else(|| panic!("{metric_name} metric should be present: {report}"))
}

fn assert_metric_close(actual: f64, expected: f64, epsilon: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "{label} should be {expected}, got {actual}"
    );
}
