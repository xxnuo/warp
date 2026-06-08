use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

define_settings_group!(I18nSettings, settings: [
    locale: UiLocale {
        type: String,
        default: warp_i18n::AUTO_LOCALE.to_owned(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "ui.locale",
        description: "The UI locale. Use auto to follow the system locale.",
    },
]);
