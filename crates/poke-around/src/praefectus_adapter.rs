use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::Signer;
use praefectus::semantic::{SemanticObservation, SemanticTargetRef};
use praefectus::{
    AckState, Action, ActionAck, ActionRequest, AuthorityGrant, CancellationToken, Capabilities,
    DispatchError, DispatchReceipt, Ed25519AuthorityVerifier, Engine, Executor, InteractionMode,
    NativeExecutor, Observation, PROTOCOL_VERSION, ProtocolError, ResolvedTarget, SafetyClass,
    SessionIsolation, SignedAuthority, TargetRef, Terminal, VerificationPolicy,
    canonical_authority_bytes, normalized_action_hash,
};
use serde_json::{Value, json, to_value};
use sha2::{Digest, Sha256};

use crate::mcp::{AppState, error_result, ok_json};
use crate::{Error, Result};

const MAX_BOUND_OBSERVATIONS: usize = 32;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_VALUE_CHARS: usize = 4_096;
const MAX_VALUE_BYTES: usize = 16 * 1_024;

#[derive(Clone)]
pub(crate) struct BoundSemanticObservation {
    session_hash: String,
    observation: SemanticObservation,
}

#[derive(Clone)]
struct SharedNativeExecutor(Arc<NativeExecutor>);

impl Executor for SharedNativeExecutor {
    fn capabilities(&self) -> std::result::Result<Capabilities, ProtocolError> {
        self.0.capabilities()
    }

    fn session_isolation(&self) -> SessionIsolation {
        self.0.session_isolation()
    }

    fn shared_desktop_context_hash(&self) -> std::result::Result<String, ProtocolError> {
        self.0.shared_desktop_context_hash()
    }

    fn shared_desktop_context_hash_with_boundary(
        &self,
        cancellation: &CancellationToken,
        deadline_at_ms: i64,
    ) -> std::result::Result<String, ProtocolError> {
        self.0
            .shared_desktop_context_hash_with_boundary(cancellation, deadline_at_ms)
    }

    fn observe(&self, target: &TargetRef) -> std::result::Result<Observation, ProtocolError> {
        self.0.observe(target)
    }

    fn observe_with_boundary(
        &self,
        target: &TargetRef,
        cancellation: &CancellationToken,
        deadline_at_ms: i64,
    ) -> std::result::Result<Observation, ProtocolError> {
        self.0
            .observe_with_boundary(target, cancellation, deadline_at_ms)
    }

    fn resolve(&self, target: &TargetRef) -> std::result::Result<ResolvedTarget, ProtocolError> {
        self.0.resolve(target)
    }

    fn resolve_with_boundary(
        &self,
        target: &TargetRef,
        cancellation: &CancellationToken,
        deadline_at_ms: i64,
    ) -> std::result::Result<ResolvedTarget, ProtocolError> {
        self.0
            .resolve_with_boundary(target, cancellation, deadline_at_ms)
    }

    fn dispatch(
        &self,
        action: &Action,
        target: &ResolvedTarget,
        verification: &VerificationPolicy,
        cancellation: &CancellationToken,
        deadline_at_ms: i64,
    ) -> std::result::Result<DispatchReceipt, DispatchError> {
        self.0
            .dispatch(action, target, verification, cancellation, deadline_at_ms)
    }
}

pub(crate) fn execute_tool(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    rpc_request_id: &str,
    state: &AppState,
    cancellation: &CancellationToken,
) -> Result<Option<Value>> {
    match tool_name {
        "observe_ui" => execute_observation(args, session_id, state, cancellation).map(Some),
        "click" | "set_value" => execute_effect(
            tool_name,
            args,
            session_id,
            rpc_request_id,
            state,
            cancellation,
        )
        .map(Some),
        _ => Ok(None),
    }
}

