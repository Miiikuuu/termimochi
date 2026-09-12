//! Declarative window chrome; never accepts executable tab-bar scripts.
use serde::{Deserialize, Serialize};
use termimochi_core::Rgb;
#[cfg(test)]
#[path = "window_top_tests.rs"]
mod tests;

pub(crate) fn hex(value: [u8; 3]) -> String {
    Rgb::new(value[0], value[1], value[2]).to_hex()
}
pub(crate) fn color(value: &str) -> Result<[u8; 3], String> {
    let c: Rgb = value
        .parse()
        .map_err(|_| "Use a hexadecimal RGB color".to_owned())?;
    Ok([c.red(), c.green(), c.blue()])
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TitlebarColor {
    #[default]
    System,
    Background,
    Custom([u8; 3]),
}
impl TitlebarColor {
    pub fn native(self) -> String {
        match self {
            Self::System => "system".into(),
            Self::Background => "background".into(),
            Self::Custom(c) => hex(c),
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "background" => Some(Self::Background),
            _ => color(value).ok().map(Self::Custom),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TabStyle {
    #[default]
    Fade,
    Slant,
    Separator,
    Powerline,
}
impl TabStyle {
    pub const ALL: [Self; 4] = [Self::Fade, Self::Slant, Self::Separator, Self::Powerline];
    pub fn native(self) -> &'static str {
        match self {
            Self::Fade => "fade",
            Self::Slant => "slant",
            Self::Separator => "separator",
            Self::Powerline => "powerline",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.native() == value)
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TabEdge {
    Top,
    #[default]
    Bottom,
}
impl TabEdge {
    pub fn native(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

pub(crate) fn titlebar_supported_in(wayland: bool, desktop: &str) -> bool {
    wayland && desktop.split(':').any(|p| p.eq_ignore_ascii_case("gnome"))
}
pub(crate) fn titlebar_supported() -> bool {
    titlebar_supported_in(
        std::env::var_os("WAYLAND_DISPLAY").is_some_and(|s| !s.is_empty()),
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
    )
}

pub(crate) const fn default_min_tabs() -> u8 {
    2
}
pub(crate) const fn active_fg() -> [u8; 3] {
    [0, 0, 0]
}
pub(crate) const fn active_bg() -> [u8; 3] {
    [238, 238, 238]
}
pub(crate) const fn inactive_fg() -> [u8; 3] {
    [68, 68, 68]
}
pub(crate) const fn inactive_bg() -> [u8; 3] {
    [153, 153, 153]
}

pub(crate) fn kitty_field(field: &str) -> Option<&'static str> {
    Some(match field {
        "titlebar_color" => "wayland_titlebar_color",
        "tab_style" | "tab_bar" => "tab_bar_style",
        "tab_edge" => "tab_bar_edge",
        "tab_min_tabs" => "tab_bar_min_tabs",
        "tab_active_fg" => "active_tab_foreground",
        "tab_active_bg" => "active_tab_background",
        "tab_inactive_fg" => "inactive_tab_foreground",
        "tab_inactive_bg" => "inactive_tab_background",
        _ => return None,
    })
}

pub(crate) fn properties(l: &super::LayoutSettings) -> std::collections::BTreeMap<String, String> {
    [
        ("wayland_titlebar_color", l.titlebar_color.native()),
        (
            "tab_bar_style",
            if l.tab_bar {
                l.tab_style.native()
            } else {
                "hidden"
            }
            .into(),
        ),
        ("tab_bar_edge", l.tab_edge.native().into()),
        ("tab_bar_min_tabs", l.tab_min_tabs.to_string()),
        ("active_tab_foreground", hex(l.tab_active_fg)),
        ("active_tab_background", hex(l.tab_active_bg)),
        ("inactive_tab_foreground", hex(l.tab_inactive_fg)),
        ("inactive_tab_background", hex(l.tab_inactive_bg)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// Kitty chrome inherits Kitty defaults, not the reference terminal's UI.
/// This only initializes a disposable preview; it does not claim theme fields.
pub(crate) fn reset_reference(l: &mut super::LayoutSettings) {
    l.tab_bar = true;
    l.titlebar_color = TitlebarColor::System;
    l.tab_style = TabStyle::Fade;
    l.tab_edge = TabEdge::Bottom;
    l.tab_min_tabs = default_min_tabs();
    l.tab_active_fg = active_fg();
    l.tab_active_bg = active_bg();
    l.tab_inactive_fg = inactive_fg();
    l.tab_inactive_bg = inactive_bg();
}
