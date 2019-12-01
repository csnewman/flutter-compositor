use crate::backends::udev::UdevInner;
use crate::backends::winit::WInitInner;
use crate::shell::init_shell;
use crate::FlutterCompositorWeakRef;
use smithay::reexports::wayland_server::protocol::wl_output;
use smithay::wayland::data_device::{
    default_action_chooser, init_data_device,
};
use smithay::wayland::output::{Mode, Output, PhysicalProperties};

use smithay::wayland::shm::init_shm_global;
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use wayland_server::calloop::EventLoop;
use wayland_server::Display;

use crate::backends::input::manager::InputManager;
use crate::backends::seat::FlutterSeat;
use log::{debug, error, info, trace, warn};
use std::ffi::c_void;
use std::sync::Arc;

pub(crate) mod udev;
pub(crate) mod winit;

pub(crate) mod seat;

pub(crate) mod input;

pub struct CompositorBackend {
    compositor: RefCell<FlutterCompositorWeakRef>,
    display: RefCell<Option<Display>>,
    pub(crate) event_loop: Arc<RefCell<Option<EventLoop<()>>>>,
    kind: CompositorBackendKind,
    seat: RefCell<Option<FlutterSeat>>,
    pub(crate) input: RefCell<Option<InputManager>>,
}

pub enum CompositorBackendKind {
    WInit(WInitInner),
    TtyUDev(UdevInner),
}

impl CompositorBackend {
    pub fn new_winit() -> Self {
        Self {
            compositor: RefCell::new(FlutterCompositorWeakRef::default()),
            display: RefCell::new(None),
            event_loop: Arc::new(RefCell::new(None)),
            kind: CompositorBackendKind::WInit(WInitInner::default()),
            seat: RefCell::new(None),
            input: RefCell::new(None),
        }
    }

    pub fn new_tty_udev() -> Self {
        Self {
            compositor: RefCell::new(FlutterCompositorWeakRef::default()),
            display: RefCell::new(None),
            event_loop: Arc::new(RefCell::new(None)),
            kind: CompositorBackendKind::TtyUDev(UdevInner::default()),
            seat: RefCell::new(None),
            input: RefCell::new(None),
        }
    }