fn execute_observation(
    args: &Value,
    session_id: &str,
    state: &AppState,
    cancellation: &CancellationToken,
) -> Result<Value> {
    if !valid_keys(args, &["approval_request_id"]) || !valid_approval_id(args) {
        return Ok(invalid_arguments());
    }
    let deadline_at_ms = now_ms().saturating_add(30_000);
    let observation = match state
        .inner
        .praefectus_executor
        .observe_semantic(cancellation, deadline_at_ms)
    {
        Ok(observation) => observation,
        Err(ProtocolError::ObservationCancelled) => {
            return Ok(terminal_error("CANCELLED_BEFORE_EFFECT", true));
        }
        Err(ProtocolError::ObservationExpired) => {
            return Ok(terminal_error("EXPIRED_BEFORE_EFFECT", true));
        }
        Err(_) => return Ok(error_result("semantic UI observation unavailable")),
    };
    if let Err(error) = bind_observation(state, session_id, observation.clone()) {
        return Ok(error_result(error.to_string()));
    }
    Ok(ok_json(public_observation(&observation)))
}

fn execute_effect(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    rpc_request_id: &str,
    state: &AppState,
    cancellation: &CancellationToken,
) -> Result<Value> {
    let input = match SemanticInput::parse(tool_name, args) {
        Some(input) => input,
        None => return Ok(invalid_arguments()),
    };
    let bound = match bound_target(state, session_id, &input, tool_name) {
        Ok(bound) => bound,
        Err(message) => return Ok(error_result(message)),
    };
    let action = match tool_name {
        "click" => Action::Invoke,
        "set_value" => Action::SetValue {
            value: input.value.clone().unwrap_or_default(),
        },
        _ => return Ok(invalid_arguments()),
    };
    let request = match authorized_request(
        action,
        bound.target,
        input.interaction_mode,
        session_id,
        rpc_request_id,
        state,
        bound.expires_at_ms,
    ) {
        Ok(request) => request,
        Err(_) => return Ok(terminal_error("REJECTED", true)),
    };
    let verifier = match Ed25519AuthorityVerifier::new([(
        request.authority.grant.issuer.clone(),
        request.authority.grant.key_id.clone(),
        request.authority.grant.policy_generation.clone(),
        state.inner.praefectus_signing_key.verifying_key(),
    )]) {
        Ok(verifier) => verifier,
        Err(_) => return Ok(terminal_error("REJECTED", true)),
    };
    let operations_log = match crate::config::ensure_private_config_dir() {
        Ok(directory) => directory.join("praefectus-operations.jsonl"),
        Err(_) => return Ok(terminal_error("REJECTED", true)),
    };
    let report = match Engine::new(
        SharedNativeExecutor(Arc::clone(&state.inner.praefectus_executor)),
        operations_log.clone(),
        verifier,
    )
    .execute(&request, cancellation)
    {
        Ok(report) => report,
        Err(ProtocolError::Conflict) => return Ok(terminal_error("CONFLICT", false)),
        Err(_) => return Ok(terminal_error("OUTCOME_UNKNOWN", false)),
    };
    let _ = crate::config::restrict_private_file(&operations_log);
    let retry_safe = acknowledgements_are_retry_safe(&report.acknowledgements);
    let succeeded = acknowledgements_succeeded(&report.acknowledgements);
    let report = match to_value(report) {
        Ok(report) => report,
        Err(_) => return Ok(terminal_error("OUTCOME_UNKNOWN", false)),
    };
    let mut response = ok_json(json!({
        "report": report,
        "retry_safe": retry_safe,
    }));
    if !succeeded {
        response["isError"] = Value::Bool(true);
    }
    Ok(response)
}

struct BoundTarget {
    target: SemanticTargetRef,
    expires_at_ms: i64,
    role: String,
    name: Option<String>,
}

pub(crate) struct ApprovalSummary {
    pub(crate) host: String,
    pub(crate) caller: String,
}

