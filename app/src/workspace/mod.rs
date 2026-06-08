mod action;
mod active_session;
pub(crate) mod auto_handoff;
pub mod bonus_grant_notification_model;
#[cfg(target_os = "macos")]
mod cli_install;
mod close_session_confirmation_dialog;
pub(crate) mod cross_window_tab_drag;
pub mod delete_conversation_confirmation_dialog;
mod global_actions;
pub mod header_toolbar_editor;
pub mod header_toolbar_item;
pub mod hoa_onboarding;
mod home;
mod lightbox_view;
mod native_modal;
mod one_time_modal_model;
mod registry;
pub mod rewind_confirmation_dialog;
pub mod sync_inputs;
pub mod tab_group;
pub mod tab_settings;
mod toast_stack;
pub mod util;
pub mod view;

pub use action::{
    AutoCloudHandoffTrigger, CommandSearchOptions, InitContent, RestoreConversationLayout,
    TabContextMenuAnchor, VerticalTabsPaneContextMenuTarget, WorkspaceAction,
};
pub use active_session::ActiveSession;
pub use global_actions::{
    ForkAIConversationParams, ForkFromExchange, ForkedConversationDestination,
};
use serde::{Deserialize, Serialize};
pub use util::{active_terminal_in_window, PaneViewLocator, TabMovement};
pub use view::{
    Workspace, NEW_SESSION_MENU_BUTTON_POSITION_ID, NEW_TAB_BUTTON_POSITION_ID,
    PANEL_HEADER_HEIGHT, TAB_BAR_HEIGHT, TOTAL_TAB_BAR_HEIGHT, WORKSPACE_PADDING,
};
use warp_core::context_flag::ContextFlag;
use warp_i18n::tr;
use warpui::accessibility::AccessibilityVerbosity;
use warpui::elements::DropTargetData;
use warpui::keymap::{BindingDescription, EditableBinding, FixedBinding};
use warpui::AppContext;

use crate::ai::blocklist::new_agent_pane_label;
use crate::channel::{Channel, ChannelState};
use crate::features::FeatureFlag;
use crate::palette::PaletteMode;
use crate::pane_group::TabBarHoverIndex;
use crate::server::telemetry::{AgentModeEntrypoint, PaletteSource};
use crate::settings_view::{self, flags, SettingsSection};
use crate::tab::uses_vertical_tabs;
use crate::util::bindings::{self, cmd_or_ctrl_shift, is_binding_pty_compliant, CustomAction};
use crate::{code, modal, notebooks, tab_configs};

// Helper function to access panel header corner radius from other modules
pub fn panel_header_corner_radius() -> warpui::elements::CornerRadius {
    warpui::elements::CornerRadius::with_top(warpui::elements::Radius::Pixels(8.))
}

pub use one_time_modal_model::OneTimeModalModel;
pub use registry::WorkspaceRegistry;
pub use toast_stack::ToastStack;

use crate::workspace::view::{
    LEFT_PANEL_AGENT_CONVERSATIONS_BINDING_NAME, LEFT_PANEL_GLOBAL_SEARCH_BINDING_NAME,
    LEFT_PANEL_PROJECT_EXPLORER_BINDING_NAME, LEFT_PANEL_WARP_DRIVE_BINDING_NAME,
    NEW_AGENT_TAB_BINDING_NAME, NEW_AMBIENT_AGENT_TAB_BINDING_NAME, NEW_TAB_BINDING_NAME,
    NEW_TERMINAL_TAB_BINDING_NAME, OPEN_GLOBAL_SEARCH_BINDING_NAME,
    TOGGLE_CONVERSATION_LIST_VIEW_BINDING_NAME, TOGGLE_NOTIFICATION_MAILBOX_BINDING_NAME,
    TOGGLE_PROJECT_EXPLORER_BINDING_NAME, TOGGLE_RIGHT_PANEL_BINDING_NAME,
    TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME, TOGGLE_VERTICAL_TABS_PANEL_BINDING_NAME,
    TOGGLE_WARP_DRIVE_BINDING_NAME,
};

