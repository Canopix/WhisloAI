use std::process::Command;

mod app;
mod commands;
mod domain;
mod overlay;
mod platform;

use domain::anchor::*;
use domain::ai::*;
use domain::config::*;
#[cfg(test)]
use domain::geometry::*;
use domain::providers::*;
use overlay::*;

#[cfg(target_os = "macos")]
mod macos_ax;

#[cfg(target_os = "macos")]
fn is_running_under_rosetta() -> bool {
    *overlay::refocus::RUNNING_UNDER_ROSETTA.get_or_init(|| {
        let Ok(output) = Command::new("sysctl")
            .args(["-in", "sysctl.proc_translated"])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        String::from_utf8_lossy(&output.stdout).trim() == "1"
    })
}

#[cfg(not(target_os = "macos"))]
fn is_running_under_rosetta() -> bool {
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}

#[cfg(test)]
mod tests {
    use super::{
        audio_file_name, download_progress_percent, external_target_restore_reason,
        local_prefers_openai_chat_endpoint, local_transcription_block_reason, logical_to_physical,
        non_empty_trimmed, normalize_anchor_behavior, normalize_local_transcription_output,
        normalize_provider_base_url, parse_anchor_snapshot_probe_output, point_in_rect,
        provider_endpoint, sanitize_scale_factor, scale_for_logical_point_in_rects,
        should_clear_external_cache_on_restore_error,
        should_clear_external_cache_on_restore_reason, should_hide_contextual_anchor,
        transcribe_error, update_hybrid_fallback_state, AnchorSnapshotRawParse, ExternalAppTarget,
        HybridFallbackState, INPUT_TARGET_TTL_MS,
    };

    #[test]
    fn point_in_rect_is_inclusive_of_min_and_exclusive_of_max() {
        assert!(point_in_rect(10, 10, 10, 10, 20, 20));
        assert!(point_in_rect(29, 29, 10, 10, 20, 20));
        assert!(!point_in_rect(30, 30, 10, 10, 20, 20));
        assert!(!point_in_rect(9, 10, 10, 10, 20, 20));
    }

    #[test]
    fn normalize_anchor_behavior_maps_unknown_to_contextual() {
        assert_eq!(normalize_anchor_behavior("floating"), "floating");
        assert_eq!(normalize_anchor_behavior("FLOATING"), "floating");
        assert_eq!(normalize_anchor_behavior("contextual"), "contextual");
        assert_eq!(normalize_anchor_behavior("anything-else"), "contextual");
    }

    #[test]
    fn sanitize_scale_factor_falls_back_to_one_for_invalid_values() {
        assert_eq!(sanitize_scale_factor(0.0), 1.0);
        assert_eq!(sanitize_scale_factor(-1.5), 1.0);
        assert_eq!(sanitize_scale_factor(f64::NAN), 1.0);
        assert_eq!(sanitize_scale_factor(1.5), 1.5);
    }

    #[test]
    fn logical_to_physical_rounds_to_nearest_pixel() {
        assert_eq!(logical_to_physical(11, 1.5), 17);
        assert_eq!(logical_to_physical(10, 2.0), 20);
        assert_eq!(logical_to_physical(-3, 2.0), -6);
    }

    #[test]
    fn scale_for_logical_point_selects_candidate_that_contains_converted_point() {
        let candidates = vec![(0, 0, 1000, 800, 2.0), (1000, 0, 1200, 900, 1.0)];
        assert_eq!(
            scale_for_logical_point_in_rects(1500, 200, &candidates),
            Some(1.0)
        );
        assert_eq!(
            scale_for_logical_point_in_rects(300, 100, &candidates),
            Some(2.0)
        );
        assert_eq!(
            scale_for_logical_point_in_rects(2600, 100, &candidates),
            None
        );
    }

    #[test]
    fn download_progress_percent_handles_known_totals() {
        assert_eq!(download_progress_percent(0, Some(100)), Some(0));
        assert_eq!(download_progress_percent(55, Some(100)), Some(55));
        assert_eq!(download_progress_percent(101, Some(100)), Some(100));
    }

