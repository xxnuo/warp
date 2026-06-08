use warp_core::ui::appearance::Appearance;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::{Fill, WarpTheme};
use warp_i18n::tr;
use warpui::color::ColorU;
use warpui::elements::{ConstrainedBox, Container, CornerRadius, Radius};
use warpui::Element;

use crate::ai::agent::conversation::{ConversationStatus, StatusColorStyle};
use crate::ai::agent_conversations_model::AgentRunDisplayStatus;
use crate::ui_components::icons::Icon;

/// Padding around the status icon
pub const STATUS_ELEMENT_PADDING: f32 = 2.;

pub trait StatusElementStyle {
    fn status_icon_and_color(&self, theme: &WarpTheme) -> (Icon, ColorU);
}

pub fn conversation_status_label(status: &ConversationStatus) -> String {
    match status {
        ConversationStatus::InProgress => tr("agent_status.in_progress"),
        ConversationStatus::TransientError => tr("agent_status.in_progress"),
        ConversationStatus::Success => tr("agent_status.done"),
        ConversationStatus::Error => tr("agent_status.error"),
        ConversationStatus::Cancelled => tr("agent_status.cancelled"),
        ConversationStatus::Blocked { .. } => tr("agent_status.blocked"),
        ConversationStatus::WaitingForEvents => tr("agent_status.in_progress"),
    }
}

pub fn agent_run_display_status_label(status: &AgentRunDisplayStatus) -> String {
    match status {
        AgentRunDisplayStatus::TaskQueued => tr("agent_status.queued"),
        AgentRunDisplayStatus::TaskPending => tr("agent_status.pending"),
        AgentRunDisplayStatus::TaskClaimed => tr("agent_status.claimed"),
        AgentRunDisplayStatus::TaskInProgress | AgentRunDisplayStatus::ConversationInProgress => {
            tr("agent_status.in_progress")
        }
        AgentRunDisplayStatus::TaskSucceeded | AgentRunDisplayStatus::ConversationSucceeded => {
            tr("agent_status.done")
        }
        AgentRunDisplayStatus::TaskFailed | AgentRunDisplayStatus::TaskUnknown => {
            tr("agent_status.failed")
        }
        AgentRunDisplayStatus::TaskError | AgentRunDisplayStatus::ConversationError => {
            tr("agent_status.error")
        }
        AgentRunDisplayStatus::TaskBlocked { .. }
        | AgentRunDisplayStatus::ConversationBlocked { .. } => tr("agent_status.blocked"),
        AgentRunDisplayStatus::TaskCancelled | AgentRunDisplayStatus::ConversationCancelled => {
            tr("agent_status.cancelled")
        }
    }
}

impl StatusElementStyle for ConversationStatus {
    fn status_icon_and_color(&self, theme: &WarpTheme) -> (Icon, ColorU) {
        ConversationStatus::status_icon_and_color(self, theme, StatusColorStyle::Standard)
    }
}

impl StatusElementStyle for AgentRunDisplayStatus {
    fn status_icon_and_color(&self, theme: &WarpTheme) -> (Icon, ColorU) {
        AgentRunDisplayStatus::status_icon_and_color(self, theme)
    }
}

/// Render the status element used by agent and conversation views.
pub fn render_status_element(
    status: &impl StatusElementStyle,
    icon_size: f32,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let (icon, color) = status.status_icon_and_color(theme);

    Container::new(
        ConstrainedBox::new(icon.to_warpui_icon(Fill::from(color)).finish())
            .with_width(icon_size)
            .with_height(icon_size)
            .finish(),
    )
    .with_uniform_padding(STATUS_ELEMENT_PADDING)
    .with_background(coloru_with_opacity(color, 10))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .finish()
}
