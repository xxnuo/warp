// Onboarding library crate

mod agent_onboarding_view;
pub mod callout;
mod model;
pub mod slides;
pub mod telemetry;

/// The user's intention selected during onboarding slides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingIntention {
    Terminal,
    AgentDrivenDevelopment,
}

impl std::fmt::Display for OnboardingIntention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OnboardingIntention::AgentDrivenDevelopment => write!(f, "agent_driven"),
            OnboardingIntention::Terminal => write!(f, "terminal"),
        }
    }
}

pub use callout::{OnboardingCalloutView, OnboardingKeybindings};

/// User-facing names of the AI features enabled when the agent intention is selected.
/// Shared by the intention slide's agent card checklist and the login slide's
/// skip-login confirmation dialog so the two always stay in sync.
pub const AI_FEATURES: &[&str] = &[
    "Warp agents",
    "Oz Cloud Agents Platform",
    "Prompt suggestions",
    "Next command predictions",
    "Full Terminal Use",
    "Codebase Context",
    "Remote Control with Claude Code, Codex, and other agents",
];

pub const AI_FEATURE_KEYS: &[&str] = &[
    "onboarding.intention.ai_feature.warp_agents",
    "onboarding.intention.ai_feature.oz_cloud_agents",
    "onboarding.intention.ai_feature.next_command_predictions",
    "onboarding.intention.ai_feature.prompt_suggestions",
    "onboarding.intention.ai_feature.codebase_context",
    "onboarding.intention.ai_feature.remote_control",
    "onboarding.intention.ai_feature.agents_over_ssh",
];

/// User-facing names of the Warp Drive features enabled when the terminal
/// intention is selected with Warp Drive turned on. Shared by the login slide's
/// skip-login confirmation dialog so the list stays in sync with any future
/// surfaces that need it.
pub const WARP_DRIVE_FEATURES: &[&str] = &["Warp Drive", "Session Sharing"];

pub const WARP_DRIVE_FEATURE_KEYS: &[&str] = &[
    "onboarding.feature.warp_drive",
    "onboarding.feature.session_sharing",
];

cfg_if::cfg_if! {
    if #[cfg(feature = "bin")] {
        mod telemetry_provider;
        pub use telemetry_provider::MockTelemetryContextProvider;
    }
}

pub mod components;
mod visuals;

/// The default mode for new sessions, chosen during onboarding.
/// Mapped to `DefaultSessionMode` at the application boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionDefault {
    #[default]
    Agent,
    Terminal,
}

impl std::fmt::Display for SessionDefault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionDefault::Agent => write!(f, "agent"),
            SessionDefault::Terminal => write!(f, "terminal"),
        }
    }
}

pub use agent_onboarding_view::{AgentOnboardingAction, AgentOnboardingEvent, AgentOnboardingView};
pub use model::{OnboardingAuthState, SelectedSettings, UICustomizationSettings};
pub use slides::ProjectOnboardingSettings;
pub use telemetry::OnboardingEvent;

pub fn init(app: &mut warpui_core::AppContext) {
    agent_onboarding_view::init(app);
    callout::init(app);
}
