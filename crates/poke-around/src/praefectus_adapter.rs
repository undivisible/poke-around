use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::Signer;
use praefectus::{
    AckState, Action, ActionAck, ActionRequest, AuthorityGrant, CancellationToken, Capabilities,
    Ed25519AuthorityVerifier, Engine, Executor, MouseButton, NativeExecutor, PROTOCOL_VERSION,
    SafetyClass, SignedAuthority, TargetRef, Terminal, VerificationPolicy,
    canonical_authority_bytes, normalized_action_hash,
};
use serde_json::{Value, json, to_value};
use sha2::{Digest, Sha256};

use crate::mcp::{AppState, error_result, int_arg, ok_json, str_arg};
use crate::{Error, Result};

const TARGET_EFFECT_UNAVAILABLE: &str =
    "targeted computer-use effect unavailable: live target identity fencing is not supported";
const AUTOMATION_UNAVAILABLE: &str =
    "automation scripts unavailable: command-level target fencing is not supported";

pub(crate) fn execute_tool(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    rpc_request_id: &str,
    state: &AppState,
    cancellation: &CancellationToken,
) -> Result<Option<Value>> {
    if crate::mcp_tools::is_unavailable_tool(tool_name) {
        return Ok(Some(error_result(
            "tool unavailable: hardened authority, privacy, or cancellation guarantees are not supported",
        )));
    }
    if tool_name == "run" {
        return Ok(Some(error_result(AUTOMATION_UNAVAILABLE)));
    }
    if matches!(
        tool_name,
        "click"
            | "press"
            | "type"
            | "paste"
            | "hotkey"
            | "scroll"
            | "move"
            | "set_value"
            | "perform_action"
            | "window"
            | "app"
            | "menu"
    ) {
        return Ok(Some(error_result(TARGET_EFFECT_UNAVAILABLE)));
    }
    let (executor, capabilities) = match backend_for_candidate(tool_name, args, || {
        let executor = NativeExecutor::default();
        let capabilities = executor.capabilities()?;
        Ok((executor, capabilities))
    }) {
        CandidateBackend::NotCandidate => return Ok(None),
        CandidateBackend::Unavailable => {
            return Ok(Some(error_result(TARGET_EFFECT_UNAVAILABLE)));
        }
        CandidateBackend::Available(executor, capabilities) => (executor, capabilities),
    };
    if !should_use_praefectus(tool_name, args, &capabilities) {
        return Ok(Some(error_result(TARGET_EFFECT_UNAVAILABLE)));
    }
    let action = match tool_name {
        "click" if args.get("x").is_some() && args.get("y").is_some() => Action::Click {
            button: match str_arg(args, "button").unwrap_or("left") {
                "left" => MouseButton::Left,
                "right" => MouseButton::Right,
                _ => return Err(Error::msg("invalid click button")),
            },
            count: normalized_click_count(args) as u32,
            allow_coordinate_fallback: false,
        },
        "move" if args.get("x").is_some() && args.get("y").is_some() => Action::Move,
        _ => return Ok(None),
    };
    let x = int_arg(args, "x").ok_or_else(|| Error::msg("missing x coordinate"))?;
    let y = int_arg(args, "y").ok_or_else(|| Error::msg("missing y coordinate"))?;
    let observation = executor.observe_coordinates()?;
    let display = observation
        .displays
        .iter()
        .find(|display| {
            display.width > 0
                && display.height > 0
                && x >= display.x
                && y >= display.y
                && x < display.x.saturating_add(display.width)
                && y < display.y.saturating_add(display.height)
        })
        .ok_or_else(|| Error::msg("coordinate is outside every display"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::msg(error.to_string()))?
        .as_millis() as i64;
    let operation_id = operation_id(session_id, rpc_request_id);
    let protocol_session_id = format!("poke-session-{}", hash_parts(&[session_id.as_bytes()]));
    let issuer = "poke-around".to_string();
    let key_id = "process-key".to_string();
    let policy_generation = format!(
        "poke-policy-{}",
        state
            .inner
            .approval_generation
            .load(std::sync::atomic::Ordering::Acquire)
    );
    let subject = "poke-local-host".to_string();
    let (safety, verification) = match &action {
        Action::Move => (SafetyClass::Reversible, VerificationPolicy::None),
        _ => (SafetyClass::External, VerificationPolicy::SnapshotChanged),
    };
    let mut request = ActionRequest {
        protocol_version: PROTOCOL_VERSION,
        action_version: PROTOCOL_VERSION,
        target_version: PROTOCOL_VERSION,
        verification_version: PROTOCOL_VERSION,
        operation_id: operation_id.clone(),
        subject: subject.clone(),
        session_id: protocol_session_id.clone(),
        authority: SignedAuthority {
            grant: AuthorityGrant {
                protocol_version: PROTOCOL_VERSION,
                issuer: issuer.clone(),
                key_id: key_id.clone(),
                operation_id,
                subject,
                session_id: protocol_session_id,
                risk: safety,
                expires_at_ms: now + 30_000,
                policy_generation: policy_generation.clone(),
                action_hash: String::new(),
            },
            signature: String::new(),
        },
        action,
        target: TargetRef::Coordinates {
            x,
            y,
            display_id: display.display_id.clone(),
            display_geometry_hash: observation.display_geometry_hash,
            snapshot_id: observation.snapshot_id,
            snapshot_content_hash: observation.snapshot_content_hash,
        },
        deadline_at_ms: now + 30_000,
        verification,
        safety,
    };
    request.authority.grant.action_hash = normalized_action_hash(&request)?;
    let signature = state
        .inner
        .praefectus_signing_key
        .sign(&canonical_authority_bytes(&request.authority.grant)?);
    request.authority.signature = signature
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let verifier = Ed25519AuthorityVerifier::new([(
        issuer,
        key_id,
        policy_generation,
        state.inner.praefectus_signing_key.verifying_key(),
    )]);
    let report = Engine::new(
        executor,
        state.inner.home.join("praefectus-operations.jsonl"),
        verifier,
    )
    .execute(&request, cancellation)?;
    let retry_safe = acknowledgements_are_retry_safe(&report.acknowledgements);
    let succeeded = acknowledgements_succeeded(&report.acknowledgements);
    let mut response = ok_json(json!({
        "report": to_value(report)?,
        "retry_safe": retry_safe,
    }));
    if !succeeded {
        response["isError"] = Value::Bool(true);
    }
    Ok(Some(response))
}

fn hash_parts(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn operation_id(session_id: &str, rpc_request_id: &str) -> String {
    format!(
        "poke-{}",
        hash_parts(&[session_id.as_bytes(), rpc_request_id.as_bytes()])
    )
}

fn acknowledgements_are_retry_safe(acknowledgements: &[ActionAck]) -> bool {
    matches!(
        acknowledgements.last().map(|acknowledgement| &acknowledgement.state),
        Some(AckState::Terminal { terminal })
            if matches!(
                &**terminal,
                Terminal::Rejected { .. }
                    | Terminal::CancelledBeforeEffect
                    | Terminal::ExpiredBeforeEffect
            )
    )
}

fn acknowledgements_succeeded(acknowledgements: &[ActionAck]) -> bool {
    matches!(
        acknowledgements
            .last()
            .map(|acknowledgement| &acknowledgement.state),
        Some(AckState::Terminal { terminal }) if matches!(&**terminal, Terminal::Succeeded { .. })
    )
}

fn should_use_praefectus(tool_name: &str, args: &Value, capabilities: &Capabilities) -> bool {
    let coordinate_capture = capabilities
        .permissions
        .get("coordinate_capture")
        .copied()
        .unwrap_or(false);
    if !coordinate_capture {
        return false;
    }
    let Some(action) = candidate_action(tool_name, args) else {
        return false;
    };
    capabilities
        .supported_actions
        .iter()
        .any(|supported| supported == action)
}

fn candidate_action(tool_name: &str, args: &Value) -> Option<&'static str> {
    if args.get("x").is_none()
        || args.get("y").is_none()
        || ["index", "element_id", "on"]
            .iter()
            .any(|field| args.get(*field).is_some())
    {
        return None;
    }
    match tool_name {
        "move" => "move",
        "click"
            if args.get("background").and_then(Value::as_bool) == Some(false)
                && matches!(str_arg(args, "button").unwrap_or("left"), "left" | "right")
                && (1..=3).contains(&normalized_click_count(args)) =>
        {
            "click"
        }
        _ => return None,
    }
    .into()
}

fn coordinate_candidate(tool_name: &str, args: &Value) -> bool {
    matches!(tool_name, "click" | "move")
        && args.get("x").is_some()
        && args.get("y").is_some()
        && !["index", "element_id", "on"]
            .iter()
            .any(|field| args.get(*field).is_some())
}

enum CandidateBackend<T> {
    NotCandidate,
    Unavailable,
    Available(T, Capabilities),
}

fn backend_for_candidate<T>(
    tool_name: &str,
    args: &Value,
    load: impl FnOnce() -> Result<(T, Capabilities)>,
) -> CandidateBackend<T> {
    if !coordinate_candidate(tool_name, args) {
        return CandidateBackend::NotCandidate;
    }
    if candidate_action(tool_name, args).is_none() {
        return CandidateBackend::Unavailable;
    }
    match load() {
        Ok((executor, capabilities)) => CandidateBackend::Available(executor, capabilities),
        Err(_) => CandidateBackend::Unavailable,
    }
}

fn normalized_click_count(args: &Value) -> i64 {
    int_arg(args, "count").unwrap_or(1).max(1)
}

#[cfg(test)]
fn capabilities_with_actions(coordinate_capture: bool, supported_actions: &[&str]) -> Capabilities {
    Capabilities {
        platform: "test".to_string(),
        backend: "test".to_string(),
        supported_actions: supported_actions
            .iter()
            .map(|action| (*action).to_string())
            .collect(),
        permissions: [("coordinate_capture".to_string(), coordinate_capture)]
            .into_iter()
            .collect(),
        display_geometry_hash: "display".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use praefectus::{Effect, FailureCode, Receipt};
    use std::cell::Cell;

    fn capabilities(coordinate_capture: bool) -> Capabilities {
        capabilities_with_actions(coordinate_capture, &["click", "move"])
    }

    fn terminal_ack(terminal: Terminal) -> ActionAck {
        ActionAck {
            protocol_version: PROTOCOL_VERSION,
            operation_id: "operation".to_string(),
            sequence: 2,
            action_hash: "action".to_string(),
            replayed: false,
            state: AckState::Terminal {
                terminal: Box::new(terminal),
            },
        }
    }

    #[test]
    fn routing_requires_capture_and_exact_foreground_semantics() {
        let available = capabilities(true);
        let unavailable = capabilities(false);

        assert!(should_use_praefectus(
            "click",
            &json!({ "x": 10, "y": 20, "background": false }),
            &available,
        ));
        assert!(should_use_praefectus(
            "move",
            &json!({ "x": 10, "y": 20 }),
            &available,
        ));
        assert!(!should_use_praefectus(
            "click",
            &json!({ "x": 10, "y": 20 }),
            &available,
        ));
        assert!(!should_use_praefectus(
            "click",
            &json!({ "x": 10, "y": 20, "background": false }),
            &unavailable,
        ));
        assert!(!should_use_praefectus(
            "click",
            &json!({ "x": 10, "y": 20, "background": false }),
            &capabilities_with_actions(true, &["move"]),
        ));
        assert!(!should_use_praefectus(
            "move",
            &json!({ "x": 10, "y": 20 }),
            &capabilities_with_actions(true, &["click"]),
        ));
    }

    #[test]
    fn unrelated_tools_should_not_probe_native_capabilities() {
        let probed = Cell::new(false);
        let backend =
            backend_for_candidate("read_file", &json!({ "path": "/tmp/example" }), || {
                probed.set(true);
                Ok(((), capabilities(true)))
            });

        assert!(matches!(backend, CandidateBackend::NotCandidate));
        assert!(!probed.get());
    }

    #[test]
    fn semantic_targets_should_not_route_as_coordinates() {
        let available = capabilities(true);

        for target in [
            json!({ "index": 1 }),
            json!({ "element_id": "button" }),
            json!({ "on": "Submit" }),
        ] {
            let mut args = json!({ "x": 10, "y": 20, "background": false });
            args.as_object_mut()
                .unwrap()
                .extend(target.as_object().unwrap().clone());
            assert!(!should_use_praefectus("click", &args, &available));
        }
    }

    #[test]
    fn coordinate_candidates_fail_closed_when_the_backend_is_unavailable() {
        let backend = backend_for_candidate(
            "click",
            &json!({ "x": 10, "y": 20, "background": false }),
            || Err::<((), Capabilities), _>(Error::msg("capability probe failed")),
        );

        assert!(matches!(backend, CandidateBackend::Unavailable));
        assert!(matches!(
            backend_for_candidate(
                "click",
                &json!({ "x": 10, "y": 20 }),
                || -> Result<((), Capabilities,)> { panic!() }
            ),
            CandidateBackend::Unavailable
        ));
    }

    #[test]
    fn coordinate_dispatch_returns_a_stable_error_without_fallback() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let result = execute_tool(
            "move",
            &json!({ "x": 10, "y": 20 }),
            "session",
            "request",
            &state,
            &CancellationToken::default(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(result["isError"].as_bool(), Some(true));
        assert_eq!(
            result["content"][0]["text"],
            "tool unavailable: hardened authority, privacy, or cancellation guarantees are not supported"
        );
    }

    #[test]
    fn automation_dispatch_returns_a_stable_error_without_fallback() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let result = execute_tool(
            "run",
            &json!({ "file": "automation.json" }),
            "session",
            "request",
            &state,
            &CancellationToken::default(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(result["isError"].as_bool(), Some(true));
        assert_eq!(
            result["content"][0]["text"],
            "tool unavailable: hardened authority, privacy, or cancellation guarantees are not supported"
        );
    }

    #[test]
    fn routing_preserves_click_count_and_button_semantics() {
        let available = capabilities(true);

        for count in [-4, 0, 1, 2, 3] {
            assert!(should_use_praefectus(
                "click",
                &json!({ "x": 10, "y": 20, "background": false, "count": count }),
                &available,
            ));
        }
        for count in [4, 10] {
            assert!(!should_use_praefectus(
                "click",
                &json!({ "x": 10, "y": 20, "background": false, "count": count }),
                &available,
            ));
        }
        assert!(!should_use_praefectus(
            "click",
            &json!({
                "x": 10,
                "y": 20,
                "background": false,
                "button": "middle"
            }),
            &available,
        ));
    }

    #[test]
    fn acknowledgements_are_retry_safe_should_reject_outcome_unknown() {
        let acknowledgements = [terminal_ack(Terminal::OutcomeUnknown {
            receipt: Receipt {
                protocol_version: PROTOCOL_VERSION,
                action_name: "click".to_string(),
                action_hash: "action".to_string(),
                started_at_ms: 1,
                finished_at_ms: 2,
                backend: "test".to_string(),
                fallback_chain: Vec::new(),
                effect: Effect::Unknown,
                before: None,
                after: None,
                warnings: Vec::new(),
            },
            message: "effect outcome is unknown".to_string(),
        })];

        assert!(!acknowledgements_are_retry_safe(&acknowledgements));
        assert!(!acknowledgements_succeeded(&acknowledgements));
    }

    #[test]
    fn operation_identity_is_stable_and_session_bound() {
        assert_eq!(operation_id("session", "1"), operation_id("session", "1"));
        assert_ne!(operation_id("session", "1"), operation_id("session", "2"));
        assert_ne!(operation_id("session", "1"), operation_id("other", "1"));
    }

    #[test]
    fn retry_safety_requires_a_proven_no_effect_terminal() {
        let rejected = [terminal_ack(Terminal::Rejected {
            code: FailureCode::PermissionDenied,
            message: "denied".to_string(),
        })];
        let succeeded = [terminal_ack(Terminal::Succeeded {
            receipt: Receipt {
                protocol_version: PROTOCOL_VERSION,
                action_name: "move".to_string(),
                action_hash: "action".to_string(),
                started_at_ms: 1,
                finished_at_ms: 2,
                backend: "test".to_string(),
                fallback_chain: Vec::new(),
                effect: Effect::ExecutedUnverified,
                before: None,
                after: None,
                warnings: Vec::new(),
            },
        })];

        assert!(acknowledgements_are_retry_safe(&rejected));
        assert!(!acknowledgements_are_retry_safe(&succeeded));
    }
}
