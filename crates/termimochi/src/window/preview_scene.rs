//! Observation is session-only view state, not an editor module or saved design.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreviewScene {
    Terminal,
    Prompt,
    Greeting,
}

impl PreviewScene {
    fn index(self) -> u32 {
        match self {
            Self::Terminal => 0,
            Self::Prompt => 1,
            Self::Greeting => 2,
        }
    }
    fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Prompt,
            2 => Self::Greeting,
            _ => Self::Terminal,
        }
    }
}
impl Workbench {
    fn preview_scene(&self) -> PreviewScene {
        if self.greeting_preview.get() {
            PreviewScene::Greeting
        } else if self.preview_uses_prompt.get() {
            PreviewScene::Prompt
        } else {
            PreviewScene::Terminal
        }
    }
    pub(super) fn observes_prompt(&self) -> bool {
        self.greeting_preview.get() || self.preview_uses_prompt.get()
    }
    pub(super) fn sync_preview_scene(&self) {
        let navigating = self.navigating_preview.replace(true);
        self.preview_scene_selector
            .set_selected(self.preview_scene().index());
        self.navigating_preview.set(navigating);
    }
    pub(super) fn connect_preview_scene(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.preview_scene_selector
            .connect_selected_notify(move |selector| {
                if let Some(this) = weak.upgrade()
                    && !this.navigating_preview.get()
                {
                    this.select_preview_scene(PreviewScene::from_index(selector.selected()));
                }
            });
    }
    fn select_preview_scene(self: &Rc<Self>, scene: PreviewScene) {
        if self.preview_scene() == scene {
            return;
        }
        match scene {
            PreviewScene::Greeting => self.show_greeting_preview(),
            PreviewScene::Terminal => {
                self.reset_prompt_preview();
                self.refresh_preview();
            }
            PreviewScene::Prompt => {
                let navigating = self.navigating_preview.replace(true);
                self.reset_prompt_preview();
                self.redraw_preview_contents();
                // Inspect may select a different editor page without changing
                // the design's prompt source. Scene navigation must not save it.
                let source = self.preview_prompt_source.get();
                if source == 0 {
                    self.ensure_starship_copy();
                }
                self.activate_prompt_preview(source);
                self.navigating_preview.set(navigating);
                if source == 0 {
                    self.schedule_copy_preview();
                }
                self.redraw_preview_contents();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::greeting::tests::{controller, descendants, settle};

    #[test]
    #[ignore = "isolated GTK: editor navigation preserves independent terminal, prompt and GIF scenes"]
    fn independent_preview_scenes_survive_editor_navigation() {
        assert_eq!(std::env::var("GSETTINGS_BACKEND").as_deref(), Ok("memory"));
        adw::init().unwrap();
        gio::resources_register_include!("termimochi.gresource").unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.miiikuuu.termimochi.PreviewSceneTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let root = tempfile::tempdir().unwrap();
        present_with_preset(&app, None, root.path().join(typography_preset::PRESET_NAME));
        let window = app.active_window().unwrap();
        let this = controller(&window);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while this.preview_loading.get() {
            assert!(std::time::Instant::now() < deadline);
            settle();
        }
        this.preview_scene_selector.set_selected(0);
        this.prompt_module_button.set_active(true);
        this.prompt_source_selector.set_selected(1);
        assert_eq!(this.preview_scene(), PreviewScene::Terminal);
        this.greeting_module_button.set_active(true);
        let (mut settings, _) = crate::pixel_trial::tests::fixture(true);
        settings.presentation.visual = crate::greeting_output::Visual::Animation;
        this.greeting.replace(settings, true);
        settle();
        assert_eq!(this.preview_scene(), PreviewScene::Terminal);
        this.save_workspace_path(
            root.path().join("scene.termimochi.json"),
            this.workspace_snapshot(),
        )
        .unwrap();
        let saved = this.workspace_snapshot();
        let trial = descendants(&this.output_bar.root.clone().upcast())
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.action_name().as_deref() == Some("win.try-greeting"))
            .unwrap();
        assert_eq!(trial.label().as_deref(), Some("Try Greeting"));
        assert!(
            trial
                .tooltip_text()
                .unwrap()
                .contains("not the whole scheme")
        );
        let stack = this
            .inspect_layer
            .child()
            .and_downcast::<gtk::Stack>()
            .unwrap();
        for scene in [
            PreviewScene::Greeting,
            PreviewScene::Prompt,
            PreviewScene::Terminal,
        ] {
            this.preview_scene_selector.set_selected(scene.index());
            settle();
            for module in [
                &this.palette_module_button,
                &this.typography_module_button,
                &this.layout_module_button,
                &this.prompt_module_button,
                &this.greeting_module_button,
            ] {
                let feed = crate::window::greeting::tests::feed(&this);
                module.set_active(true);
                settle();
                assert_eq!(this.preview_scene(), scene);
                assert_eq!(this.preview_scene_selector.selected(), scene.index());
                assert_eq!(crate::window::greeting::tests::feed(&this), feed);
                assert_eq!(
                    stack.visible_child_name().as_deref(),
                    Some(if scene == PreviewScene::Greeting {
                        "pixels"
                    } else {
                        "terminal"
                    })
                );
                assert!(trial.is_mapped());
                assert_eq!(this.workspace_snapshot(), saved);
                assert!(this.workspace_is_clean());
            }
        }
        this.preview_scene_selector.set_selected(2);
        this.inspect_preview_target(PreviewTarget::Typography);
        assert_eq!(this.preview_scene(), PreviewScene::Greeting);
        this.palette_module_button.set_active(true);
        this.refresh_preview();
        settle();
        assert_eq!(stack.visible_child_name().as_deref(), Some("pixels"));
        this.inspect_preview_target(PreviewTarget::PromptCopy);
        this.preview_scene_selector.set_selected(0);
        this.preview_scene_selector.set_selected(1);
        assert_eq!(
            this.workspace_snapshot(),
            saved,
            "Inspect followed by a scene change must not modify the saved prompt source"
        );
        this.preview_scene_selector.set_selected(2);
        for (width, height) in [(1024, 700), (1280, 900)] {
            window.set_default_size(width, height);
            settle();
            for widget in [
                this.preview_scene_selector.clone().upcast::<gtk::Widget>(),
                trial.clone().upcast(),
            ] {
                assert!(widget.is_mapped());
                let bounds = widget.compute_bounds(&window).unwrap();
                assert!(bounds.x() >= 0.0 && bounds.y() >= 0.0);
                assert!(bounds.x() + bounds.width() <= window.width() as f32);
                assert!(bounds.y() + bounds.height() <= window.height() as f32);
            }
        }
        window.destroy();
    }
}