pub(crate) fn approval_summary(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    state: &AppState,
) -> Option<Result<ApprovalSummary>> {
    if !matches!(tool_name, "click" | "set_value") {
        return None;
    }
    Some((|| {
        let input = SemanticInput::parse(tool_name, args)
            .ok_or_else(|| Error::msg("invalid semantic tool arguments"))?;
        let bound = bound_target(state, session_id, &input, tool_name).map_err(Error::msg)?;
        let interaction_mode = match input.interaction_mode {
            InteractionMode::Interactive => "interactive",
            InteractionMode::BackgroundOnly => "background_only",
            InteractionMode::Unknown => "unknown",
        };
        let observation = &input.observation_id[..12];
        let host_name = public_name(&bound.role, bound.name.as_deref())
            .map(|name| format!(" named '{name}'"))
            .unwrap_or_else(|| " with redacted name".to_string());
        let caller = match tool_name {
            "click" => format!(
                "Click semantic '{}' target {} from observation {} generation {} in {} mode",
                bound.role, input.tag, observation, input.generation, interaction_mode
            ),
            "set_value" => format!(
                "Set semantic '{}' target {} from observation {} generation {} in {} mode to {} chars",
                bound.role,
                input.tag,
                observation,
                input.generation,
                interaction_mode,
                input
                    .value
                    .as_deref()
                    .map_or(0, |value| value.chars().count())
            ),
            other => {
                return Err(Error::msg(format!(
                    "approval summary unavailable for tool '{other}'"
                )));
            }
        };
        let host = format!("{caller}{host_name}");
        Ok(ApprovalSummary { host, caller })
    })())
}

fn bind_observation(
    state: &AppState,
    session_id: &str,
    observation: SemanticObservation,
) -> Result<()> {
    let now = now_ms();
    observation
        .validate(now)
        .map_err(|_| Error::msg("semantic UI observation unavailable"))?;
    let mut observations = state
        .inner
        .semantic_observations
        .lock()
        .map_err(|_| Error::msg("semantic UI observation unavailable"))?;
    observations.retain(|_, bound| bound.observation.expires_at_ms > now);
    if observations.len() >= MAX_BOUND_OBSERVATIONS
        && !observations.contains_key(&observation.observation_id)
    {
        return Err(Error::msg("semantic UI observation capacity reached"));
    }
    observations.insert(
        observation.observation_id.clone(),
        BoundSemanticObservation {
            session_hash: session_hash(session_id),
            observation,
        },
    );
    Ok(())
}

fn bound_target(
    state: &AppState,
    session_id: &str,
    input: &SemanticInput,
    tool_name: &str,
) -> std::result::Result<BoundTarget, &'static str> {
    let now = now_ms();
    let mut observations = state
        .inner
        .semantic_observations
        .lock()
        .map_err(|_| "semantic UI observation unavailable")?;
    observations.retain(|_, bound| bound.observation.expires_at_ms > now);
    let bound = observations
        .get(&input.observation_id)
        .filter(|bound| {
            bound.session_hash == session_hash(session_id)
                && bound.observation.generation == input.generation
        })
        .ok_or("semantic UI observation unavailable, stale, or session-mismatched")?;
    bound
        .observation
        .validate(now)
        .map_err(|_| "semantic UI observation unavailable, stale, or session-mismatched")?;
    let element = bound
        .observation
        .elements
        .iter()
        .find(|element| element.tag == input.tag)
        .ok_or("semantic target unavailable, stale, or ambiguous")?;
    let actionability = element.actionability;
    let common = actionability.visible
        && actionability.enabled
        && actionability.unambiguous
        && actionability.stable;
    let actionable = match tool_name {
        "click" => common && actionability.invokable,
        "set_value" => common && actionability.editable,
        _ => false,
    };
    if !actionable {
        return Err("semantic target unavailable, stale, or ambiguous");
    }
    let target = bound
        .observation
        .target(&input.tag)
        .map_err(|_| "semantic target unavailable, stale, or ambiguous")?;
    Ok(BoundTarget {
        target,
        expires_at_ms: bound.observation.expires_at_ms,
        role: element.role.clone(),
        name: element.name.clone(),
    })
}

