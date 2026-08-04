use clay::perf::budgets::TYPOGRAPHY_PAYLOAD_BUDGET_BYTES;
use clay::protocol::{
    ActiveTypography, ActiveTypographyValidationError, DecorationKind, DecorationProvenance,
    DecorationSet, DecorationSpan, DocumentFontRole, FontProfile, FontProfileValidationError,
    FontRole, LigaturePolicy, ServerMessage, codec::Codec,
};
use clay::server::decorations::{DecorationValidationError, validate_decoration_set};

fn syntax_span(kind: DecorationKind) -> DecorationSpan {
    DecorationSpan::from_style_token(
        0,
        4,
        kind,
        "markup.inline-code",
        1,
        DecorationProvenance {
            package_name: "@clay/test".to_string(),
            package_version: "0.1.0".to_string(),
            package_prefix: "test".to_string(),
        },
    )
}

#[test]
fn active_typography_round_trips_all_profiles_and_revision() {
    let typography = ActiveTypography {
        revision: 7,
        monospace: FontProfile {
            families: vec!["JetBrains Mono".to_string(), "monospace".to_string()],
            size: 16.0,
            ..FontProfile::default()
        },
        proportional: FontProfile {
            families: vec!["Inter".to_string(), "sans-serif".to_string()],
            size: 17.0,
            ..FontProfile::default()
        },
        ui: FontProfile {
            families: vec!["system-ui".to_string()],
            size: 13.0,
            ..FontProfile::default()
        },
        ..ActiveTypography::default()
    };
    typography.validate().expect("valid bounded typography");

    let message = ServerMessage::ActiveTypography(typography);
    let codec = Codec::default();
    let frame = codec
        .encode_server_message(&message)
        .expect("typography snapshot encodes");

    assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
}

#[test]
fn active_typography_rejects_invalid_sizes_and_family_stacks() {
    let mut typography = ActiveTypography::default();
    typography.monospace.size = f32::NAN;

    assert_eq!(
        typography.validate(),
        Err(ActiveTypographyValidationError::InvalidProfile {
            role: FontRole::Monospace,
            source: FontProfileValidationError::InvalidSize,
        })
    );

    let oversized_family = "x".repeat(129);
    assert_eq!(
        FontProfile {
            families: vec![oversized_family, "monospace".to_string()],
            size: 16.0,
            ..FontProfile::default()
        }
        .validate(),
        Err(FontProfileValidationError::FamilyTooLong)
    );
}

#[test]
fn decoration_font_role_is_limited_to_syntax_and_semantic_layers() {
    let mut diagnostic = syntax_span(DecorationKind::Diagnostic);
    diagnostic.font_role = Some(DocumentFontRole::Monospace);
    let set = DecorationSet {
        document_id: 1,
        document_version: 1,
        package_prefix: "test".to_string(),
        kind: DecorationKind::Diagnostic,
        viewport_byte_start: 0,
        viewport_byte_end: 4,
        spans: vec![diagnostic],
    };

    assert_eq!(
        validate_decoration_set(1, set, None),
        Err(DecorationValidationError::FontRoleOnNonLayoutSpan {
            index: 0,
            kind: DecorationKind::Diagnostic,
        })
    );
}

#[test]
fn first_party_modes_declare_roles_without_rendering_language_branches() {
    for (package, role) in [
        ("markdown", "proportional"),
        ("rust", "monospace"),
        ("typescript", "monospace"),
        ("javascript", "monospace"),
    ] {
        let manifest = std::fs::read_to_string(format!("packages/{package}/package.json")).unwrap();
        assert!(
            manifest.contains(&format!("\"defaultFontRole\":\"{role}\""))
                || manifest.contains(&format!("\"defaultFontRole\": \"{role}\""))
        );
    }

    for path in [
        "src/editor/layout.rs",
        "src/editor/surface.rs",
        "src/masonry_editor.rs",
        "src/masonry_sdui.rs",
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        assert!(!source.contains("mode_id == \"markdown\""), "{path}");
        assert!(!source.contains("mode_id == \"rust\""), "{path}");
        assert!(!source.contains("mode_id == \"typescript\""), "{path}");
        assert!(!source.contains("mode_id == \"javascript\""), "{path}");
    }
}

#[test]
fn typography_payload_limits_reject_oversized_family_data_before_publication() {
    let profile = FontProfile {
        families: vec!["named".to_string(); 9],
        size: 16.0,
        ..FontProfile::default()
    };

    assert_eq!(
        profile.validate(),
        Err(FontProfileValidationError::TooManyFamilies)
    );
    assert!(
        rkyv::to_bytes::<rkyv::rancor::Error>(&ActiveTypography::default())
            .expect("default typography serializes")
            .len()
            <= TYPOGRAPHY_PAYLOAD_BUDGET_BYTES
    );
}

fn profile_with_ligatures(ligatures: LigaturePolicy) -> FontProfile {
    FontProfile {
        families: vec!["monospace".to_string()],
        size: 16.0,
        ligatures: Box::new(ligatures),
    }
}

#[test]
fn ligature_policy_default_enables_standard_and_contextual() {
    let policy = LigaturePolicy::default();
    assert!(policy.enable_standard);
    assert!(policy.enable_contextual);
    assert!(profile_with_ligatures(policy).validate().is_ok());
}

#[test]
fn ligature_policy_rejects_too_many_discretionary_features() {
    let policy = LigaturePolicy {
        discretionary_features: vec!["ss01".to_string(); 33],
        ..LigaturePolicy::default()
    };
    assert_eq!(
        profile_with_ligatures(policy).validate(),
        Err(FontProfileValidationError::TooManyDiscretionaryFeatures)
    );
}

#[test]
fn ligature_policy_rejects_too_many_disabled_features() {
    let policy = LigaturePolicy {
        disable_features: vec!["liga".to_string(); 33],
        ..LigaturePolicy::default()
    };
    assert_eq!(
        profile_with_ligatures(policy).validate(),
        Err(FontProfileValidationError::TooManyDisabledFeatures)
    );
}

#[test]
fn ligature_policy_rejects_oversized_raw_features_source() {
    let policy = LigaturePolicy {
        raw_features: Some("x".repeat(257)),
        ..LigaturePolicy::default()
    };
    assert_eq!(
        profile_with_ligatures(policy).validate(),
        Err(FontProfileValidationError::RawFeaturesTooLong)
    );
}

#[test]
fn ligature_policy_rejects_invalid_feature_name() {
    // Five characters exceeds the four-byte OpenType tag limit.
    let policy = LigaturePolicy {
        discretionary_features: vec!["ss001".to_string()],
        ..LigaturePolicy::default()
    };
    assert_eq!(
        profile_with_ligatures(policy).validate(),
        Err(FontProfileValidationError::InvalidFeatureName)
    );
}

#[test]
fn ligature_policy_accepts_disable_overriding_enable() {
    // A valid policy that disables liga despite the default enable toggle.
    let policy = LigaturePolicy {
        disable_features: vec!["liga".to_string()],
        ..LigaturePolicy::default()
    };
    assert!(profile_with_ligatures(policy).validate().is_ok());
}