pub fn init(app: &mut AppContext) {
    app.add_singleton_model(|_| WorkspaceRegistry::new());
    app.add_singleton_model(|_| cross_window_tab_drag::CrossWindowTabDrag::new());
    use warpui::keymap::macros::*;
    app.register_binding_validator::<Workspace>(is_binding_pty_compliant);

    modal::init(app);
    native_modal::init(app);
    lightbox_view::init(app);
    rewind_confirmation_dialog::init(app);
    delete_conversation_confirmation_dialog::init(app);
    crate::tab_configs::remove_confirmation_dialog::init(app);
    hoa_onboarding::init(app);
    tab_configs::session_config_modal::init(app);
    view::launch_modal::oz_launch::init(app);
    view::openwarp_launch_modal::init(app);
    view::orchestration_launch_modal::init(app);
    view::cloud_agent_capacity_modal::init(app);
    view::codex_modal::init(app);
    view::free_tier_limit_hit_modal::init(app);
    view::global_search::view::GlobalSearchView::init(app);
    view::right_panel::RightPanelView::init(app);
    header_toolbar_editor::init(app);
    view::conversation_list::view::register_conversation_list_view_bindings(app);

    settings_view::init_actions_from_parent_view(app, &id!("Workspace"), |settings_action| {
        WorkspaceAction::DispatchToSettingsTab(settings_action)
    });
    global_actions::init_global_actions(app);
    notebooks::init(app);
    code::init(app);
    sync_inputs::init(app);
    lsp::init(app);

    app.register_fixed_bindings([FixedBinding::empty(
        tr("workspace.binding.dump_debug_info"),
        WorkspaceAction::DumpDebugInfo,
        id!("Workspace"),
    )]);
    app.register_fixed_bindings([
        FixedBinding::new(
            "escape",
            WorkspaceAction::DismissSessionConfigTabConfigChip,
            id!("Workspace") & id!(flags::SESSION_CONFIG_TAB_CONFIG_CHIP_OPEN),
        ),
        FixedBinding::new(
            "enter",
            WorkspaceAction::DismissSessionConfigTabConfigChip,
            id!("Workspace") & id!(flags::SESSION_CONFIG_TAB_CONFIG_CHIP_OPEN),
        ),
    ]);

    if ChannelState::enable_debug_features() {
        let crash_description = if cfg!(target_os = "macos") {
            tr("workspace.binding.crash_app_testing_sentry_cocoa")
        } else {
            tr("workspace.binding.crash_app_testing_sentry_native")
        };
        app.register_editable_bindings([
            EditableBinding::new("workspace:crash", crash_description, WorkspaceAction::Crash)
                .with_context_predicate(id!("Workspace")),
            EditableBinding::new(
                "workspace:log_review_comment_send_status_for_active_tab",
                tr("workspace.binding.debug_log_review_comment_send_status"),
                WorkspaceAction::LogReviewCommentSendStatusForActiveTab,
            )
            .with_context_predicate(id!("Workspace")),
            EditableBinding::new(
                "workspace:panic",
                tr("workspace.binding.trigger_panic_testing_sentry_rust"),
                WorkspaceAction::Panic,
            )
            .with_context_predicate(id!("Workspace")),
            EditableBinding::new(
                "workspace:open_view_tree_debug_view",
                tr("workspace.binding.open_view_tree_debugger"),
                WorkspaceAction::OpenViewTreeDebugWindow,
            )
            .with_context_predicate(id!("Workspace")),
        ]);
        app.register_fixed_bindings([FixedBinding::empty(
            tr("workspace.binding.debug_view_first_time_user_experience"),
            WorkspaceAction::AddGetStartedTab,
            id!("Workspace"),
        )]);
        #[cfg(debug_assertions)]
        {
            // Debug actions for build plan migration modal (command palette only)
            app.register_editable_bindings([
                EditableBinding::new(
                    "workspace:open_build_plan_migration_modal",
                    tr("workspace.binding.debug_open_build_plan_migration_modal"),
                    WorkspaceAction::OpenBuildPlanMigrationModal,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:reset_build_plan_migration_modal_state",
                    tr("workspace.binding.debug_reset_build_plan_migration_modal_state"),
                    WorkspaceAction::ResetBuildPlanMigrationModalState,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:debug_reset_aws_bedrock_login_banner_dismissed",
                    tr("workspace.binding.debug_un_dismiss_aws_login_banner"),
                    WorkspaceAction::DebugResetAwsBedrockLoginBannerDismissed,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:open_oz_launch_modal",
                    tr("workspace.binding.debug_open_oz_launch_modal"),
                    WorkspaceAction::OpenOzLaunchModal,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:reset_oz_launch_modal_state",
                    tr("workspace.binding.debug_reset_oz_launch_modal_state"),
                    WorkspaceAction::ResetOzLaunchModalState,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:open_openwarp_launch_modal",
                    tr("workspace.binding.debug_open_openwarp_launch_modal"),
                    WorkspaceAction::OpenOpenWarpLaunchModal,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:reset_openwarp_launch_modal_state",
                    tr("workspace.binding.debug_reset_openwarp_launch_modal_state"),
                    WorkspaceAction::ResetOpenWarpLaunchModalState,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:open_orchestration_launch_modal",
                    tr("workspace.binding.debug_open_orchestration_launch_modal"),
                    WorkspaceAction::OpenOrchestrationLaunchModal,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:reset_orchestration_launch_modal_state",
                    tr("workspace.binding.debug_reset_orchestration_launch_modal_state"),
                    WorkspaceAction::ResetOrchestrationLaunchModalState,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:install_opencode_warp_plugin",
                    tr("workspace.binding.debug_install_opencode_warp_plugin"),
                    WorkspaceAction::InstallOpenCodeWarpPlugin,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:use_local_opencode_warp_plugin",
                    tr("workspace.binding.debug_use_local_opencode_warp_plugin"),
                    WorkspaceAction::UseLocalOpenCodeWarpPlugin,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:open_session_config_modal",
                    tr("workspace.binding.debug_open_session_config_modal"),
                    WorkspaceAction::ShowSessionConfigModal,
                )
                .with_context_predicate(id!("Workspace")),
                EditableBinding::new(
                    "workspace:show_hoa_onboarding_flow",
                    tr("workspace.binding.debug_start_hoa_onboarding_flow"),
                    WorkspaceAction::ShowHoaOnboardingFlow,
                )
                .with_context_predicate(id!("Workspace")),
            ]);
        }
    }

    #[cfg(target_os = "macos")]
    app.register_editable_bindings([EditableBinding::new(
        "workspace:sample_process",
        tr("workspace.binding.sample_process"),
        WorkspaceAction::SampleProcess,
    )
    .with_context_predicate(id!("Workspace"))]);

    #[cfg(feature = "dhat_heap_profiling")]
    {
        app.register_editable_bindings([EditableBinding::new(
            "workspace:dump_heap_profile",
            tr("workspace.binding.dump_heap_profile_once"),
            WorkspaceAction::DumpHeapProfile,
        )
        .with_context_predicate(id!("Workspace"))]);
    }

    app.register_fixed_bindings([
        FixedBinding::custom(
            CustomAction::CycleNextSession,
            WorkspaceAction::CycleNextSession,
            tr("workspace.binding.switch_to_next_tab"),
            id!("Workspace") & id!("Workspace_MultipleTabs"),
        ),
        FixedBinding::custom(
            CustomAction::CyclePrevSession,
            WorkspaceAction::CyclePrevSession,
            tr("workspace.binding.switch_to_previous_tab"),
            id!("Workspace") & id!("Workspace_MultipleTabs"),
        ),
        FixedBinding::custom(
            CustomAction::AddWindow,
            WorkspaceAction::AddWindow,
            tr("workspace.binding.create_new_window"),
            id!("Workspace"),
        )
        .with_enabled(|| ContextFlag::CreateNewSession.is_enabled()),
        FixedBinding::custom(
            CustomAction::NewFile,
            WorkspaceAction::NewCodeFile,
            tr("workspace.binding.new_file"),
            id!("Workspace") & !id!("Workspace_ViewOnlySharedSession"),
        ),
    ]);

    if FeatureFlag::UIZoom.is_enabled() {
        app.register_fixed_bindings([
            FixedBinding::custom(
                CustomAction::IncreaseZoom,
                WorkspaceAction::IncreaseZoom,
                tr("workspace.binding.zoom_in"),
                id!("Workspace"),
            )
            .with_group(bindings::BindingGroup::Settings.as_str()),
            FixedBinding::custom(
                CustomAction::DecreaseZoom,
                WorkspaceAction::DecreaseZoom,
                tr("workspace.binding.zoom_out"),
                id!("Workspace"),
            )
            .with_group(bindings::BindingGroup::Settings.as_str()),
            FixedBinding::custom(
                CustomAction::ResetZoom,
                WorkspaceAction::ResetZoom,
                tr("workspace.binding.reset_zoom"),
                id!("Workspace"),
            )
            .with_group(bindings::BindingGroup::Settings.as_str()),
        ]);
    } else {
        app.register_fixed_bindings([
            FixedBinding::custom(
                CustomAction::IncreaseFontSize,
                WorkspaceAction::IncreaseFontSize,
                tr("workspace.binding.increase_font_size"),
                id!("Workspace"),
            )
            .with_group(bindings::BindingGroup::Settings.as_str()),
            FixedBinding::custom(
                CustomAction::DecreaseFontSize,
                WorkspaceAction::DecreaseFontSize,
                tr("workspace.binding.decrease_font_size"),
                id!("Workspace"),
            )
            .with_group(bindings::BindingGroup::Settings.as_str()),
        ]);
    }

    if ContextFlag::LaunchConfigurations.is_enabled() {
        app.register_fixed_bindings([FixedBinding::custom(
            CustomAction::SaveCurrentConfig,
            WorkspaceAction::OpenLaunchConfigSaveModal,
            tr("workspace.binding.save_new_launch_configuration"),
            id!("Workspace"),
        )]);
    }

    if ChannelState::channel() == Channel::Integration {
        // Hack: Add explicit bindings for the tests, since the tests' injected
        // keypresses won't trigger Mac menu items. Unfortunately we can't use
        // cfg[test] because we are a separate process!
        app.register_fixed_bindings([
            FixedBinding::new(
                cmd_or_ctrl_shift("t"),
                WorkspaceAction::AddDefaultTab,
                id!("Workspace"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("p"),
                WorkspaceAction::TogglePalette {
                    mode: PaletteMode::Command,
                    source: PaletteSource::IntegrationTest,
                },
                id!("Workspace"),
            ),
            FixedBinding::new(
                "cmdorctrl-,",
                WorkspaceAction::ShowSettings,
                id!("Workspace"),
            ),
        ]);
    }

    if FeatureFlag::UIZoom.is_enabled() {
        app.register_editable_bindings([
            EditableBinding::new(
                "workspace:increase_zoom",
                tr("workspace.binding.increase_zoom_level"),
                WorkspaceAction::IncreaseZoom,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("cmdorctrl-="),
            EditableBinding::new(
                "workspace:decrease_zoom",
                tr("workspace.binding.decrease_zoom_level"),
                WorkspaceAction::DecreaseZoom,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("cmdorctrl--"),
            EditableBinding::new(
                "workspace:reset_zoom",
                tr("workspace.binding.reset_zoom_level_default"),
                WorkspaceAction::ResetZoom,
            )
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_context_predicate(id!("Workspace")),
            EditableBinding::new(
                "workspace:increase_font_size",
                tr("workspace.binding.increase_font_size"),
                WorkspaceAction::IncreaseFontSize,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("alt-shift->"),
            EditableBinding::new(
                "workspace:decrease_font_size",
                tr("workspace.binding.decrease_font_size"),
                WorkspaceAction::DecreaseFontSize,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("alt-shift-<"),
            EditableBinding::new(
                "workspace:reset_font_size",
                tr("workspace.binding.reset_font_size_default"),
                WorkspaceAction::ResetFontSize,
            )
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_context_predicate(id!("Workspace")),
        ]);
    } else {
        app.register_editable_bindings([
            EditableBinding::new(
                "workspace:increase_font_size",
                tr("workspace.binding.increase_font_size"),
                WorkspaceAction::IncreaseFontSize,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("cmdorctrl-="),
            EditableBinding::new(
                "workspace:decrease_font_size",
                tr("workspace.binding.decrease_font_size"),
                WorkspaceAction::DecreaseFontSize,
            )
            .with_context_predicate(id!("Workspace"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_key_binding("cmdorctrl--"),
            EditableBinding::new(
                "workspace:reset_font_size",
                tr("workspace.binding.reset_font_size_default"),
                WorkspaceAction::ResetFontSize,
            )
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_context_predicate(id!("Workspace"))
            .with_key_binding("cmdorctrl-0")
            .with_custom_action(CustomAction::ResetFontSize),
        ]);
    }

    app.register_fixed_bindings([
        // Menu dispatch for the "Open File Picker" custom action.
        FixedBinding::custom(
            CustomAction::ToggleProjectExplorer,
            WorkspaceAction::ToggleProjectExplorer,
            BindingDescription::new(tr("workspace.binding.toggle_project_explorer"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.project_explorer"),
                ),
            id!("Workspace") & id!(flags::SHOW_PROJECT_EXPLORER),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:show_theme_chooser",
            tr("workspace.binding.open_theme_picker"),
            WorkspaceAction::ShowThemeChooserForActiveTheme,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Settings.as_str()),
        EditableBinding::new(
            TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME,
            tr("workspace.binding.open_tab_configs_menu"),
            WorkspaceAction::ToggleTabConfigsMenu,
        )
        .with_context_predicate(id!("Workspace"))
        .with_mac_key_binding("cmd-ctrl-t")
        .with_linux_or_windows_key_binding("ctrl-alt-shift-T"),
        EditableBinding::new(
            "workspace:activate_first_tab",
            tr("workspace.binding.switch_to_first_tab"),
            WorkspaceAction::ActivateTabByNumber(1),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-1"),
        EditableBinding::new(
            "workspace:activate_second_tab",
            tr("workspace.binding.switch_to_second_tab"),
            WorkspaceAction::ActivateTabByNumber(2),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-2"),
        EditableBinding::new(
            "workspace:activate_third_tab",
            tr("workspace.binding.switch_to_third_tab"),
            WorkspaceAction::ActivateTabByNumber(3),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-3"),
        EditableBinding::new(
            "workspace:activate_fourth_tab",
            tr("workspace.binding.switch_to_fourth_tab"),
            WorkspaceAction::ActivateTabByNumber(4),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-4"),
        EditableBinding::new(
            "workspace:activate_fifth_tab",
            tr("workspace.binding.switch_to_fifth_tab"),
            WorkspaceAction::ActivateTabByNumber(5),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-5"),
        EditableBinding::new(
            "workspace:activate_sixth_tab",
            tr("workspace.binding.switch_to_sixth_tab"),
            WorkspaceAction::ActivateTabByNumber(6),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-6"),
        EditableBinding::new(
            "workspace:activate_seventh_tab",
            tr("workspace.binding.switch_to_seventh_tab"),
            WorkspaceAction::ActivateTabByNumber(7),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-7"),
        EditableBinding::new(
            "workspace:activate_eighth_tab",
            tr("workspace.binding.switch_to_eighth_tab"),
            WorkspaceAction::ActivateTabByNumber(8),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-8"),
        EditableBinding::new(
            "workspace:activate_last_tab",
            tr("workspace.binding.switch_to_last_tab"),
            WorkspaceAction::ActivateLastTab,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_key_binding("cmdorctrl-9"),
        EditableBinding::new(
            "workspace:activate_prev_tab",
            tr("workspace.binding.activate_previous_tab"),
            WorkspaceAction::ActivatePrevTab,
        )
        .with_context_predicate(
            id!("Workspace") & id!("Workspace_MultipleTabs") & !id!("Workspace_PaneDragging"),
        )
        .with_mac_key_binding("shift-cmd-{")
        .with_linux_or_windows_key_binding("ctrl-pageup"),
        EditableBinding::new(
            "workspace:activate_next_tab",
            tr("workspace.binding.activate_next_tab"),
            WorkspaceAction::ActivateNextTab,
        )
        .with_context_predicate(
            id!("Workspace") & id!("Workspace_MultipleTabs") & !id!("Workspace_PaneDragging"),
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_mac_key_binding("shift-cmd-}")
        .with_linux_or_windows_key_binding("ctrl-pagedown"),
        EditableBinding::new(
            "pane_group:navigate_prev",
            tr("workspace.binding.activate_previous_pane"),
            WorkspaceAction::NavigatePrevPaneOrPanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_custom_action(CustomAction::ActivatePreviousPane),
        EditableBinding::new(
            "pane_group:navigate_next",
            tr("workspace.binding.activate_next_pane"),
            WorkspaceAction::NavigateNextPaneOrPanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_custom_action(CustomAction::ActivateNextPane),
        EditableBinding::new(
            "workspace:create_team_notebook",
            BindingDescription::new(tr("workspace.binding.create_new_team_notebook"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_team_notebook"),
                ),
            WorkspaceAction::CreateTeamNotebook,
        )
        .with_custom_action(CustomAction::NewTeamNotebook)
        .with_context_predicate(
            id!("Workspace")
                & id!(flags::ENABLE_WARP_DRIVE)
                & id!("WarpDrive_BelongsToTeam")
                & id!("IsOnline"),
        )
        .with_group(bindings::BindingGroup::Notebooks.as_str()),
        EditableBinding::new(
            "workspace:create_personal_notebook",
            BindingDescription::new(tr("workspace.binding.create_new_personal_notebook"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_personal_notebook"),
                ),
            WorkspaceAction::CreatePersonalNotebook,
        )
        .with_group(bindings::BindingGroup::Notebooks.as_str())
        .with_custom_action(CustomAction::NewPersonalNotebook)
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE)),
        EditableBinding::new(
            "workspace:create_team_workflow",
            BindingDescription::new(tr("workspace.binding.create_new_team_workflow"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_team_workflow"),
                ),
            WorkspaceAction::CreateTeamWorkflow,
        )
        .with_custom_action(CustomAction::NewTeamWorkflow)
        .with_context_predicate(
            id!("Workspace")
                & id!(flags::ENABLE_WARP_DRIVE)
                & id!("IsOnline")
                & id!("WarpDrive_BelongsToTeam"),
        )
        .with_group(bindings::BindingGroup::Workflow.as_str()),
        EditableBinding::new(
            "workspace:create_personal_workflow",
            BindingDescription::new(tr("workspace.binding.create_new_personal_workflow"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_personal_workflow"),
                ),
            WorkspaceAction::CreatePersonalWorkflow,
        )
        .with_group(bindings::BindingGroup::Workflow.as_str())
        .with_custom_action(CustomAction::NewPersonalWorkflow)
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE)),
        EditableBinding::new(
            "workspace:create_team_folder",
            BindingDescription::new(tr("workspace.binding.create_new_team_folder"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_team_folder"),
                ),
            WorkspaceAction::CreateTeamFolder,
        )
        .with_context_predicate(
            id!("Workspace")
                & id!(flags::ENABLE_WARP_DRIVE)
                & id!("IsOnline")
                & id!("WarpDrive_BelongsToTeam"),
        )
        .with_group(bindings::BindingGroup::Folders.as_str()),
        EditableBinding::new(
            "workspace:create_personal_folder",
            BindingDescription::new(tr("workspace.binding.create_new_personal_folder"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_personal_folder"),
                ),
            WorkspaceAction::CreatePersonalFolder,
        )
        .with_group(bindings::BindingGroup::Folders.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE) & id!("IsOnline")),
        EditableBinding::new(
            NEW_TAB_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.create_new_tab")),
            WorkspaceAction::AddDefaultTab,
        )
        .with_context_predicate(id!("Workspace") & !id!("Workspace_PaneDragging"))
        .with_custom_action(CustomAction::NewTab)
        .with_enabled(|| ContextFlag::CreateNewSession.is_enabled()),
        EditableBinding::new(
            NEW_TERMINAL_TAB_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.new_terminal_tab")),
            WorkspaceAction::AddTerminalTab {
                hide_homepage: false,
            },
        )
        .with_context_predicate(id!("Workspace") & !id!("Workspace_PaneDragging"))
        .with_custom_action(CustomAction::NewTerminalTab)
        .with_enabled(|| ContextFlag::CreateNewSession.is_enabled()),
        EditableBinding::new(
            NEW_AGENT_TAB_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.new_agent_tab")),
            WorkspaceAction::AddAgentTab,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_custom_action(CustomAction::NewAgentTab)
        .with_context_predicate(
            id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED) & !id!("Workspace_PaneDragging"),
        ),
        EditableBinding::new(
            NEW_AMBIENT_AGENT_TAB_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.new_cloud_agent_tab")),
            WorkspaceAction::AddAmbientAgentTab,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_context_predicate(
            id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED) & !id!("Workspace_PaneDragging"),
        )
        .with_enabled(|| {
            FeatureFlag::AgentView.is_enabled() && FeatureFlag::CloudMode.is_enabled()
        }),
        EditableBinding::new(
            "workspace:toggle_left_panel",
            BindingDescription::new(tr("workspace.binding.open_left_panel")),
            WorkspaceAction::ToggleLeftPanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ToggleWarpDrive),
        EditableBinding::new(
            TOGGLE_RIGHT_PANEL_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.toggle_code_review"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.toggle_code_review"),
                ),
            WorkspaceAction::ToggleRightPanel,
        )
        .with_enabled(|| cfg!(feature = "local_fs"))
        .with_context_predicate(id!("Workspace"))
        .with_mac_key_binding("cmd-shift-+")
        .with_linux_or_windows_key_binding("ctrl-shift-+"),
        EditableBinding::new(
            TOGGLE_VERTICAL_TABS_PANEL_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.toggle_vertical_tabs_panel"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.toggle_vertical_tabs_panel"),
                ),
            WorkspaceAction::ToggleVerticalTabsPanel,
        )
        .with_context_predicate(id!("Workspace") & id!(flags::USE_VERTICAL_TABS_FLAG))
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_enabled(|| FeatureFlag::VerticalTabs.is_enabled())
        .with_key_binding(cmd_or_ctrl_shift("b")),
        EditableBinding::new(
            LEFT_PANEL_PROJECT_EXPLORER_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.left_panel_project_explorer")),
            WorkspaceAction::ToggleProjectExplorer,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_PROJECT_EXPLORER))
        .with_custom_action(CustomAction::ToggleProjectExplorer),
        EditableBinding::new(
            LEFT_PANEL_AGENT_CONVERSATIONS_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.left_panel_agent_conversations")),
            WorkspaceAction::ToggleConversationListView,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_CONVERSATION_HISTORY))
        .with_enabled(|| FeatureFlag::AgentViewConversationListView.is_enabled())
        .with_custom_action(CustomAction::ToggleConversationListView),
        EditableBinding::new(
            LEFT_PANEL_GLOBAL_SEARCH_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.left_panel_global_search")),
            WorkspaceAction::ToggleGlobalSearch,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_GLOBAL_SEARCH))
        .with_enabled(|| FeatureFlag::GlobalSearch.is_enabled())
        .with_custom_action(CustomAction::ToggleGlobalSearch),
        EditableBinding::new(
            "file_tree:toggle_hidden_files",
            BindingDescription::new(tr("workspace.binding.toggle_hidden_files_project_explorer")),
            WorkspaceAction::ToggleHiddenFiles,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_PROJECT_EXPLORER))
        .with_mac_key_binding("cmd-shift->")
        .with_linux_or_windows_key_binding("ctrl-shift->"),
        EditableBinding::new(
            LEFT_PANEL_WARP_DRIVE_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.left_panel_warp_drive")),
            WorkspaceAction::ToggleWarpDrive,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE))
        .with_mac_key_binding("ctrl-4")
        .with_linux_or_windows_key_binding("alt-4"),
        EditableBinding::new(
            TOGGLE_PROJECT_EXPLORER_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.toggle_project_explorer"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.project_explorer"),
                ),
            WorkspaceAction::ToggleProjectExplorer,
        )
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_PROJECT_EXPLORER)),
        EditableBinding::new(
            OPEN_GLOBAL_SEARCH_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.open_global_search"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.global_search"),
                ),
            WorkspaceAction::OpenGlobalSearch,
        )
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_GLOBAL_SEARCH))
        .with_mac_key_binding("cmd-shift-F")
        // we use alt because we use ctrl-shift-f for find because ctrl-f needs to be reserved for the shell
        .with_linux_or_windows_key_binding("alt-shift-F"),
        EditableBinding::new(
            TOGGLE_WARP_DRIVE_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.toggle_warp_drive"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.warp_drive"),
                ),
            WorkspaceAction::ToggleWarpDrive,
        )
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE)),
        EditableBinding::new(
            TOGGLE_CONVERSATION_LIST_VIEW_BINDING_NAME,
            BindingDescription::new(tr("workspace.binding.toggle_agent_conversation_list_view"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.agent_conversation_list_view"),
                ),
            WorkspaceAction::ToggleConversationListView,
        )
        .with_enabled(|| FeatureFlag::AgentViewConversationListView.is_enabled())
        .with_context_predicate(id!("Workspace") & id!(flags::SHOW_CONVERSATION_HISTORY))
        .with_mac_key_binding("cmd-shift-A")
        .with_linux_or_windows_key_binding("ctrl-shift-A")
        .with_group(bindings::BindingGroup::WarpAi.as_str()),
        EditableBinding::new(
            "workspace:close_panel",
            BindingDescription::new(tr("workspace.binding.close_focused_panel"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.close_focused_panel"),
                ),
            WorkspaceAction::ClosePanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::CloseCurrentSession),
        EditableBinding::new(
            "workspace:toggle_command_palette",
            BindingDescription::new(tr("workspace.binding.toggle_command_palette"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.command_palette"),
                ),
            WorkspaceAction::TogglePalette {
                mode: PaletteMode::Command,
                source: PaletteSource::Keybinding,
            },
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace") & !id!("Workspace_CloudConversationWebViewer"))
        .with_custom_action(CustomAction::CommandPalette),
        EditableBinding::new(
            "workspace:move_tab_left",
            BindingDescription::new(tr("workspace.binding.move_tab_left")).with_dynamic_override(
                |ctx| uses_vertical_tabs(ctx).then(|| tr("workspace.binding.move_tab_up")),
            ),
            WorkspaceAction::MoveActiveTabLeft,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(
            id!("Workspace")
                & id!("Workspace_MultipleTabs")
                & !id!("Workspace_LeftmostTabActive")
                & !id!("Workspace_PaneDragging"),
        )
        .with_custom_action(CustomAction::MoveTabLeft),
        EditableBinding::new(
            "workspace:move_tab_right",
            BindingDescription::new(tr("workspace.binding.move_tab_right")).with_dynamic_override(
                |ctx| uses_vertical_tabs(ctx).then(|| tr("workspace.binding.move_tab_down")),
            ),
            WorkspaceAction::MoveActiveTabRight,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(
            id!("Workspace")
                & id!("Workspace_MultipleTabs")
                & !id!("Workspace_RightmostTabActive")
                & !id!("Workspace_PaneDragging"),
        )
        .with_custom_action(CustomAction::MoveTabRight),
        EditableBinding::new(
            "workspace:toggle_keybindings_page",
            tr("workspace.binding.toggle_keyboard_shortcuts"),
            WorkspaceAction::ToggleKeybindingsPage,
        )
        .with_group(bindings::BindingGroup::KeyboardShortcuts.as_str())
        .with_context_predicate(id!("Workspace") & !id!("Workspace_TextOpen"))
        .with_custom_action(CustomAction::ToggleKeybindingsPage),
        EditableBinding::new(
            "workspace:show_keybinding_settings",
            tr("workspace.binding.open_keybindings_editor"),
            WorkspaceAction::ConfigureKeybindingSettings {
                keybinding_name: None,
            },
        )
        .with_group(bindings::BindingGroup::KeyboardShortcuts.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_mac_key_binding("cmd-ctrl-k"),
        EditableBinding::new(
            "workspace:toggle_block_snackbar",
            tr("workspace.binding.toggle_sticky_command_header"),
            WorkspaceAction::ToggleBlockSnackbar,
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
    ]);

    // TODO(PLAT-113): Support a11y on non-MacOS platforms
    if cfg!(target_os = "macos") {
        app.register_editable_bindings([
            EditableBinding::new(
                "workspace:set_a11y_concise_verbosity_level",
                tr("workspace.binding.a11y_set_concise_announcements"),
                WorkspaceAction::SetA11yVerbosityLevel(AccessibilityVerbosity::Concise),
            )
            .with_context_predicate(id!("Workspace"))
            .with_key_binding("cmdorctrl-alt-c"),
            EditableBinding::new(
                "workspace:set_a11y_verbose_verbosity_level",
                tr("workspace.binding.a11y_set_verbose_announcements"),
                WorkspaceAction::SetA11yVerbosityLevel(AccessibilityVerbosity::Verbose),
            )
            .with_context_predicate(id!("Workspace"))
            .with_key_binding("cmdorctrl-alt-v"),
        ]);
    }

    app.register_editable_bindings([EditableBinding::new(
        "workspace:rename_active_tab",
        tr("workspace.binding.rename_current_tab"),
        WorkspaceAction::RenameActiveTab,
    )
    .with_group(bindings::BindingGroup::Settings.as_str())
    .with_custom_action(CustomAction::RenameTab)
    .with_context_predicate(id!("Workspace"))]);

    // Pane rename — same shape as RenameActiveTab but acts on the focused pane
    // in the active tab. Ships with no default keybinding so it surfaces in
    // Settings → Keyboard shortcuts as remappable; resolves issue #9351, where
    // the action existed only in the right-click context menu and was not
    // reachable via the binding registry.
    app.register_editable_bindings([EditableBinding::new(
        "workspace:rename_active_pane",
        tr("workspace.binding.rename_current_pane"),
        WorkspaceAction::RenameActivePane,
    )
    .with_group(bindings::BindingGroup::Settings.as_str())
    .with_context_predicate(id!("Workspace"))]);

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:terminate_app",
            tr("workspace.binding.quit_warp"),
            WorkspaceAction::TerminateApp,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Close.as_str())
        .with_enabled(|| ContextFlag::CloseWindow.is_enabled()),
        EditableBinding::new(
            "workspace:close_window",
            BindingDescription::new(tr("workspace.binding.close_window")).with_custom_description(
                bindings::MAC_MENUS_CONTEXT,
                tr("workspace.binding.close_window"),
            ),
            WorkspaceAction::CloseWindow,
        )
        .with_mac_key_binding("cmd-shift-W")
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Close.as_str())
        .with_custom_action(CustomAction::CloseWindow)
        .with_enabled(|| ContextFlag::CloseWindow.is_enabled()),
        EditableBinding::new(
            "workspace:close_active_tab",
            tr("workspace.binding.close_current_tab"),
            WorkspaceAction::CloseActiveTab,
        )
        .with_custom_action(CustomAction::CloseTab)
        .with_group(bindings::BindingGroup::Close.as_str())
        .with_context_predicate(
            id!("Workspace") & (id!("Workspace_CloseWindow") | id!("Workspace_MultipleTabs")),
        ),
        EditableBinding::new(
            "workspace:close_other_tabs",
            tr("workspace.binding.close_other_tabs"),
            WorkspaceAction::CloseNonActiveTabs,
        )
        .with_custom_action(CustomAction::CloseOtherTabs)
        .with_group(bindings::BindingGroup::Close.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:close_tabs_right_active_tab",
            BindingDescription::new(tr("workspace.binding.close_tabs_to_right"))
                .with_dynamic_override(|ctx| {
                    uses_vertical_tabs(ctx).then(|| tr("workspace.binding.close_tabs_below"))
                }),
            WorkspaceAction::CloseTabsRightActiveTab,
        )
        .with_group(bindings::BindingGroup::Close.as_str())
        .with_custom_action(CustomAction::CloseTabsRight)
        .with_context_predicate(id!("Workspace")),
        // We have two actions depending on the current state
        // (i.e. whether notifications are already on or off).
        EditableBinding::new(
            "workspace:toggle_notifications_on",
            tr("workspace.binding.turn_notifications_on"),
            WorkspaceAction::ToggleNotifications,
        )
        .with_group(bindings::BindingGroup::Notifications.as_str())
        .with_context_predicate(id!("Workspace") & !id!("Notifications_Enabled")),
        EditableBinding::new(
            "workspace:toggle_notifications_off",
            tr("workspace.binding.turn_notifications_off"),
            WorkspaceAction::ToggleNotifications,
        )
        .with_group(bindings::BindingGroup::Notifications.as_str())
        .with_context_predicate(id!("Workspace") & id!("Notifications_Enabled")),
        EditableBinding::new(
            "workspace:toggle_navigation_palette",
            BindingDescription::new(tr("workspace.binding.toggle_navigation_palette"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.navigation_palette"),
                ),
            WorkspaceAction::TogglePalette {
                mode: PaletteMode::Navigation,
                source: PaletteSource::Keybinding,
            },
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::NavigationPalette),
        EditableBinding::new(
            "workspace:toggle_launch_config_palette",
            tr("workspace.binding.launch_configuration_palette"),
            WorkspaceAction::TogglePalette {
                mode: PaletteMode::LaunchConfig,
                source: PaletteSource::Keybinding,
            },
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::LaunchConfigPalette)
        .with_enabled(|| ContextFlag::LaunchConfigurations.is_enabled()),
        EditableBinding::new(
            "workspace:toggle_files_palette",
            tr("workspace.binding.toggle_files_palette"),
            WorkspaceAction::TogglePalette {
                mode: PaletteMode::Files,
                source: PaletteSource::Keybinding,
            },
        )
        .with_context_predicate(id!("Workspace") & !id!("Workspace_ViewOnlySharedSession"))
        .with_custom_action(CustomAction::FilesPalette),
        EditableBinding::new(
            "workspace:open_launch_config_save_modal",
            tr("workspace.binding.save_new_launch_configuration"),
            WorkspaceAction::OpenLaunchConfigSaveModal,
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::SaveCurrentConfig)
        .with_enabled(|| ContextFlag::LaunchConfigurations.is_enabled()),
        EditableBinding::new(
            // If you rename this name, please update the name in command_palette/action/data_source.rs
            "workspace:search_drive",
            tr("workspace.binding.search_warp_drive"),
            WorkspaceAction::OpenPalette {
                mode: PaletteMode::WarpDrive,
                source: PaletteSource::Keybinding,
                query: None,
            },
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::SearchDrive),
    ]);

    if FeatureFlag::Autoupdate.is_enabled() {
        app.register_editable_bindings([
            EditableBinding::new(
                "workspace:update_and_relaunch",
                tr("workspace.binding.install_update_and_relaunch"),
                // TODO(vorporeal): I wonder if we should change wording here?
                WorkspaceAction::ApplyUpdate,
            )
            .with_group(bindings::BindingGroup::AutoUpdate.as_str())
            .with_context_predicate(id!("Workspace") & id!("AutoupdateState_UpdateReady"))
            .with_enabled(|| ContextFlag::PromptForVersionUpdates.is_enabled()),
            EditableBinding::new(
                "workspace:check_for_updates",
                tr("workspace.binding.check_for_updates"),
                WorkspaceAction::CheckForUpdate,
            )
            .with_group(bindings::BindingGroup::AutoUpdate.as_str())
            .with_context_predicate(id!("Workspace") & !id!("AutoupdateState_UpdateReady"))
            .with_enabled(|| ContextFlag::PromptForVersionUpdates.is_enabled()),
        ]);
    }

    app.register_editable_bindings([EditableBinding::new(
        "workspace:log_out",
        tr("workspace.binding.log_out"),
        WorkspaceAction::LogOut,
    )
    .with_group(bindings::BindingGroup::Settings.as_str())
    .with_context_predicate(id!("Workspace") & !id!("IsAnonymousUser"))]);

    if !FeatureFlag::AvatarInTabBar.is_enabled() {
        app.register_editable_bindings([EditableBinding::new(
            "workspace:toggle_resource_center",
            tr("workspace.binding.toggle_resource_center"),
            WorkspaceAction::ToggleResourceCenter,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ToggleResourceCenter)]);
    }

    if cfg!(not(target_family = "wasm")) {
        app.register_editable_bindings([EditableBinding::new(
            "workspace:export_all_warp_drive_objects",
            tr("workspace.binding.export_all_warp_drive_objects"),
            WorkspaceAction::ExportAllWarpDriveObjects,
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE))]);
    }

    // CLI install/uninstall actions (macOS only)
    #[cfg(target_os = "macos")]
    {
        app.register_editable_bindings([
            EditableBinding::new(
                "workspace:install_cli",
                tr("workspace.binding.install_oz_cli_command"),
                WorkspaceAction::InstallCLI,
            )
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_context_predicate(id!("Workspace")),
            EditableBinding::new(
                "workspace:uninstall_cli",
                tr("workspace.binding.uninstall_oz_cli_command"),
                WorkspaceAction::UninstallCLI,
            )
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_context_predicate(id!("Workspace")),
        ]);
    }

    if FeatureFlag::Changelog.is_enabled() {
        app.register_editable_bindings([
            // Always show the "View latest changelog" action in the command palette,
            // but without a keybinding when the update toast is not visible.
            EditableBinding::new(
                "workspace:view_changelog",
                tr("workspace.binding.view_latest_changelog"),
                WorkspaceAction::ViewLatestChangelog,
            )
            .with_context_predicate(id!("Workspace") & !id!("UpdateToastVisible"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            // Note that while the changelog resides in WarpEssentials, we should gate access to
            // the changelog based on whether WarpEssentials is an available view.
            .with_enabled(|| ContextFlag::WarpEssentials.is_enabled()),
            // When the update toast is visible, register the keybinding as well.
            EditableBinding::new(
                "workspace:view_changelog",
                tr("workspace.binding.view_latest_changelog"),
                WorkspaceAction::ViewLatestChangelog,
            )
            .with_context_predicate(id!("Workspace") & id!("UpdateToastVisible"))
            .with_group(bindings::BindingGroup::Settings.as_str())
            .with_custom_action(CustomAction::ViewChangelog)
            .with_linux_or_windows_key_binding(format!("alt-{}", cmd_or_ctrl_shift("o")))
            .with_enabled(|| ContextFlag::WarpEssentials.is_enabled()),
        ]);
    }

    // We use the same binding name for the AI Assistant and block list AI to preserve custom
    // keybindings between them.
    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:toggle_ai_assistant",
            BindingDescription::new(tr("workspace.binding.new_agent_pane"))
                .with_dynamic_override(|_| Some(new_agent_pane_label())),
            WorkspaceAction::NewPaneInAgentMode {
                entrypoint: AgentModeEntrypoint::NewPaneBinding,
                zero_state_prompt_suggestion_type: None,
            },
        )
        .with_enabled(|| FeatureFlag::AgentMode.is_enabled())
        .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_custom_action(CustomAction::NewAgentModePane),
        EditableBinding::new(
            "workspace:toggle_ai_assistant",
            tr("workspace.binding.toggle_warp_ai"),
            WorkspaceAction::ToggleAIAssistant,
        )
        .with_enabled(|| !FeatureFlag::AgentMode.is_enabled())
        .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        // We use the same custom action as AM so that we don't have
        // two mac menu items for AM vs Warp AI since they are mutually exclusive.
        .with_custom_action(CustomAction::NewAgentModePane),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:create_team_env_vars",
            BindingDescription::new(tr("workspace.binding.create_new_team_env_vars"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_team_env_vars"),
                ),
            WorkspaceAction::CreateTeamEnvVarCollection,
        )
        .with_custom_action(CustomAction::NewTeamEnvVars)
        .with_context_predicate(
            id!("Workspace")
                & id!(flags::ENABLE_WARP_DRIVE)
                & id!("WarpDrive_BelongsToTeam")
                & id!("IsOnline"),
        )
        .with_group(bindings::BindingGroup::EnvVarCollection.as_str()),
        EditableBinding::new(
            "workspace:create_personal_env_vars",
            BindingDescription::new(tr("workspace.binding.create_new_personal_env_vars"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_personal_env_vars"),
                ),
            WorkspaceAction::CreatePersonalEnvVarCollection,
        )
        .with_group(bindings::BindingGroup::EnvVarCollection.as_str())
        .with_custom_action(CustomAction::NewPersonalEnvVars)
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE)),
        EditableBinding::new(
            "workspace:create_personal_ai_prompt",
            BindingDescription::new(tr("workspace.binding.create_new_personal_prompt"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_personal_prompt"),
                ),
            WorkspaceAction::CreatePersonalAIPrompt,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_custom_action(CustomAction::NewPersonalAIPrompt)
        .with_context_predicate(
            id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE) & id!(flags::IS_ANY_AI_ENABLED),
        ),
        EditableBinding::new(
            "workspace:create_team_ai_prompt",
            BindingDescription::new(tr("workspace.binding.create_new_team_prompt"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.new_team_prompt"),
                ),
            WorkspaceAction::CreateTeamAIPrompt,
        )
        .with_group(bindings::BindingGroup::WarpAi.as_str())
        .with_custom_action(CustomAction::NewTeamAIPrompt)
        .with_context_predicate(
            id!("Workspace")
                & id!(flags::ENABLE_WARP_DRIVE)
                & id!("WarpDrive_BelongsToTeam")
                & id!("IsOnline")
                & id!(flags::IS_ANY_AI_ENABLED),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:shift_focus_left",
            tr("workspace.binding.switch_focus_to_left_panel"),
            WorkspaceAction::FocusLeftPanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_key_binding("cmdorctrl-shift-("),
        EditableBinding::new(
            "workspace:shift_focus_right",
            tr("workspace.binding.switch_focus_to_right_panel"),
            WorkspaceAction::FocusRightPanel,
        )
        .with_context_predicate(id!("Workspace"))
        .with_key_binding("cmdorctrl-shift-)"),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:import_to_personal_drive",
            tr("workspace.binding.import_to_personal_drive"),
            WorkspaceAction::ImportToPersonalDrive,
        )
        .with_context_predicate(id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE)),
        EditableBinding::new(
            "workspace:import_to_team_drive",
            tr("workspace.binding.import_to_team_drive"),
            WorkspaceAction::ImportToTeamDrive,
        )
        .with_context_predicate(
            id!("Workspace") & id!(flags::ENABLE_WARP_DRIVE) & id!("WarpDrive_BelongsToTeam"),
        ),
    ]);

    // Register a debug-only action for writing the user's access token to the system clipboard
    // to aid debugging and development.
    #[cfg(not(feature = "skip_login"))]
    if ChannelState::enable_debug_features() {
        app.register_editable_bindings([EditableBinding::new(
            "workspace:copy_access_token_to_clipboard",
            tr("workspace.binding.copy_access_token_to_clipboard"),
            WorkspaceAction::CopyAccessTokenToClipboard,
        )
        .with_context_predicate(id!("Workspace"))]);
    }

    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:open_repository",
            BindingDescription::new(tr("workspace.binding.open_repository"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.open_repository"),
                ),
            WorkspaceAction::OpenRepository { path: None },
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::OpenRepository)
        .with_group(bindings::BindingGroup::Folders.as_str()),
        EditableBinding::new(
            "workspace:open_ai_fact_collection",
            BindingDescription::new(tr("workspace.binding.open_ai_rules")).with_custom_description(
                bindings::MAC_MENUS_CONTEXT,
                tr("workspace.binding.open_ai_rules"),
            ),
            WorkspaceAction::OpenAIFactCollection,
        )
        .with_enabled(|| FeatureFlag::AIRules.is_enabled())
        .with_custom_action(CustomAction::OpenAIFactCollection)
        .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
        .with_group(bindings::BindingGroup::WarpAi.as_str()),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:open_mcp_servers",
        BindingDescription::new(tr("workspace.binding.open_mcp_servers")).with_custom_description(
            bindings::MAC_MENUS_CONTEXT,
            tr("workspace.binding.open_mcp_servers"),
        ),
        WorkspaceAction::OpenMCPServerCollection,
    )
    .with_enabled(|| {
        FeatureFlag::McpServer.is_enabled() && ContextFlag::ShowMCPServers.is_enabled()
    })
    .with_custom_action(CustomAction::OpenMCPServerCollection)
    .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
    .with_group(bindings::BindingGroup::WarpAi.as_str())]);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:jump_to_latest_toast",
        tr("workspace.binding.jump_to_latest_agent_task"),
        WorkspaceAction::JumpToLatestToast,
    )
    .with_enabled(|| FeatureFlag::AgentMode.is_enabled())
    .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
    .with_mac_key_binding("cmd-shift-G")
    .with_linux_or_windows_key_binding("ctrl-shift-G")
    .with_group(bindings::BindingGroup::WarpAi.as_str())]);

    app.register_editable_bindings([EditableBinding::new(
        TOGGLE_NOTIFICATION_MAILBOX_BINDING_NAME,
        tr("workspace.binding.toggle_notification_mailbox"),
        WorkspaceAction::ToggleNotificationMailbox { select_first: true },
    )
    .with_enabled(|| FeatureFlag::HOANotifications.is_enabled())
    .with_context_predicate(id!("Workspace"))
    .with_mac_key_binding("cmd-shift-U")
    .with_linux_or_windows_key_binding("ctrl-shift-U")
    .with_group(bindings::BindingGroup::WarpAi.as_str())]);

    add_open_setting_pages_as_editable_binding(app);
    add_overflow_menu_items_as_editable_binding(app);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:toggle_agent_management_view",
        tr("workspace.binding.toggle_agent_management_view"),
        WorkspaceAction::ToggleAgentManagementView,
    )
    .with_enabled(|| FeatureFlag::AgentManagementView.is_enabled())
    .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))
    .with_mac_key_binding("cmd-shift-M")
    .with_linux_or_windows_key_binding("ctrl-shift-M")
    .with_group(bindings::BindingGroup::WarpAi.as_str())]);
}