fn public_observation(observation: &SemanticObservation) -> Value {
    let elements = observation
        .elements
        .iter()
        .filter_map(|element| {
            let actionability = element.actionability;
            let common = actionability.visible
                && actionability.enabled
                && actionability.unambiguous
                && actionability.stable;
            let mut actions = Vec::new();
            if common && actionability.invokable {
                actions.push("click");
            }
            if common && actionability.editable {
                actions.push("set_value");
            }
            if actions.is_empty() {
                return None;
            }
            Some(json!({
                "tag": element.tag,
                "role": element.role,
                "name": public_name(&element.role, element.name.as_deref()),
                "actions": actions,
            }))
        })
        .collect::<Vec<_>>();
    json!({
        "observation_id": observation.observation_id,
        "generation": observation.generation,
        "expires_at_ms": observation.expires_at_ms,
        "truncated": observation.truncated,
        "elements": elements,
    })
}

fn public_name<'a>(role: &str, name: Option<&'a str>) -> Option<&'a str> {
    let role = role.to_ascii_lowercase();
    (!role.contains("password") && !role.contains("secure"))
        .then_some(name)
        .flatten()
}

struct SemanticInput {
    observation_id: String,
    generation: u64,
    tag: String,
    value: Option<String>,
    interaction_mode: InteractionMode,
}

impl SemanticInput {
    fn parse(tool_name: &str, args: &Value) -> Option<Self> {
        let allowed = match tool_name {
            "click" => &[
                "observation_id",
                "generation",
                "tag",
                "interaction_mode",
                "approval_request_id",
            ][..],
            "set_value" => &[
                "observation_id",
                "generation",
                "tag",
                "value",
                "interaction_mode",
                "approval_request_id",
            ][..],
            _ => return None,
        };
        if !valid_keys(args, allowed) || !valid_approval_id(args) {
            return None;
        }
        let observation_id = args.get("observation_id")?.as_str()?;
        let generation = args.get("generation")?.as_u64()?;
        let tag = args.get("tag")?.as_str()?;
        let interaction_mode = match args.get("interaction_mode")?.as_str()? {
            "interactive" => InteractionMode::Interactive,
            "background_only" => InteractionMode::BackgroundOnly,
            _ => return None,
        };
        if !is_lower_hash(observation_id)
            || generation == 0
            || generation > MAX_SAFE_INTEGER
            || !valid_tag(tag)
        {
            return None;
        }
        let value = match tool_name {
            "set_value" => {
                let value = args.get("value")?.as_str()?;
                if value.chars().count() > MAX_VALUE_CHARS
                    || value.len() > MAX_VALUE_BYTES
                    || value.contains('\0')
                {
                    return None;
                }
                Some(value.to_string())
            }
            _ => None,
        };
        Some(Self {
            observation_id: observation_id.to_string(),
            generation,
            tag: tag.to_string(),
            value,
            interaction_mode,
        })
    }
}

fn authorized_request(
    action: Action,
    target: SemanticTargetRef,
    interaction_mode: InteractionMode,
    session_id: &str,
    rpc_request_id: &str,
    state: &AppState,
    observation_expires_at_ms: i64,
) -> Result<ActionRequest> {
    let now = now_ms();
    let operation_id = operation_id(session_id, rpc_request_id);
    let protocol_session_id = format!("poke-session-{}", session_hash(session_id));
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
    let deadline_at_ms = observation_expires_at_ms;
    if deadline_at_ms <= now {
        return Err(Error::msg("semantic observation expired"));
    }
    let verification = match &action {
        Action::SetValue { value } => VerificationPolicy::TargetValueHash {
            sha256: Sha256::digest(value.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        },
        _ => VerificationPolicy::None,
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
                issuer,
                key_id,
                operation_id,
                subject,
                session_id: protocol_session_id,
                risk: SafetyClass::External,
                expires_at_ms: deadline_at_ms,
                policy_generation,
                action_hash: String::new(),
            },
            signature: String::new(),
        },
        action,
        target: TargetRef::Element { target },
        interaction_mode,
        deadline_at_ms,
        verification,
        safety: SafetyClass::External,
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
    Ok(request)
}

