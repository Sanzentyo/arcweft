/// Actual current-frame report and PNG retained by the local CLI blob writer.
struct NativeStoredViewportCapture {
    report: serde_json::Value,
    image: arcweft_agent_protocol::image::AgentImageMetadata,
    bytes: Vec<u8>,
}

impl NativeCliFixture {
    fn capture_progressed_viewport(
        &self,
        dir: &Path,
        advances: u32,
    ) -> NativeStoredViewportCapture {
        let controller = dir.join("progressed-viewport.awfagent");
        fs::write(
            &controller,
            format!(
                r#"
fn progressed_viewport() -> Result<Unit, AgentError>
effects {{ agent.observe, agent.act.semantic, agent.capture, agent.resource.read }}
{{
    observe()
    for index in 0..{advances} {{ advance_text() }}
    read_resource("arcweft://session/cli/observation/latest.json")
    let shot = try capture(viewport())
    read_resource(shot.uri)
    return Ok(())
}}
entry agent @entry.agent.main {{ controller = progressed_viewport }}
"#
            ),
        )
        .unwrap();
        let blob_dir = dir.join("blobs");
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .args(["agent", "script", "run"])
            .arg(&controller)
            .arg("--native-source")
            .arg(&self.path)
            .arg("--entry")
            .arg(self.entry.public_label().as_str())
            .args([
                "--native-mode",
                "drain",
                "--native-steps",
                "4",
                "--native-max-ops",
                "64",
                "--json",
                "--blob-dir",
            ])
            .arg(&blob_dir)
            .output()
            .expect("public CLI captures the progressed native viewport");
        assert!(
            output.status.success(),
            "native viewport script failed: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        let run: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(run["ok"], true);
        assert_eq!(run["blobs_written"], 1);
        let responses = run["responses"].as_array().unwrap();
        let observations = responses
            .iter()
            .filter(|response| response["kind"] == "observation")
            .collect::<Vec<_>>();
        assert_eq!(observations.len(), 1);
        let actions = responses
            .iter()
            .filter(|response| response["kind"] == "action")
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), usize::try_from(advances).unwrap());
        for action in actions {
            assert_eq!(action["response"]["accepted"], true);
            assert_eq!(
                action["response"]["after_tick"].as_u64().unwrap(),
                action["response"]["before_tick"].as_u64().unwrap() + 4
            );
        }
        let resources = responses
            .iter()
            .filter(|response| response["kind"] == "resource")
            .map(|response| {
                serde_json::from_value::<arcweft_agent_protocol::resource::AgentResource>(
                    response["response"].clone(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(resources.len(), 2);
        let report = match &resources[0].body {
            arcweft_agent_protocol::resource::AgentResourceBody::Json(report) => report.clone(),
            other => panic!("actual final observation JSON: {other:?}"),
        };
        let typed = arcweft_agent_protocol::observation::AgentObservationReport::decode_json(
            &serde_json::to_vec(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(typed.tick, 3 + usize::try_from(advances).unwrap() * 4);
        assert_eq!(typed.steps, typed.tick + 1);
        assert!(typed.diagnostics.is_empty());
        let captures = responses
            .iter()
            .filter(|response| response["kind"] == "capture")
            .collect::<Vec<_>>();
        assert_eq!(captures.len(), 1);
        let capture = &captures[0]["response"];
        let resource = &resources[1];
        assert_eq!(resource.uri.as_str(), capture["uri"].as_str().unwrap());
        assert_eq!(resource.mime_type, "image/png");
        assert_eq!(capture["media_type"], "image/png");
        let image = resource.image.clone().unwrap();
        arcweft_agent_protocol::image::AgentImageMetadata::decode_json(
            &serde_json::to_vec(&image).unwrap(),
        )
        .unwrap();
        let arcweft_agent_protocol::resource::AgentResourceBody::BytesBase64(body) = &resource.body
        else {
            panic!("actual image bytes")
        };
        let expected = general_purpose::STANDARD.decode(&body.data).unwrap();
        let mut pending = vec![blob_dir];
        let mut files = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    pending.push(entry.path());
                } else if entry.file_type().unwrap().is_file() {
                    files.push(entry.path());
                }
            }
        }
        assert_eq!(files.len(), 1, "one actual CLI-written capture blob");
        let bytes = fs::read(&files[0]).unwrap();
        assert_eq!(
            bytes, expected,
            "CLI blob bytes equal the same retained-frame URI read"
        );
        assert_eq!(capture["byte_len"].as_u64().unwrap(), bytes.len() as u64);
        assert_eq!(run["blob_bytes"].as_u64().unwrap(), bytes.len() as u64);
        assert_eq!(
            capture["content_hash"],
            format!("blake3:{}", blake3::hash(&bytes).to_hex())
        );
        assert_eq!(image.capture_step, typed.steps);
        NativeStoredViewportCapture {
            report,
            image,
            bytes,
        }
    }
}