fn add_open_setting_pages_as_editable_binding(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    // Add the ability to open setting modals to the command palette.
    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:show_settings",
            BindingDescription::new(tr("workspace.binding.open_settings")).with_custom_description(
                bindings::MAC_MENUS_CONTEXT,
                tr("workspace.binding.settings"),
            ),
            WorkspaceAction::ShowSettings,
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_custom_action(CustomAction::ShowSettings),
        EditableBinding::new(
            "workspace:show_settings_account_page",
            tr("workspace.binding.open_settings_account"),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Account),
        )
        .with_context_predicate(id!("Workspace"))
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_custom_action(CustomAction::ShowAccount),
        EditableBinding::new(
            "workspace:show_settings_appearance_page",
            BindingDescription::new(tr("workspace.binding.open_settings_appearance"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.appearance_ellipsis"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Appearance),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ShowAppearance),
        EditableBinding::new(
            "workspace:show_settings_features_page",
            tr("workspace.binding.open_settings_features"),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Features),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_shared_blocks_page",
            BindingDescription::new(tr("workspace.binding.open_settings_shared_blocks"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.view_shared_blocks_ellipsis"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::SharedBlocks),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ViewSharedBlocks),
        EditableBinding::new(
            "workspace:show_settings_keyboard_shortcuts_page",
            BindingDescription::new(tr("workspace.binding.open_settings_keyboard_shortcuts"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.configure_keyboard_shortcuts_ellipsis"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Keybindings),
        )
        .with_group(bindings::BindingGroup::KeyboardShortcuts.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ConfigureKeybindings),
        EditableBinding::new(
            "workspace:show_settings_about_page",
            BindingDescription::new(tr("workspace.binding.open_settings_about"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.about_warp"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::About),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ShowAboutWarp),
        EditableBinding::new(
            "workspace:show_settings_teams_page",
            BindingDescription::new(tr("workspace.binding.open_settings_teams"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.open_team_settings"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Teams),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_custom_action(CustomAction::OpenTeamSettings)
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_privacy_page",
            BindingDescription::new(tr("workspace.binding.open_settings_privacy")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Privacy),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_warpify_page",
            BindingDescription::new(tr("workspace.binding.open_settings_warpify"))
                .with_custom_description(
                    bindings::MAC_MENUS_CONTEXT,
                    tr("workspace.binding.configure_warpify_ellipsis"),
                ),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Warpify),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_ai_settings_page",
            BindingDescription::new(tr("workspace.binding.open_settings_ai")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::WarpAgent),
        )
        .with_enabled(|| FeatureFlag::AgentMode.is_enabled())
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_billing_and_usage_page",
            BindingDescription::new(tr("workspace.binding.open_settings_billing_and_usage")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::BillingAndUsage),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_code_page",
            BindingDescription::new(tr("workspace.binding.open_settings_code")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::CodeIndexing),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_referrals_page",
            BindingDescription::new(tr("workspace.binding.open_settings_referrals")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::Referrals),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_settings_environments_page",
            BindingDescription::new(tr("workspace.binding.open_settings_environments")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::CloudEnvironments),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:show_mcp_servers_settings_page",
            BindingDescription::new(tr("workspace.binding.open_settings_mcp_servers")),
            WorkspaceAction::ShowSettingsPage(SettingsSection::MCPServers),
        )
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:open_settings_file",
            tr("workspace.binding.open_settings_file"),
            WorkspaceAction::OpenSettingsFile,
        )
        .with_enabled(|| FeatureFlag::SettingsFile.is_enabled() && cfg!(feature = "local_fs"))
        .with_group(bindings::BindingGroup::Settings.as_str())
        .with_context_predicate(id!("Workspace")),
    ]);
}

fn add_overflow_menu_items_as_editable_binding(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    // Add the ability to open all overflow menu items to the command palette.
    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:show_invite_modal",
            tr("workspace.binding.invite_people_ellipsis"),
            WorkspaceAction::ShowReferralSettingsPage,
        )
        .with_context_predicate(id!("Workspace"))
        .with_custom_action(CustomAction::ReferAFriend),
        EditableBinding::new(
            "workspace:link_to_slack",
            tr("workspace.binding.join_slack_external"),
            WorkspaceAction::JoinSlack,
        )
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:link_to_user_docs",
            tr("workspace.binding.view_user_docs_external"),
            WorkspaceAction::ViewUserDocs,
        )
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:send_feedback",
            BindingDescription::new(tr("workspace.binding.send_feedback_external")),
            WorkspaceAction::SendFeedback,
        )
        .with_context_predicate(id!("Workspace")),
        #[cfg(not(target_family = "wasm"))]
        EditableBinding::new(
            "workspace:view_logs",
            tr("workspace.binding.view_warp_logs"),
            WorkspaceAction::ViewLogs,
        )
        .with_context_predicate(id!("Workspace")),
        EditableBinding::new(
            "workspace:link_to_privacy_policy",
            tr("workspace.binding.view_privacy_policy_external"),
            WorkspaceAction::ViewPrivacyPolicy,
        )
        .with_context_predicate(id!("Workspace")),
    ]);
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub struct TabBarDropTargetData {
    pub tab_bar_location: TabBarLocation,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub struct VerticalTabsPaneDropTargetData {
    pub tab_bar_location: TabBarLocation,
    pub tab_hover_index: TabBarHoverIndex,
}

#[derive(PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum TabBarLocation {
    TabIndex(usize),
    AfterTabIndex(usize), // Indicates any area after the tabs in the tab bar, includes the total tab count
}

impl DropTargetData for TabBarDropTargetData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl DropTargetData for VerticalTabsPaneDropTargetData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
