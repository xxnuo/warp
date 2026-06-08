//! Tips for cloud mode loading screen.

use warp_i18n::tr;
use warpui::keymap::Keystroke;
use warpui::AppContext;

use crate::ai::agent_tips::AITip;

/// A cloud mode tip with text and optional link.
#[derive(Clone, Debug)]
pub struct CloudModeTip {
    text: String,
    link: Option<String>,
}

impl CloudModeTip {
    pub fn new(text: impl Into<String>, link: Option<impl Into<String>>) -> Self {
        Self {
            text: text.into(),
            link: link.map(|l| l.into()),
        }
    }
}

impl AITip for CloudModeTip {
    fn keystroke(&self, _app: &AppContext) -> Option<Keystroke> {
        None
    }

    fn link(&self) -> Option<String> {
        self.link.clone()
    }

    fn description(&self) -> &str {
        &self.text
    }

    // Uses the default implementation which adds "Tip: " prefix and parses backticks as inline code
}

/// Returns a collection of tips for the cloud mode loading screen.
pub fn get_cloud_mode_tips() -> Vec<CloudModeTip> {
    fn tip(key: &str, link: Option<&str>) -> CloudModeTip {
        CloudModeTip::new(tr(key), link)
    }

    vec![
        tip(
            "ambient_agent.tips.slack_integration",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/slack"),
        ),
        tip(
            "ambient_agent.tips.programmatic_agents",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.secrets_command",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/secrets"),
        ),
        tip("ambient_agent.tips.view_runs", Some("https://oz.warp.dev")),
        tip(
            "ambient_agent.tips.session_sharing_realtime",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/viewing-cloud-agent-runs"),
        ),
        tip(
            "ambient_agent.tips.recurring_agents",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.linear_bugs",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/linear"),
        ),
        tip(
            "ambient_agent.tips.ci_failures",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/github-actions"),
        ),
        tip(
            "ambient_agent.tips.github_actions",
            Some("https://github.com/warpdotdev/oz-agent-action"),
        ),
        tip(
            "ambient_agent.tips.rest_api",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.reusable_environments",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/environments"),
        ),
        tip(
            "ambient_agent.tips.share_session_links",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/viewing-cloud-agent-runs"),
        ),
        tip(
            "ambient_agent.tips.share_flag",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/platform"),
        ),
        tip(
            "ambient_agent.tips.fork_completed_session",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/viewing-cloud-agent-runs"),
        ),
        tip(
            "ambient_agent.tips.database_questions",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations"),
        ),
        tip(
            "ambient_agent.tips.scheduled_feature_flags",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.linear_mentions",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/linear"),
        ),
        tip(
            "ambient_agent.tips.remote_dev_boxes",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/platform"),
        ),
        tip(
            "ambient_agent.tips.mcp_servers",
            Some("https://docs.warp.dev/agent-platform/capabilities/mcp"),
        ),
        tip(
            "ambient_agent.tips.agent_run_cli",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/platform"),
        ),
        tip(
            "ambient_agent.tips.teammate_runs",
            Some("https://oz.warp.dev"),
        ),
        tip(
            "ambient_agent.tips.triage_github_issues",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/github-actions"),
        ),
        tip(
            "ambient_agent.tips.daily_issue_summaries",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/github-actions"),
        ),
        tip(
            "ambient_agent.tips.pr_reviews",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/github-actions"),
        ),
        tip(
            "ambient_agent.tips.environment_create",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/environments"),
        ),
        tip(
            "ambient_agent.tips.webhook_incidents",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.restart_services",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers"),
        ),
        tip(
            "ambient_agent.tips.personal_secrets",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/secrets"),
        ),
        tip(
            "ambient_agent.tips.team_secrets",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/secrets"),
        ),
        tip(
            "ambient_agent.tips.dependency_updates",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.format_lint_schedule",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.schedule_create",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.schedule_pause",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/triggers/scheduled-agents"),
        ),
        tip(
            "ambient_agent.tips.mcp_list",
            Some("https://docs.warp.dev/agent-platform/capabilities/mcp"),
        ),
        tip(
            "ambient_agent.tips.slack_bot",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/slack"),
        ),
        tip(
            "ambient_agent.tips.slack_mentions",
            Some("https://docs.warp.dev/agent-platform/cloud-agents/integrations/slack"),
        ),
        tip(
            "ambient_agent.tips.typescript_sdk",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.python_sdk",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.monitor_success_rates",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
        tip(
            "ambient_agent.tips.activity_dashboard",
            Some("https://docs.warp.dev/reference/api-and-sdk"),
        ),
    ]
}
