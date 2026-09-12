//! A detached saved-settings import; the existing window is never mutated.
use super::*;
use crate::design_document::{DesignDocument, Kind};
#[cfg(test)]
#[path = "system_import_tests.rs"]
mod tests;

#[derive(Clone)]
pub(super) enum Source {
    Ptyxis(String),
    Kitty(PathBuf),
}
impl Workbench {
    pub(super) fn choose_system_theme(self: &Rc<Self>) {
        let dialog=gtk::AlertDialog::builder().message("Create a theme from my terminal")
            .detail("Read saved settings into a new theme window. This is not a capture of a running terminal. Your current theme and terminal settings remain unchanged.")
            .buttons(["Cancel","Ptyxis","Kitty"]).cancel_button(0).default_button(0).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            Some(&self.window()),
            gio::Cancellable::NONE,
            move |answer| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match answer {
                    Ok(1) => match crate::ptyxis::appearance_profiles() {
                        Ok(profiles) if profiles.len() == 1 => {
                            this.review_system_source(Source::Ptyxis(profiles[0].0.clone()))
                        }
                        Ok(profiles) => this.choose_system_candidate(
                            profiles
                                .into_iter()
                                .map(|(id, label)| (format!("{label} · {id}"), Source::Ptyxis(id)))
                                .collect(),
                        ),
                        Err(e) => this.system_import_error(&e),
                    },
                    Ok(2) => {
                        let paths = crate::system_import::kitty_candidates();
                        if paths.len() == 1 {
                            this.review_system_source(Source::Kitty(paths[0].clone()));
                        } else if paths.is_empty() {
                            this.choose_system_kitty_file();
                        } else {
                            this.choose_system_candidate(
                                paths
                                    .into_iter()
                                    .map(|p| (p.display().to_string(), Source::Kitty(p)))
                                    .collect(),
                            );
                        }
                    }
                    _ => {}
                }
            },
        );
    }
    fn system_import_error(&self, error: &str) {
        gtk::AlertDialog::builder()
            .message("Saved settings could not be read")
            .detail(error)
            .buttons(["Close"])
            .build()
            .show(Some(&self.window()));
    }
    fn choose_system_candidate(self: &Rc<Self>, sources: Vec<(String, Source)>) {
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "Choose saved configuration",
            "Not another window's active tab",
        );
        let labels: Vec<_> = sources.iter().map(|(label, _)| label.as_str()).collect();
        let select = gtk::DropDown::from_strings(&labels);
        body.append(&select);
        if sources.iter().any(|(_, s)| matches!(s, Source::Kitty(_))) {
            let choose = gtk::Button::with_label("Choose another Kitty file…");
            body.append(&choose);
            let weak = Rc::downgrade(self);
            let w = window.downgrade();
            choose.connect_clicked(move |_| {
                if let (Some(this), Some(w)) = (weak.upgrade(), w.upgrade()) {
                    w.destroy();
                    this.choose_system_kitty_file();
                }
            });
        }
        let next = gtk::Button::with_label("Review Snapshot");
        buttons.append(&next);
        let weak = Rc::downgrade(self);
        let w = window.downgrade();
        next.connect_clicked(move |_| {
            if let (Some(this), Some(w)) = (weak.upgrade(), w.upgrade()) {
                let source = sources[select.selected() as usize].1.clone();
                w.destroy();
                this.review_system_source(source);
            }
        });
        window.present();
    }
    fn choose_system_kitty_file(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Choose saved Kitty configuration")
            .accept_label("Review Snapshot")
            .build();
        let weak = Rc::downgrade(self);
        dialog.open(Some(&self.window()), gio::Cancellable::NONE, move |r| {
            if let Some(this) = weak.upgrade() {
                match r {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            this.review_system_source(Source::Kitty(path));
                        }
                    }
                    Err(e) if e.matches(gtk::DialogError::Dismissed) => {}
                    Err(e) => this.system_import_error(&e.to_string()),
                }
            }
        });
    }
    pub(super) fn review_system_source(self: &Rc<Self>, source: Source) {
        let (window, body, buttons) = super::scheme::dialog(
            &self.window(),
            "From My Terminal",
            "Saved configuration snapshot · new theme, never Apply",
        );
        window.set_default_size(620, 360);
        let status = super::scheme::label("Reading saved appearance…");
        body.append(&status);
        let create = gtk::Button::with_label("Create Theme");
        create.set_widget_name("system-import-create");
        create.set_sensitive(false);
        buttons.append(&create);
        let cancel = gtk::Button::with_label("Cancel");
        buttons.prepend(&cancel);
        let w = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(w) = w.upgrade() {
                w.destroy();
            }
        });
        let (tx, rx) = mpsc::channel();
        // One bounded job for this modal review; no continuous input queue.
        std::thread::spawn(move || {
            let _ = tx.send(match source {
                Source::Ptyxis(id) => crate::system_import::ptyxis(&id),
                Source::Kitty(path) => crate::system_import::kitty(&path),
            });
        });
        let weak = Rc::downgrade(self);
        let w = window.downgrade();
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let (Some(this), Some(window)) = (weak.upgrade(), w.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            match rx.try_recv() {
                Err(mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,
                Err(_)=>status.set_text("Import worker unavailable. Close this review and try a file or a blank theme."),
                Ok(Err(e))=>status.set_text(&format!("Not imported: {e}\nYou can still create a blank theme or open a different file.")),
                Ok(Ok(design))=>{
                    let report=&design.theme.as_ref().unwrap().import_report;
                    status.set_text(&format!("{} · saved snapshot · {} unread/unsupported notes\nUnspecified values stay inherited. No write or launch authorization is copied.",super::theme_workspace::target_label(design.target_hint.unwrap()),report.iter().filter(|r|matches!(r.status,crate::system_import::ReadStatus::Unparsed|crate::system_import::ReadStatus::Unrecognized)).count()));
                    let details=gtk::Expander::builder().label("Sources, inherited values and limits").child(&super::scheme::label(&report.iter().map(crate::system_import::FieldReport::label).collect::<Vec<_>>().join("\n"))).build();
                    body.append(&details);
                    let candidates=optional_candidates();
                    let mut choices=Vec::new();
                    for (kind,path) in candidates {
                        let check=gtk::CheckButton::with_label(&format!("Include {} candidate (not confirmed in this terminal)",kind.label()));
                        check.set_tooltip_text(Some(&path.display().to_string()));
                        body.append(&check); choices.push((check,kind,path));
                    }
                    create.set_sensitive(true);
                    let weak=Rc::downgrade(&this); let w=window.downgrade(); let status=status.clone();
                    create.connect_clicked(move |button|{
                        let Some(this)=weak.upgrade() else{return;};
                        button.set_sensitive(false);
                        let selected:Vec<_>=choices.iter().filter(|(c,_,_)|c.is_active()).map(|(_,k,p)|(*k,p.clone())).collect();
                        let mut next=design.clone();
                        let reference=this.workspace_snapshot();
                        let (tx,rx)=mpsc::channel();
                        std::thread::spawn(move ||{let result=attach_candidates(&mut next,&selected,&reference).map(|()|next);let _=tx.send(result);});
                        let weak=Rc::downgrade(&this); let w=w.clone(); let button=button.downgrade();let status=status.clone();
                        glib::timeout_add_local(Duration::from_millis(40),move ||{
                            let (Some(this),Some(w))=(weak.upgrade(),w.upgrade()) else{return glib::ControlFlow::Break;};
                            match rx.try_recv(){
                                Err(mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,
                                Ok(Ok(design))=>{w.destroy();this.open_design_window(design);},
                                result=>{status.set_text(&format!("Not created: {}",match result{Ok(Err(e))=>e,_=>"Import worker unavailable".into()})); if let Some(b)=button.upgrade(){b.set_sensitive(true);}}
                            }
                            glib::ControlFlow::Break
                        });
                    });
                }
            }
            glib::ControlFlow::Break
        });
        window.present();
    }
}
fn optional_candidates() -> Vec<(Kind, PathBuf)> {
    let mut result = Vec::new();
    let starship = std::env::var_os("STARSHIP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::user_config_dir().join("starship.toml"));
    if starship.is_file() {
        result.push((Kind::Prompt, starship));
    }
    for name in ["fastfetch/config.jsonc", "fastfetch/config.json"] {
        let path = glib::user_config_dir().join(name);
        if path.is_file() {
            result.push((Kind::Greeting, path));
            break;
        }
    }
    result
}
fn attach_candidates(
    design: &mut DesignDocument,
    selected: &[(Kind, PathBuf)],
    reference: &Workspace,
) -> Result<(), String> {
    for (kind, path) in selected {
        let bytes = typography_preset::read_private_with_limit(path, DesignDocument::MAX_BYTES)?
            .ok_or("Candidate no longer exists")?;
        let mut imported = crate::design_document::import_as(&bytes, *kind, reference)?;
        if *kind == Kind::Prompt {
            design.components.prompt = imported.components.prompt;
            design.theme.as_mut().unwrap().prompt_enabled = true;
        } else if let Some(mut greeting) = imported.components.greeting.take() {
            if let Some(source) = greeting.imported_source.as_deref() {
                greeting.source_logo = crate::greeting_art::snapshot_file(source, path)?;
            }
            design.components.greeting = Some(greeting);
        }
        if let Some(source) = imported.native {
            design.theme.as_mut().unwrap().retain(source);
        }
        design
            .theme
            .as_mut()
            .unwrap()
            .import_report
            .push(crate::system_import::FieldReport {
                field: kind.label().into(),
                status: crate::system_import::ReadStatus::Read,
                source: format!(
                    "Explicitly selected candidate: {}; runtime association unverified",
                    path.display()
                ),
            });
    }
    design.validate()
}