    #[test]
    fn download_progress_percent_returns_none_for_unknown_or_invalid_total() {
        assert_eq!(download_progress_percent(50, None), None);
        assert_eq!(download_progress_percent(50, Some(0)), None);
    }

    #[test]
    fn non_empty_trimmed_returns_none_for_blank_values() {
        assert_eq!(non_empty_trimmed(""), None);
        assert_eq!(non_empty_trimmed("   "), None);
        assert_eq!(non_empty_trimmed("  hola "), Some("hola"));
    }

    #[test]
    fn normalize_provider_base_url_strips_terminal_endpoint_paths() {
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/chat"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/chat/completions"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/models"),
            "http://localhost:1234/api/v1"
        );
        assert_eq!(
            normalize_provider_base_url("http://localhost:1234/api/v1/audio/transcriptions"),
            "http://localhost:1234/api/v1"
        );
    }

    #[test]
    fn provider_endpoint_avoids_endpoint_duplication() {
        assert_eq!(
            provider_endpoint("http://localhost:1234/api/v1/chat", "chat"),
            "http://localhost:1234/api/v1/chat"
        );
        assert_eq!(
            provider_endpoint(
                "http://localhost:1234/api/v1/chat/completions",
                "chat/completions"
            ),
            "http://localhost:1234/api/v1/chat/completions"
        );
        assert_eq!(
            provider_endpoint("http://localhost:1234/api/v1", "chat"),
            "http://localhost:1234/api/v1/chat"
        );
    }

    #[test]
    fn local_prefers_openai_chat_endpoint_detects_v1_roots() {
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api/v1"
        ));
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api/v1/chat"
        ));
        assert!(local_prefers_openai_chat_endpoint(
            "http://localhost:1234/v1/chat/completions"
        ));
        assert!(!local_prefers_openai_chat_endpoint("http://localhost:1234"));
        assert!(!local_prefers_openai_chat_endpoint(
            "http://localhost:1234/api"
        ));
    }

    #[test]
    fn local_transcription_guard_does_not_block_rosetta() {
        assert!(local_transcription_block_reason(true, true).is_none());
        assert!(local_transcription_block_reason(true, false).is_none());
        assert!(local_transcription_block_reason(false, true).is_none());
    }

    #[test]
    fn transcribe_error_passthrough_returns_original_message() {
        let result = transcribe_error("boom");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "boom".to_string());
    }

    #[test]
    fn audio_file_name_handles_mime_variants() {
        assert_eq!(
            audio_file_name(Some("audio/webm;codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("audio/webm; codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("Audio/WEBM; codecs=opus")),
            "recording.webm"
        );
        assert_eq!(
            audio_file_name(Some("audio/mp4; codecs=mp4a.40.2")),
            "recording.m4a"
        );
        assert_eq!(
            audio_file_name(Some("audio/wav; charset=binary")),
            "recording.wav"
        );
    }

    #[test]
    fn normalize_local_transcription_output_rejects_empty_text() {
        let result = normalize_local_transcription_output("   ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn contextual_hide_requires_ttl_expiry_and_debounce_elapsed() {
        assert!(!should_hide_contextual_anchor(
            1_400,
            Some(1_000),
            Some(1_000),
            420,
            1_200
        ));
        assert!(!should_hide_contextual_anchor(
            2_050,
            Some(1_900),
            Some(500),
            420,
            1_200
        ));
        assert!(should_hide_contextual_anchor(
            2_500,
            Some(1_900),
            Some(500),
            420,
            1_200
        ));
        assert!(!should_hide_contextual_anchor(
            2_500,
            None,
            Some(500),
            420,
            1_200
        ));
    }

    #[test]
    fn hybrid_fallback_activates_after_threshold_and_respects_cooldown() {
        let mut state = HybridFallbackState::default();

        assert!(!update_hybrid_fallback_state(
            &mut state, true, 100, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 200, 3, 1_000
        ));
        assert!(update_hybrid_fallback_state(
            &mut state, true, 300, 3, 1_000
        ));
        assert_eq!(state.fallback_cooldown_until_ms, 1_300);
        assert_eq!(state.consecutive_native_failures, 0);

        assert!(!update_hybrid_fallback_state(
            &mut state, true, 500, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 600, 3, 1_000
        ));
        assert!(!update_hybrid_fallback_state(
            &mut state, true, 700, 3, 1_000
        ));
        assert!(update_hybrid_fallback_state(
            &mut state, true, 1_300, 3, 1_000
        ));

        assert!(!update_hybrid_fallback_state(
            &mut state, false, 1_301, 3, 1_000
        ));
        assert_eq!(state.consecutive_native_failures, 0);
    }

    #[test]
    fn restore_error_classification_clears_cache_for_not_running_targets() {
        assert!(should_clear_external_cache_on_restore_error("NOT_RUNNING"));
        assert!(should_clear_external_cache_on_restore_error("not running"));
        assert!(!should_clear_external_cache_on_restore_error(
            "Automation denied"
        ));
    }

    #[test]
    fn external_target_validation_detects_missing_invalid_stale_and_ready_targets() {
        let now_ms = 1_000_000_u128;
        assert_eq!(
            external_target_restore_reason(None, now_ms),
            ("missing_target", None)
        );

        let invalid = ExternalAppTarget {
            bundle_id: "com.whisloai.desktop".to_string(),
            captured_at_ms: now_ms,
        };
        assert_eq!(
            external_target_restore_reason(Some(&invalid), now_ms),
            ("invalid_target", None)
        );

        let stale = ExternalAppTarget {
            bundle_id: "com.tinyspeck.slackmacgap".to_string(),
            captured_at_ms: now_ms.saturating_sub(INPUT_TARGET_TTL_MS + 1),
        };
        let (stale_reason, stale_age_ms) = external_target_restore_reason(Some(&stale), now_ms);
        assert_eq!(stale_reason, "stale_target");
        assert!(stale_age_ms.unwrap_or(0) > INPUT_TARGET_TTL_MS);

        let fresh = ExternalAppTarget {
            bundle_id: "com.tinyspeck.slackmacgap".to_string(),
            captured_at_ms: now_ms.saturating_sub(250),
        };
        assert_eq!(
            external_target_restore_reason(Some(&fresh), now_ms),
            ("ready", Some(250))
        );
    }

    #[test]
    fn cleanup_policy_clears_cache_for_invalid_stale_and_not_running_reasons() {
        assert!(should_clear_external_cache_on_restore_reason(
            "invalid_target"
        ));
        assert!(should_clear_external_cache_on_restore_reason(
            "stale_target"
        ));
        assert!(should_clear_external_cache_on_restore_reason("not_running"));
        assert!(!should_clear_external_cache_on_restore_reason(
            "missing_target"
        ));
        assert!(!should_clear_external_cache_on_restore_reason(
            "restore_error"
        ));
    }

    #[test]
    fn parse_anchor_snapshot_probe_output_reads_ok_payload() {
        let parsed =
            parse_anchor_snapshot_probe_output("OK\tcom.tinyspeck.slackmacgap\t100,200,300,48");
        assert_eq!(
            parsed,
            Some(AnchorSnapshotRawParse::Found {
                bundle_id: Some("com.tinyspeck.slackmacgap".to_string()),
                x: 100,
                y: 200,
                w: 300,
                h: 48,
            })
        );
    }

    #[test]
    fn parse_anchor_snapshot_probe_output_reads_skip_reason_and_bundle() {
        let parsed = parse_anchor_snapshot_probe_output(
            "SKIP\tblocked_dom_input_type:search\tcom.google.Chrome",
        );
        assert_eq!(
            parsed,
            Some(AnchorSnapshotRawParse::Skip {
                reason: "blocked_dom_input_type:search".to_string(),
                bundle_id: Some("com.google.Chrome".to_string()),
            })
        );
    }
}