fn valid_keys(args: &Value, allowed: &[&str]) -> bool {
    args.as_object()
        .is_some_and(|object| object.keys().all(|key| allowed.contains(&key.as_str())))
}

fn valid_approval_id(args: &Value) -> bool {
    args.get("approval_request_id").is_none_or(|value| {
        value.as_str().is_some_and(|value| {
            value.len() == 32
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    })
}

fn valid_tag(value: &str) -> bool {
    let Some(index) = value.strip_prefix('e') else {
        return false;
    };
    !index.is_empty()
        && (index == "0" || !index.starts_with('0'))
        && index
            .parse::<usize>()
            .is_ok_and(|index| index < praefectus::semantic::MAX_SEMANTIC_ELEMENTS)
}

fn is_lower_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_arguments() -> Value {
    error_result("invalid semantic tool arguments")
}

fn terminal_error(status: &str, retry_safe: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": status }],
        "structuredContent": { "status": status, "retry_safe": retry_safe },
        "isError": true
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
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

fn session_hash(session_id: &str) -> String {
    hash_parts(&[session_id.as_bytes()])
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

#[cfg(test)]
mod tests {
    use super::*;
    use praefectus::semantic::{
        Actionability, SemanticBackend, SemanticElement, SemanticProvenance,
    };
    use praefectus::{ContextPreservation, DeliveryRoute, Effect, FailureCode, Receipt};

    fn observation() -> SemanticObservation {
        SemanticObservation {
            protocol_version: PROTOCOL_VERSION,
            observation_id: "0".repeat(64),
            generation: 1,
            provenance: SemanticProvenance {
                backend: SemanticBackend::Accessibility,
                backend_name: "test".to_string(),
                process_id: 1,
                process_generation: "generation".to_string(),
                window_id: "window".to_string(),
                document_id: None,
                display_geometry_hash: "1".repeat(64),
                host_opt_ins: Vec::new(),
            },
            observed_at_ms: now_ms(),
            expires_at_ms: now_ms().saturating_add(30_000),
            truncated: false,
            elements: vec![SemanticElement {
                tag: "e0".to_string(),
                element_id: "2".repeat(64),
                parent_id: None,
                fingerprint_hash: "3".repeat(64),
                role: "button".to_string(),
                name: Some("Submit".to_string()),
                bounds: Some(praefectus::Rect {
                    x: 10,
                    y: 20,
                    width: 30,
                    height: 40,
                }),
                actionability: Actionability {
                    visible: true,
                    enabled: true,
                    unambiguous: true,
                    stable: true,
                    receives_events: true,
                    invokable: true,
                    editable: false,
                },
            }],
        }
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
    fn public_observation_exposes_only_bounded_tags_and_labels() {
        let value = public_observation(&observation());
        let encoded = value.to_string();

        assert_eq!(value["elements"][0]["tag"], "e0");
        assert_eq!(value["elements"][0]["actions"], json!(["click"]));
        for secret in [
            "element_id",
            "fingerprint_hash",
            "provenance",
            "bounds",
            "process_id",
            "window_id",
        ] {
            assert!(!encoded.contains(secret));
        }
    }

    #[test]
    fn semantic_approval_summary_identifies_target_without_exposing_values() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let mut observation = observation();
        observation.elements[0].actionability.editable = true;
        bind_observation(&state, "session", observation).unwrap();
        let value = "private value";
        let summary = approval_summary(
            "set_value",
            &json!({
                "observation_id": "0".repeat(64),
                "generation": 1,
                "tag": "e0",
                "value": value,
                "interaction_mode": "background_only"
            }),
            "session",
            &state,
        )
        .unwrap()
        .unwrap();

        assert!(summary.host.contains("semantic 'button' target e0"));
        assert!(
            summary
                .host
                .contains("observation 000000000000 generation 1")
        );
        assert!(summary.host.contains("background_only mode to 13 chars"));
        assert!(summary.host.contains("named 'Submit'"));
        assert!(!summary.host.contains(value));
        assert!(!summary.caller.contains("Submit"));
        assert!(!summary.caller.contains(value));
    }

    #[test]
    fn secure_semantic_approval_redacts_name_from_host_and_caller() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let mut observation = observation();
        observation.elements[0].role = "secure_text_field".to_string();
        observation.elements[0].name = Some("Password account secret".to_string());
        observation.elements[0].actionability.editable = true;
        bind_observation(&state, "session", observation).unwrap();
        let summary = approval_summary(
            "set_value",
            &json!({
                "observation_id": "0".repeat(64),
                "generation": 1,
                "tag": "e0",
                "value": "private value",
                "interaction_mode": "interactive"
            }),
            "session",
            &state,
        )
        .unwrap()
        .unwrap();

        assert!(summary.host.contains("with redacted name"));
        assert!(!summary.host.contains("Password account secret"));
        assert!(!summary.caller.contains("Password account secret"));
        assert!(!summary.caller.contains("name"));
    }

    #[test]
    fn semantic_approval_summary_rejects_unbound_targets() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let result = approval_summary(
            "click",
            &json!({
                "observation_id": "0".repeat(64),
                "generation": 1,
                "tag": "e0",
                "interaction_mode": "interactive"
            }),
            "session",
            &state,
        )
        .unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn semantic_inputs_reject_coordinates_endpoints_and_unbounded_values() {
        let valid = json!({
            "observation_id": "0".repeat(64),
            "generation": 1,
            "tag": "e0",
            "interaction_mode": "interactive"
        });
        assert!(SemanticInput::parse("click", &valid).is_some());
        let mut background = valid.clone();
        background["interaction_mode"] = json!("background_only");
        assert!(SemanticInput::parse("click", &background).is_some());

        for extra in [
            json!({ "x": 10 }),
            json!({ "endpoint": "http://127.0.0.1:9222" }),
            json!({ "screenshot": true }),
            json!({ "session_isolation": "host_isolated" }),
        ] {
            let mut invalid = valid.clone();
            invalid
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert!(SemanticInput::parse("click", &invalid).is_none());
        }
        let mut missing_mode = valid.clone();
        missing_mode
            .as_object_mut()
            .unwrap()
            .remove("interaction_mode");
        assert!(SemanticInput::parse("click", &missing_mode).is_none());
        let mut host_isolation = valid.clone();
        host_isolation["interaction_mode"] = json!("host_isolated");
        assert!(SemanticInput::parse("click", &host_isolation).is_none());
        assert!(
            SemanticInput::parse(
                "click",
                &json!({
                    "observation_id": "0".repeat(64),
                    "generation": 1,
                    "tag": "e00",
                    "interaction_mode": "interactive"
                })
            )
            .is_none()
        );
        assert!(
            SemanticInput::parse(
                "set_value",
                &json!({
                    "observation_id": "0".repeat(64),
                    "generation": 1,
                    "tag": "e0",
                    "value": "x".repeat(MAX_VALUE_CHARS + 1),
                    "interaction_mode": "interactive"
                })
            )
            .is_none()
        );
    }

    #[test]
    fn semantic_observations_are_bound_to_the_mcp_session() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let observation = observation();
        bind_observation(&state, "session-a", observation.clone()).unwrap();
        let input = SemanticInput {
            observation_id: observation.observation_id,
            generation: observation.generation,
            tag: "e0".to_string(),
            value: None,
            interaction_mode: InteractionMode::Interactive,
        };

        assert!(bound_target(&state, "session-a", &input, "click").is_ok());
        assert!(bound_target(&state, "session-b", &input, "click").is_err());
    }

    #[test]
    fn click_tool_authority_is_a_semantic_invoke() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let observation = observation();
        let target = observation.target("e0").unwrap();
        let request = authorized_request(
            Action::Invoke,
            target,
            InteractionMode::Interactive,
            "session",
            "request",
            &state,
            observation.expires_at_ms,
        )
        .unwrap();

        assert!(matches!(request.action, Action::Invoke));
        assert!(matches!(request.target, TargetRef::Element { .. }));
        assert_eq!(request.interaction_mode, InteractionMode::Interactive);
        assert_eq!(request.safety, SafetyClass::External);
        assert_eq!(request.authority.grant.risk, SafetyClass::External);
        assert_eq!(request.authority.grant.operation_id, request.operation_id);
        assert_eq!(request.authority.grant.subject, request.subject);
        assert_eq!(request.authority.grant.session_id, request.session_id);
        assert_eq!(
            request.authority.grant.action_hash,
            normalized_action_hash(&request).unwrap()
        );
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
                delivery_route: DeliveryRoute::TargetAddressed,
                session_isolation: SessionIsolation::SharedDesktop,
                interaction_mode: InteractionMode::BackgroundOnly,
                context_preservation: ContextPreservation::Changed,
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
    fn authority_setup_failure_has_a_stable_redacted_terminal() {
        let response = terminal_error("REJECTED", true);

        assert_eq!(response["structuredContent"]["status"], "REJECTED");
        assert_eq!(response["structuredContent"]["retry_safe"], true);
        assert_eq!(response["isError"], true);
    }

    #[test]
    fn operation_identity_is_stable_and_session_bound() {
        assert_eq!(operation_id("session", "1"), operation_id("session", "1"));
        assert_ne!(operation_id("session", "1"), operation_id("session", "2"));
        assert_ne!(operation_id("session", "1"), operation_id("other", "1"));
    }

    #[test]
    fn repeated_request_keeps_the_same_canonical_hash() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let observation = observation();
        let first = authorized_request(
            Action::Invoke,
            observation.target("e0").unwrap(),
            InteractionMode::BackgroundOnly,
            "session",
            "request",
            &state,
            observation.expires_at_ms,
        )
        .unwrap();
        let second = authorized_request(
            Action::Invoke,
            observation.target("e0").unwrap(),
            InteractionMode::BackgroundOnly,
            "session",
            "request",
            &state,
            observation.expires_at_ms,
        )
        .unwrap();

        assert_eq!(first.deadline_at_ms, observation.expires_at_ms);
        assert_eq!(
            normalized_action_hash(&first).unwrap(),
            normalized_action_hash(&second).unwrap()
        );
    }

    #[test]
    fn authority_hash_binds_interaction_mode() {
        let state = AppState::new(crate::policy::PermissionMode::Full, false).unwrap();
        let observation = observation();
        let interactive = authorized_request(
            Action::Invoke,
            observation.target("e0").unwrap(),
            InteractionMode::Interactive,
            "session",
            "request",
            &state,
            observation.expires_at_ms,
        )
        .unwrap();
        let background = authorized_request(
            Action::Invoke,
            observation.target("e0").unwrap(),
            InteractionMode::BackgroundOnly,
            "session",
            "request",
            &state,
            observation.expires_at_ms,
        )
        .unwrap();

        assert_ne!(
            normalized_action_hash(&interactive).unwrap(),
            normalized_action_hash(&background).unwrap()
        );
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
                action_name: "click".to_string(),
                action_hash: "action".to_string(),
                started_at_ms: 1,
                finished_at_ms: 2,
                backend: "test".to_string(),
                fallback_chain: Vec::new(),
                delivery_route: DeliveryRoute::TargetAddressed,
                session_isolation: SessionIsolation::SharedDesktop,
                interaction_mode: InteractionMode::Interactive,
                context_preservation: ContextPreservation::NotApplicable,
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
