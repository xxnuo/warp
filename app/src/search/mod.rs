pub mod action;
pub mod ai_context_menu;
mod ai_queries;
pub(crate) mod async_snapshot_data_source;
pub mod binding_source;
pub mod command_palette;
pub mod command_search;
mod env_var_collections;
pub mod external_secrets;
pub mod files;
mod filter_chip_renderer;
pub mod notebook_embedding;
mod notebooks;
mod palette_styles;
mod search_bar;
pub mod search_results_menu;
pub mod slash_command_menu;
mod workflows;

pub use data_source::QueryFilter;
use filter_chip_renderer::FilterChipRenderer;
pub use item::SearchItem;
pub use mixer::SyncDataSource;
pub use result_renderer::ItemHighlightState;
// Re-export core search types.
use warp_i18n::tr;
pub use warp_search_core::*;
pub use workflows::fuzzy_match::FuzzyMatchWorkflowResult;

pub fn query_filter_display_name(filter: QueryFilter) -> String {
    match filter {
        QueryFilter::History => tr("search.filter.history"),
        QueryFilter::NaturalLanguage => tr("search.filter.natural_language"),
        QueryFilter::Actions => tr("search.filter.actions"),
        QueryFilter::Sessions => tr("search.filter.sessions"),
        QueryFilter::Tabs => tr("search.filter.tabs"),
        QueryFilter::Drive => tr("search.filter.drive"),
        QueryFilter::LaunchConfigurations => tr("search.filter.launch_configurations"),
        QueryFilter::PromptHistory => tr("search.filter.prompt_history"),
        QueryFilter::Files => tr("search.filter.files"),
        QueryFilter::Commands => tr("search.filter.commands"),
        QueryFilter::Blocks => tr("search.filter.blocks"),
        QueryFilter::Code => tr("search.filter.code"),
        QueryFilter::Rules => tr("search.filter.rules"),
        QueryFilter::Repos => tr("search.filter.repos"),
        QueryFilter::DiffSets => tr("search.filter.diff_sets"),
        QueryFilter::StaticSlashCommands => tr("search.filter.static_slash_commands"),
        QueryFilter::Skills => tr("search.filter.skills"),
        QueryFilter::BaseModels => tr("search.filter.base_models"),
        QueryFilter::FullTerminalUseModels => tr("search.filter.full_terminal_use_models"),
        QueryFilter::CurrentDirectoryConversations => {
            tr("search.filter.current_directory_conversations")
        }
        QueryFilter::Conversations => tr("search.filter.conversations"),
        QueryFilter::Workflows => tr("search.filter.workflows"),
        QueryFilter::Notebooks => tr("search.filter.notebooks"),
        QueryFilter::Plans => tr("search.filter.plans"),
        QueryFilter::EnvironmentVariables => tr("search.filter.environment_variables"),
        QueryFilter::AgentModeWorkflows => tr("search.filter.agent_mode_workflows"),
    }
}

pub fn query_filter_placeholder_text(filter: QueryFilter) -> String {
    match filter {
        QueryFilter::History => tr("search.filter.placeholder.history"),
        QueryFilter::NaturalLanguage => tr("search.filter.placeholder.natural_language"),
        QueryFilter::Actions => tr("search.filter.placeholder.actions"),
        QueryFilter::Sessions => tr("search.filter.placeholder.sessions"),
        QueryFilter::Tabs => tr("search.filter.placeholder.tabs"),
        QueryFilter::Drive => tr("search.filter.placeholder.drive"),
        QueryFilter::LaunchConfigurations => tr("search.filter.placeholder.launch_configurations"),
        QueryFilter::PromptHistory => tr("search.filter.placeholder.prompt_history"),
        QueryFilter::Files => tr("search.filter.placeholder.files"),
        QueryFilter::Commands => tr("search.filter.placeholder.commands"),
        QueryFilter::Blocks => tr("search.filter.placeholder.blocks"),
        QueryFilter::Code => tr("search.filter.placeholder.code"),
        QueryFilter::Rules => tr("search.filter.placeholder.rules"),
        QueryFilter::Repos => tr("search.filter.placeholder.repos"),
        QueryFilter::DiffSets => tr("search.filter.placeholder.diff_sets"),
        QueryFilter::StaticSlashCommands => tr("search.filter.placeholder.static_slash_commands"),
        QueryFilter::Skills => tr("search.filter.placeholder.skills"),
        QueryFilter::BaseModels => tr("search.filter.placeholder.base_models"),
        QueryFilter::FullTerminalUseModels => {
            tr("search.filter.placeholder.full_terminal_use_models")
        }
        QueryFilter::CurrentDirectoryConversations => {
            tr("search.filter.placeholder.current_directory_conversations")
        }
        QueryFilter::Conversations => tr("search.filter.placeholder.conversations"),
        QueryFilter::Workflows => tr("search.filter.placeholder.workflows"),
        QueryFilter::Notebooks => tr("search.filter.placeholder.notebooks"),
        QueryFilter::Plans => tr("search.filter.placeholder.plans"),
        QueryFilter::EnvironmentVariables => tr("search.filter.placeholder.environment_variables"),
        QueryFilter::AgentModeWorkflows => tr("search.filter.placeholder.agent_mode_workflows"),
    }
}
