use warp_graphql::ai::{AgentTaskState, PlatformErrorCode};
use warp_i18n::{tr, tr_with};

use super::terminal::ShareSessionError;
use super::AgentDriverError;
use crate::ai::blocklist::local_agent_task_sync_model::classify_renderable_error;
use crate::server::server_api::ai::TaskStatusUpdate;

/// Classify an `AgentDriverError` into a task state and a `TaskStatusUpdate`
/// suitable for reporting via `update_agent_task`.
pub fn classify_driver_error(error: &AgentDriverError) -> (AgentTaskState, TaskStatusUpdate) {
    match error {
        // --- Warp-side errors (task → ERROR) ---
        AgentDriverError::TerminalUnavailable | AgentDriverError::InvalidRuntimeState => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.internal_error_retry_support"),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::BootstrapFailed => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.terminal_start_failed"),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::ShareSessionFailed { error: share_err } => {
            let message = match share_err {
                ShareSessionError::Internal(_) => tr("ai.task_status.driver.share_internal"),
                ShareSessionError::Failed(reason) => {
                    // The reason string comes from the session-sharing layer and is aimed at
                    // interactive users (e.g. "try sharing again"). Provide a cloud-agent-
                    // appropriate message instead of wrapping it, which would produce
                    // repetitive "try again" text.
                    tr_with("ai.task_status.driver.share_failed", &[("reason", reason)])
                }
                ShareSessionError::Disabled => tr("ai.task_status.driver.share_disabled"),
                ShareSessionError::Timeout => tr("ai.task_status.driver.share_timeout"),
                ShareSessionError::Interrupted => tr("ai.task_status.driver.share_interrupted"),
            };
            (
                AgentTaskState::Error,
                TaskStatusUpdate::with_error_code(
                    message,
                    match share_err {
                        ShareSessionError::Disabled => PlatformErrorCode::FeatureNotAvailable,
                        _ => PlatformErrorCode::InternalError,
                    },
                ),
            )
        }
        AgentDriverError::WarpDriveSyncFailed => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.warp_drive_sync_failed"),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::NotLoggedIn => {
            let bin = warp_cli::binary_name().unwrap_or_else(|| "warp".to_string());
            (
                AgentTaskState::Error,
                TaskStatusUpdate::with_error_code(
                    tr_with("ai.task_status.driver.auth_required", &[("bin", &bin)]),
                    PlatformErrorCode::AuthenticationRequired,
                ),
            )
        }
        AgentDriverError::CloudProviderSetupFailed(err) => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.cloud_access_error",
                    &[("error", &format!("{err:#}"))],
                ),
                PlatformErrorCode::InternalError,
            ),
        ),

        // --- User-side errors (task → FAILED) ---
        AgentDriverError::MCPServerNotFound(uuid) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.mcp_server_not_found",
                    &[("uuid", &uuid.to_string())],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::MCPStartupFailed => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.mcp_startup_failed"),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::MCPJsonParseError(msg) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.mcp_json_parse_failed",
                    &[("message", msg)],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::MCPMissingVariables => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.mcp_missing_variables"),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::ProfileError(name) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with("ai.task_status.driver.profile_not_found", &[("name", name)]),
                PlatformErrorCode::ResourceNotFound,
            ),
        ),
        AgentDriverError::AIWorkflowNotFound(id) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.saved_prompt_not_found",
                    &[("id", id)],
                ),
                PlatformErrorCode::ResourceNotFound,
            ),
        ),
        AgentDriverError::EnvironmentNotFound(id) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with("ai.task_status.driver.environment_not_found", &[("id", id)]),
                PlatformErrorCode::ResourceNotFound,
            ),
        ),
        AgentDriverError::EnvironmentSetupFailed(msg) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.environment_setup_failed",
                    &[("message", msg)],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::InvalidWorkingDirectory { path, .. } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.invalid_working_directory",
                    &[("path", &path.display().to_string())],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),

        // --- Conversation errors ---
        // Delegate to classify_renderable_error for proper ERROR vs FAILED
        // distinction and PlatformErrorCode. This is a belt-and-suspenders
        // fallback — LocalAgentTaskSyncModel handles most conversation errors,
        // but the driver catches them too if the conversation ends with an error.
        AgentDriverError::ConversationError { error } => {
            let (state, update) = classify_renderable_error(error);
            (
                state,
                update.unwrap_or_else(|| {
                    TaskStatusUpdate::with_error_code(
                        error.to_string(),
                        PlatformErrorCode::InternalError,
                    )
                }),
            )
        }

        // --- Cancellation / Blocked (no error code) ---
        AgentDriverError::ConversationCancelled { .. } => (
            AgentTaskState::Cancelled,
            TaskStatusUpdate::message(tr("ai.task_status.task_cancelled_with_period")),
        ),
        AgentDriverError::ConversationBlocked { blocked_action } => (
            AgentTaskState::Blocked,
            TaskStatusUpdate::message(tr_with(
                "ai.task_status.blocked_waiting_for_confirmation",
                &[("blocked_action", blocked_action)],
            )),
        ),

        // --- Setup errors ---
        AgentDriverError::TeamMetadataRefreshTimeout => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr("ai.task_status.driver.team_metadata_timeout"),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::SkillResolutionFailed(msg) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.skill_resolution_failed",
                    &[("message", msg)],
                ),
                PlatformErrorCode::ResourceNotFound,
            ),
        ),
        AgentDriverError::ConfigBuildFailed(err) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.config_build_failed",
                    &[("error", &err.to_string())],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::PromptResolutionFailed(err) => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.prompt_resolution_failed",
                    &[("error", &err.to_string())],
                ),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::SecretsFetchFailed(err) => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.secrets_fetch_failed",
                    &[("error", &err.to_string())],
                ),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::AwsBedrockCredentialsFailed(msg) => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.aws_bedrock_credentials_failed",
                    &[("message", msg)],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::ConversationLoadFailed(msg) => (
            AgentTaskState::Error,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.conversation_load_failed",
                    &[("message", msg)],
                ),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::ConversationHarnessMismatch {
            conversation_id,
            expected,
            got,
        } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.conversation_harness_mismatch",
                    &[
                        ("conversation_id", &conversation_id.to_string()),
                        ("expected", &expected.to_string()),
                        ("got", &got.to_string()),
                    ],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::TaskHarnessMismatch {
            task_id,
            expected,
            got,
        } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.task_harness_mismatch",
                    &[
                        ("task_id", &task_id.to_string()),
                        ("expected", &expected.to_string()),
                        ("got", &got.to_string()),
                    ],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::ConversationResumeStateMissing {
            harness,
            conversation_id,
        } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.conversation_resume_state_missing",
                    &[
                        ("conversation_id", &conversation_id.to_string()),
                        ("harness", &harness.to_string()),
                    ],
                ),
                PlatformErrorCode::ResourceNotFound,
            ),
        ),
        AgentDriverError::HarnessCommandFailed { exit_code } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.harness_command_failed",
                    &[("exit_code", &exit_code.to_string())],
                ),
                PlatformErrorCode::InternalError,
            ),
        ),
        AgentDriverError::HarnessSetupFailed { harness, reason } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.harness_setup_failed",
                    &[("harness", &harness.to_string()), ("reason", reason)],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::HarnessConfigSetupFailed { harness, error } => (
            AgentTaskState::Failed,
            TaskStatusUpdate::with_error_code(
                tr_with(
                    "ai.task_status.driver.harness_config_setup_failed",
                    &[
                        ("harness", &harness.to_string()),
                        ("error", &error.to_string()),
                    ],
                ),
                PlatformErrorCode::EnvironmentSetupFailed,
            ),
        ),
        AgentDriverError::HarnessAuthCheckFailed { harness, detail } => {
            let message = tr_with(
                "ai.task_status.driver.harness_auth_check_failed",
                &[("harness", &harness.to_string())],
            );
            log::error!("Preflight detail for {harness}: {detail}");
            (
                AgentTaskState::Failed,
                TaskStatusUpdate::with_error_code(
                    message,
                    PlatformErrorCode::AuthenticationRequired,
                ),
            )
        }
        AgentDriverError::HarnessRuntimeFailureDetected {
            harness,
            pattern,
            excerpt,
        } => {
            let message = tr_with(
                "ai.task_status.driver.harness_runtime_failure_detected",
                &[
                    ("harness", &harness.to_string()),
                    ("pattern", pattern),
                    ("excerpt", excerpt),
                ],
            );
            log::error!("Runtime failure for {harness}: pattern={pattern}, excerpt={excerpt}");
            (
                AgentTaskState::Failed,
                TaskStatusUpdate::with_error_code(
                    message,
                    PlatformErrorCode::AuthenticationRequired,
                ),
            )
        }
    }
}

#[cfg(test)]
#[path = "error_classification_tests.rs"]
mod tests;
