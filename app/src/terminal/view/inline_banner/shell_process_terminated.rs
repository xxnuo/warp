use warp_i18n::tr;
use warpui::elements::Text;
use warpui::Element;

use super::{
    render_inline_block_list_banner, InlineBannerContent, InlineBannerIcon, InlineBannerStyle,
};
use crate::appearance::Appearance;

pub fn render_shell_process_terminated_banner(
    appearance: &Appearance,
    was_premature_termination: bool,
) -> Box<dyn Element> {
    if was_premature_termination {
        render_inline_block_list_banner(
            InlineBannerStyle::CallToAction,
            appearance,
            InlineBannerContent {
                title: tr("terminal.inline_banner.shell_process.exited_prematurely"),
                header_icon: Some(InlineBannerIcon {
                    asset_path: "bundled/svg/warning.svg",
                    aspect_ratio: 1.,
                    color_override: Some(appearance.theme().foreground().into_solid()),
                }),
                content: Some(vec![Text::new(
                    tr("terminal.inline_banner.shell_process.initialization_output_visible"),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )]),
                ..Default::default()
            },
        )
    } else {
        render_inline_block_list_banner(
            InlineBannerStyle::LowPriority,
            appearance,
            InlineBannerContent {
                title: tr("terminal.inline_banner.shell_process.exited"),
                header_icon: Some(InlineBannerIcon {
                    asset_path: "bundled/svg/info.svg",
                    aspect_ratio: 1.,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
    }
}