    pub fn init(&self, compositor: FlutterCompositorWeakRef) {
        info!("Initialising backend");
        self.compositor.replace(compositor.clone());
        match &self.kind {
            CompositorBackendKind::WInit(inner) => {
                inner.set_compositor(compositor.clone());
            }
            CompositorBackendKind::TtyUDev(inner) => {
                inner.set_compositor(compositor.clone());
            }
        }

        // Create display
        if let CompositorBackendKind::WInit(inner) = &self.kind {
            inner.create_window();
        }

        // Create display
        debug!("Creating display");
        let event_loop = EventLoop::<()>::new().unwrap();
        let display = Display::new(event_loop.handle());

        self.event_loop.replace(Some(event_loop));
        self.display.replace(Some(display));

        let mut display_borrow = self.display.borrow_mut();
        let mut display = display_borrow.as_mut().unwrap();

        // Start wayland socket
        debug!("Creating wayland socket");
        let name = display.add_socket_auto().unwrap().into_string().unwrap();
        info!("Listening on wayland socket {}", name.clone());
        ::std::env::set_var("WAYLAND_DISPLAY", name);

        // Init SHM
        debug!("Initialising SHM");
        init_shm_global(&mut display, vec![], None);

        // Init shell
        debug!("Creating shell");
        let (compositor_token, _, _, _window_map) = init_shell(&mut display);

        // Enable clipboard/DND support
        debug!("Initialising data device");
        init_data_device(
            &mut display,
            move |event| match event {
                _ => {}
            },
            default_action_chooser,
            compositor_token.clone(),
            None,
        );

        // Create session
        if let CompositorBackendKind::TtyUDev(inner) = &self.kind {
            inner.init_session();
        }

        // Initialising IO
        self.input.replace(Some(InputManager::new(
            Default::default(),
            self.compositor.borrow().clone(),
        )));

        match &self.kind {
            CompositorBackendKind::WInit(inner) => {
                inner.init_io(display);
            }
            CompositorBackendKind::TtyUDev(inner) => {
                drop(display);
                drop(display_borrow);
                inner.init_io(
                    compositor_token.clone(),
                    RefCell::borrow(self.event_loop.borrow()).as_ref().unwrap(),
                );
                display_borrow = self.display.borrow_mut();
                display = display_borrow.as_mut().unwrap();
            }
        }

        // Configure input
        debug!("Configuring input");
        let seat = FlutterSeat::new(
            compositor.clone(),
            match &self.kind {
                CompositorBackendKind::WInit(_inner) => "Winit".into(),
                CompositorBackendKind::TtyUDev(inner) => inner.seat_name(),
            },
        );
        seat.create(&mut display, compositor_token.clone());
        self.seat.replace(Some(seat));

        info!("1");

        /*
        let pointer = w_seat.add_pointer(compositor_token.clone(), move |new_status| {
            //            *cursor_status.lock().unwrap() = new_status;
        });
        let keyboard = w_seat
            .add_keyboard(XkbConfig::default(), 1000, 500, |seat, focus| {
                set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
            })
            .expect("Failed to initialize the keyboard");
        self.keyboard.replace(Some(keyboard));*/

        let (output, _output_global) = Output::new(
            &mut display,
            "Temporary Output".into(),
            PhysicalProperties {
                width: 0,
                height: 0,
                subpixel: wl_output::Subpixel::Unknown,
                make: "Flutter-Compositor".into(),
                model: "Generic".into(),
            },
            None,
        );

        let (w, h) = self.get_framebuffer_size();
        output.change_current_state(
            Some(Mode {
                width: w as i32,
                height: h as i32,
                refresh: 60_000,
            }),
            None,
            None,
        );
        output.set_preferred(Mode {
            width: w as i32,
            height: h as i32,
            refresh: 60_000,
        });

        info!("3");
    }

    pub fn run(&self) {
        info!("run1");
        match &self.kind {
            CompositorBackendKind::WInit(_inner) => {}
            CompositorBackendKind::TtyUDev(inner) => {
                inner.run(
                    &self.display,
                    &mut RefCell::borrow_mut(self.event_loop.borrow())
                        .as_mut()
                        .unwrap(),
                );
            }
        }

        info!("run2");
    }

    pub fn update(&self) {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => {
                inner.update();
            }
            CompositorBackendKind::TtyUDev(_inner) => {
                //                inner.run(&self.display, &mut RefCell::borrow_mut(self.event_loop.borrow()).as_mut().unwrap());
            }
        }

        self.display.borrow().as_ref().unwrap().flush_clients();
    }

    pub fn present(&self) -> bool {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.present(),
            CompositorBackendKind::TtyUDev(inner) => inner.present(),
        }
    }

    pub fn make_current(&self) -> bool {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.make_current(),
            CompositorBackendKind::TtyUDev(inner) => inner.make_current(),
        }
    }

    pub fn make_resource_current(&self) -> bool {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.make_resource_current(),
            CompositorBackendKind::TtyUDev(inner) => inner.make_resource_current(),
        }
    }

    pub fn clear_current(&self) -> bool {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.clear_current(),
            CompositorBackendKind::TtyUDev(inner) => inner.clear_current(),
        }
    }

    pub fn gl_proc_resolver(&self, proc: &str) -> *mut c_void {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.gl_proc_resolver(proc),
            CompositorBackendKind::TtyUDev(inner) => inner.gl_proc_resolver(proc),
        }
    }

    pub fn get_framebuffer_size(&self) -> (u32, u32) {
        match &self.kind {
            CompositorBackendKind::WInit(inner) => inner.get_framebuffer_size(),
            CompositorBackendKind::TtyUDev(inner) => inner.get_framebuffer_size(),
        }
    }
}
