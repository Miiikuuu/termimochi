use super::*;
use crate::{
    design_document::{self, TargetHint},
    kitty_document::KittyDocument,
    layout::LayoutSettings,
    workspace::Workspace,
};

fn workspace() -> Workspace {
    Workspace::new(
        &termimochi_core::PtyxisPalette::from_text(include_str!(
            "../../resources/themes/fog-paper.palette"
        ))
        .unwrap(),
        termimochi_core::Variant::Light,
        Default::default(),
        Default::default(),
        Default::default(),
        None,
        true,
    )
}

#[test]
fn old_layouts_receive_backward_compatible_chrome_without_changing_visibility() {
    let old = r#"{"content_padding":5,"columns":80,"rows":24,"cursor_shape":"block","cursor_blink":"system","tab_bar":false,"scrollbar":false,"window_spacing":18}"#;
    let l: LayoutSettings = serde_json::from_str(old).unwrap();
    l.validate().unwrap();
    assert!(!l.tab_bar);
    assert_eq!(l.tab_min_tabs, 2);
    assert_eq!(l.tab_style, TabStyle::Fade);
    assert_eq!(l.titlebar_color, TitlebarColor::System);
    let round: LayoutSettings = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert_eq!(l, round);
}

#[test]
fn chrome_native_import_roundtrip_is_safe_and_preserves_unsupported_source() {
    let source = "background #112233\ntab_bar_style powerline\ntab_bar_edge top\ntab_bar_min_tabs 1\nactive_tab_foreground #abcdef\nactive_tab_background #123456\ninactive_tab_foreground #987654\ninactive_tab_background #abcdef\nwayland_titlebar_color background\nmap ctrl+x launch arbitrary-code\n";
    let d = KittyDocument::parse(source).unwrap();
    let w = d.project_workspace(&workspace()).unwrap();
    assert_eq!(w.layout.tab_style, TabStyle::Powerline);
    assert_eq!(w.layout.tab_edge, TabEdge::Top);
    assert_eq!(w.layout.tab_min_tabs, 1);
    assert_eq!(w.layout.titlebar_color, TitlebarColor::Background);
    assert_eq!(d.reconcile_with_reference(&w, &w).unwrap(), source);
    assert!(!d.safe_configuration().contains("arbitrary-code"));
    for (k, v) in properties(&w.layout) {
        assert_eq!(d.properties()[&k].to_lowercase(), v.to_lowercase());
    }
}

#[test]
fn color_only_theme_adds_only_edited_chrome_and_keeps_identity_after_save() {
    let mut reference = workspace();
    reference.layout.tab_bar = false;
    reference.layout.tab_active_bg = [1, 2, 3];
    let d = design_document::import(b"background #123456\nforeground #abcdef\n", &reference)
        .unwrap()
        .into_theme(TargetHint::Kitty, "Sparse Top")
        .unwrap();
    let opening = d.project_preview(&reference);
    assert!(opening.layout.tab_bar);
    assert_eq!(opening.layout.tab_active_bg, active_bg());
    let untouched = d.capture_theme(&opening, &opening).unwrap();
    let text = untouched.theme_kitty_configuration(&opening).unwrap();
    assert!(!text.contains("tab_") && !text.contains("titlebar") && !text.contains("font_size"));
    let mut edited = opening.clone();
    edited.layout.tab_edge = TabEdge::Top;
    let changed = d.capture_theme(&opening, &edited).unwrap();
    assert_eq!(changed.theme.as_ref().unwrap().layout.len(), 1);
    let text = changed.theme_kitty_configuration(&edited).unwrap();
    assert!(text.contains("tab_bar_edge top"));
    assert!(!text.contains("active_tab") && !text.contains("tab_bar_style"));
    let reopened =
        design_document::import(&serde_json::to_vec(&changed).unwrap(), &reference).unwrap();
    assert_eq!(reopened.id, d.id);
    assert_eq!(reopened.target_hint, Some(TargetHint::Kitty));
    assert_eq!(
        reopened.project_preview(&reference).layout.tab_edge,
        TabEdge::Top
    );
}

#[test]
fn unowned_reference_visibility_cannot_hide_an_explicit_kitty_style() {
    let mut reference = workspace();
    reference.layout.tab_bar = false;
    let d = design_document::DesignDocument::new_theme(TargetHint::Kitty, "Kitty", true);
    let opening = d.project_preview(&reference);
    assert!(opening.layout.tab_bar);
    let mut edited = opening.clone();
    edited.layout.tab_style = TabStyle::Powerline;
    let changed = d.capture_theme(&opening, &edited).unwrap();
    assert_eq!(changed.theme.as_ref().unwrap().layout.len(), 1);
    let native = changed.theme_kitty_configuration(&edited).unwrap();
    assert!(native.contains("tab_bar_style powerline"));
    assert!(!native.contains("active_tab_background"));
    let hidden = design_document::import(b"background #123456\ntab_bar_style hidden\n", &reference)
        .unwrap()
        .into_theme(TargetHint::Kitty, "Hidden")
        .unwrap();
    assert!(!hidden.project_preview(&reference).layout.tab_bar);
}

#[test]
fn style_and_visibility_are_independent_across_native_edits() {
    let doc = KittyDocument::parse("tab_bar_style slant\n").unwrap();
    let opening = doc.project_workspace(&workspace()).unwrap();
    let mut edited = opening.clone();
    edited.layout.tab_bar = false;
    assert!(
        doc.reconcile_with_reference(&edited, &opening)
            .unwrap()
            .contains("tab_bar_style hidden")
    );
    edited.layout.tab_bar = true;
    assert_eq!(
        doc.reconcile_with_reference(&edited, &opening).unwrap(),
        "tab_bar_style slant\n"
    );
}

#[test]
fn titlebar_capability_is_conservative_and_does_not_claim_x11_or_other_desktops() {
    assert!(titlebar_supported_in(true, "ubuntu:GNOME"));
    assert!(!titlebar_supported_in(false, "GNOME"));
    assert!(!titlebar_supported_in(true, "KDE"));
    assert!(!titlebar_supported_in(true, ""));
    for value in ["system", "background", "#aabbcc"] {
        assert!(TitlebarColor::parse(value).is_some());
    }
    for value in ["#gg0000", "background\ninclude bad", "$(touch bad)"] {
        assert!(TitlebarColor::parse(value).is_none());
    }
}

#[test]
fn invalid_chrome_is_not_in_the_kitty_safe_projection() {
    let source = "tab_bar_style custom\ntab_bar_min_tabs 999\ntab_bar_edge left\nactive_tab_background $(bad)\nwayland_titlebar_color #broken\n";
    let doc = KittyDocument::parse(source).unwrap();
    assert_eq!(doc.notices.len(), 5);
    assert!(doc.safe_configuration().is_empty());
    assert_eq!(doc.source(), source);
    let l = LayoutSettings {
        tab_min_tabs: 0,
        ..Default::default()
    };
    assert!(l.validate().is_err());
}
