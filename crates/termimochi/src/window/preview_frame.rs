//! A bounded viewport for one simulated window. GTK's allocation transform is
//! shared by painting, picking, text selection and the input method cursor rect.
use super::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct PreviewFrame {
        pub child: RefCell<Option<gtk::Widget>>,
        pub geometry: Cell<(bool, u32, i32, i32)>,
        pub scale: Cell<f64>,
        pub allocated: Cell<(i32, i32, i32, i32, u64)>,
        pub origin: Cell<(f32, f32)>,
        pub changed: RefCell<Option<Box<dyn Fn()>>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for PreviewFrame {
        const NAME: &'static str = "TermiMochiPreviewFrame";
        type Type = super::PreviewFrame;
        type ParentType = gtk::Widget;
    }
    impl ObjectImpl for PreviewFrame {
        fn dispose(&self) {
            if let Some(child) = self.child.borrow_mut().take() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for PreviewFrame {
        fn measure(&self, orientation: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                (100, 500, -1, -1)
            } else {
                (180, 340, -1, -1)
            }
        }
        fn size_allocate(&self, width: i32, height: i32, _: i32) {
            let (active, mode, target_w, target_h) = self.geometry.get();
            let margin = if active {
                canvas_margin(width, height)
            } else {
                0
            };
            let (w, h, scale) = geometry(
                active,
                mode,
                width - margin * 2,
                height - margin * 2,
                target_w,
                target_h,
            );
            let x = ((width as f64 - w as f64 * scale) / 2.0).max(0.0) as f32;
            let y = margin as f32;
            self.origin.set((x, y));
            self.scale.set(scale);
            if let Some(child) = self.child.borrow().as_ref() {
                child.allocate(
                    w,
                    h,
                    -1,
                    Some(
                        gtk::gsk::Transform::new()
                            .translate(&gtk::graphene::Point::new(x, y))
                            .scale(scale as f32, scale as f32),
                    ),
                );
            }
            let allocated = (width, height, w, h, scale.to_bits());
            if self.allocated.replace(allocated) != allocated
                && let Some(changed) = self.changed.borrow().as_ref()
            {
                changed();
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.child.borrow().as_ref() {
                if self.geometry.get().0 {
                    let (_, _, w, h, _) = self.allocated.get();
                    let scale = self.scale.get() as f32;
                    let (x, y) = self.origin.get();
                    let outline = gtk::gsk::RoundedRect::from_rect(
                        gtk::graphene::Rect::new(x, y, w as f32 * scale, h as f32 * scale),
                        10.0 * scale,
                    );
                    // The shadow lives outside the clipped window surface,
                    // within the canvas gutter. It never intercepts input.
                    snapshot.append_node(shadow(
                        &outline,
                        canvas_margin(self.obj().width(), self.obj().height()),
                    ));
                    snapshot.push_rounded_clip(&outline);
                    self.obj().snapshot_child(child, snapshot);
                    snapshot.pop();
                    return;
                }
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}

fn canvas_margin(width: i32, height: i32) -> i32 {
    36.min((width / 8).max(0)).min((height / 8).max(0))
}
fn shadow(outline: &gtk::gsk::RoundedRect, margin: i32) -> gtk::gsk::OutsetShadowNode {
    gtk::gsk::OutsetShadowNode::new(
        outline,
        &gtk::gdk::RGBA::new(0.06, 0.09, 0.12, 0.18),
        0.0,
        6.0,
        0.0,
        ((margin - 8).max(0) as f32 / 1.5).min(18.0),
    )
}
glib::wrapper! {
    pub struct PreviewFrame(ObjectSubclass<imp::PreviewFrame>)
        @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
impl PreviewFrame {
    pub fn new() -> Self {
        let obj: Self = glib::Object::new();
        obj.set_hexpand(true);
        obj.set_vexpand(true);
        obj.set_overflow(gtk::Overflow::Hidden);
        obj
    }
    pub fn set_child(&self, child: &impl IsA<gtk::Widget>) {
        child.set_parent(self);
        *self.imp().child.borrow_mut() = Some(child.clone().upcast());
    }
    pub fn configure(&self, active: bool, mode: u32, width: i32, height: i32) {
        let value = (active, mode, width.max(1), height.max(1));
        if self.imp().geometry.replace(value) != value {
            self.queue_allocate();
        }
    }
    pub fn scale(&self) -> f64 {
        self.imp().scale.get().max(0.001)
    }
    #[cfg(test)]
    pub fn shadow_bounds(&self) -> gtk::graphene::Rect {
        let (_, _, w, h, _) = self.imp().allocated.get();
        let s = self.scale() as f32;
        let (x, y) = self.imp().origin.get();
        shadow(
            &gtk::gsk::RoundedRect::from_rect(
                gtk::graphene::Rect::new(x, y, w as f32 * s, h as f32 * s),
                10.0 * s,
            ),
            canvas_margin(self.width(), self.height()),
        )
        .bounds()
    }
    pub fn connect_geometry_changed(&self, callback: impl Fn() + 'static) {
        *self.imp().changed.borrow_mut() = Some(Box::new(callback));
    }
}

fn geometry(
    active: bool,
    mode: u32,
    width: i32,
    height: i32,
    target_w: i32,
    target_h: i32,
) -> (i32, i32, f64) {
    if !active {
        return (width.max(1), height.max(1), 1.0);
    }
    let scale = match mode {
        0 => (width as f64 / target_w.max(1) as f64)
            .min(height as f64 / target_h.max(1) as f64)
            .clamp(0.001, 1.0),
        2 => 0.75,
        3 => 0.5,
        _ => 1.0,
    };
    (
        target_w.min((width as f64 / scale).floor() as i32).max(1),
        target_h.min((height as f64 / scale).floor() as i32).max(1),
        scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_transform_is_bounded_and_independent_of_history() {
        assert_eq!(geometry(true, 4, 550, 400, 1000, 800), (550, 400, 1.0));
        assert_eq!(geometry(true, 4, 1600, 1200, 1000, 800), (1000, 800, 1.0));
        assert_eq!(geometry(true, 0, 550, 400, 1000, 800), (1000, 800, 0.5));
        assert_eq!(geometry(true, 0, 1600, 1200, 1000, 800), (1000, 800, 1.0));
        assert_eq!(geometry(false, 0, 550, 400, 1000, 800), (550, 400, 1.0));
    }
}
