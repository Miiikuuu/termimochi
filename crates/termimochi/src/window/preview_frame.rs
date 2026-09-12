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
            let (w, h, scale) = geometry(active, mode, width, height, target_w, target_h);
            self.scale.set(scale);
            if let Some(child) = self.child.borrow().as_ref() {
                child.allocate(
                    w,
                    h,
                    -1,
                    Some(gtk::gsk::Transform::new().scale(scale as f32, scale as f32)),
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
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
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
    if !active || mode == 4 {
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
        assert_eq!(geometry(true, 0, 550, 400, 1000, 800), (1000, 800, 0.5));
        assert_eq!(geometry(true, 0, 1600, 1200, 1000, 800), (1000, 800, 1.0));
        assert_eq!(geometry(false, 0, 550, 400, 1000, 800), (550, 400, 1.0));
    }
}
