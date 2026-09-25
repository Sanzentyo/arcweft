use super::{FinalSemanticAnalysisError, TypeCheckEnv, analyze, fixture_with_base_environment};

const SOURCE: &str = r"
fn read(value: i64) -> i64 effects { fs.read(save) } { value }
flow main() -> i64 { return read(1i64) }
";

#[test]
fn target_effects_use_selected_rows_and_scoped_coverage() {
    let unrestricted = fixture_with_base_environment(SOURCE, None, TypeCheckEnv::standard());
    assert!(analyze(&unrestricted).is_ok());
    let permitted = fixture_with_base_environment(
        SOURCE,
        None,
        TypeCheckEnv::standard().with_available_effects(["fs.read"]),
    );
    assert!(analyze(&permitted).is_ok());
    for allowed in [Vec::<&str>::new(), vec!["fs.read(asset)"]] {
        let rejected = fixture_with_base_environment(
            SOURCE,
            None,
            TypeCheckEnv::standard().with_available_effects(allowed),
        );
        let error = analyze(&rejected).expect_err("selected target cannot provide this read");
        assert!(matches!(
            error,
            FinalSemanticAnalysisError::TargetCapabilityUnavailable { .. }
        ));
        assert_eq!(error.diagnostic_code(), "AWF-EFX-007");
        assert!(error.source_diagnostic().is_some());
    }
}

#[test]
fn selected_target_accepts_engine_line_effects_without_adapter_effects() {
    let source = concat!(
        "pub character akane {}\n",
        "flow line_handles() -> String {\n",
        "    let (_, cue) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        let cue = at(0.42s):\n",
        "            actor.look(.normal)\n",
        "        let voice = line.voice_handle()\n",
        "        defer on cancelled:\n",
        "            log.info(\"cancelled\")\n",
        "        out (voice, cue)\n",
        "    return \"done\"\n",
        "}\n",
    );
    let (document, catalog, _) = super::akane_character_registration();
    let fixture = super::fixture_with_all_registration_inputs_and_base(
        source,
        None,
        Vec::new(),
        vec![(document, catalog)],
        Vec::new(),
        TypeCheckEnv::standard().with_available_effects(Vec::<&str>::new()),
    );
    analyze(&fixture).expect("the engine provides line schedule, voice, and log effects");
}

#[test]
fn target_effects_do_not_execute_a_nonterminal_prefix() {
    let source = r"
fn staged(first: i64)(second: i64) -> i64 effects { fs.read } { first + second }
flow main() -> i64 { let prefix = staged(1i64); return 0i64 }
";
    let fixture = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_effects(Vec::<&str>::new()),
    );
    analyze(&fixture).expect("forming a prefix keeps its terminal effects latent");
}

#[test]
fn target_effects_are_part_of_the_registered_environment_identity() {
    let source = "flow main() -> i64 { return 1i64 }";
    let unrestricted = fixture_with_base_environment(source, None, TypeCheckEnv::standard());
    let empty = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_effects(Vec::<&str>::new()),
    );
    let first = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_effects(["fs.read", "fs.write"]),
    );
    let reordered = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_effects(["fs.write", "fs.read"]),
    );
    let digest = |fixture: &super::Fixture| fixture.registered.environment().environment_digest();
    assert_ne!(digest(&unrestricted), digest(&empty));
    assert_ne!(digest(&empty), digest(&first));
    assert_eq!(digest(&first), digest(&reordered));
    let first_contract = arcweft_manifest_model::HostCallContractDigest::from_bytes([1; 32]);
    let second_contract = arcweft_manifest_model::HostCallContractDigest::from_bytes([2; 32]);
    let selected = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_host_calls([first_contract, second_contract]),
    );
    let selected_reordered = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_host_calls([second_contract, first_contract]),
    );
    let no_calls = fixture_with_base_environment(
        source,
        None,
        TypeCheckEnv::standard().with_available_host_calls([]),
    );
    assert_eq!(digest(&selected), digest(&selected_reordered));
    assert_ne!(digest(&selected), digest(&no_calls));
    assert_ne!(digest(&no_calls), digest(&unrestricted));
}
