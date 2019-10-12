use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use smithay::{
    backend::{
        egl::EGLGraphicsBackend, graphics::gl::GLGraphicsBackend, input::InputBackend, winit,
    },
    reexports::{
        calloop::EventLoop,
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::{
        data_device::{
            default_action_chooser, init_data_device, set_data_device_focus, DataDeviceEvent,
        },
        output::{Mode, Output, PhysicalProperties},
        seat::{CursorImageStatus, Seat, XkbConfig},
        shm::init_shm_global,
    },
};

use log::{debug, error, info, trace, warn};

use crate::input_handler::AnvilInputHandler;
use crate::shell::init_shell;
use chrono::Utc;
use smithay::backend::winit::{WinitGraphicsBackend, WinitInputBackend};

use crate::renderer::egl_util::WrappedDisplay;
use ::winit::{dpi::LogicalSize, WindowBuilder};
use std::ffi::c_void;

pub struct WInitInner {
    renderer: RefCell<Option<WinitGraphicsBackend>>,
    input: RefCell<Option<WinitInputBackend>>,
    display: RefCell<Option<WrappedDisplay>>,
}

impl Default for WInitInner {
    fn default() -> Self {
        Self {
            renderer: RefCell::new(None),
            input: RefCell::new(None),
            display: RefCell::new(None),
        }
    }
}

impl WInitInner {
    pub fn create_window(&self) {
        info!("Creating winit window");
        let (renderer, mut input) = winit::init_from_builder(
            WindowBuilder::new()
                .with_dimensions(LogicalSize::new(1280.0 / 1.5, 800.0 / 1.5))
                .with_resizable(false)
                .with_title("Flutter Compositor")
                .with_visibility(true),
            None,
        )
        .unwrap();

        self.renderer.replace(Some(renderer));
        self.input.replace(Some(input));
    }

    pub fn init_io(&self, display: &Display) {
        debug!("Initialising I/O");

        let mut renderer_borrow = self.renderer.borrow_mut();
        let renderer = renderer_borrow.as_mut().unwrap();

        let mut input_borrow = self.input.borrow_mut();
        let input = input_borrow.as_mut().unwrap();

        debug!("Binding EGL to display");
        let egl_display = Rc::new(RefCell::new(
            if let Ok(egl_display) = renderer.bind_wl_display(&display) {
                info!("EGL hardware-acceleration enabled");
                Some(egl_display)
            } else {
                None
            },
        ));

        debug!("Creating extra EGL contexts");
        unsafe {
            renderer.make_current();
            let display = WrappedDisplay::new();

            //            // TODO: Allocate more contexts

            display.release_context();
            self.display.replace(Some(display));
        }

        debug!("Setting input handler");
        input.set_handler(AnvilInputHandler::new(
//            pointer,
//            keyboard,
//            window_map.clone(),
//            (0, 0),
//            running.clone(),
//            pointer_location.clone(),
        ));

        debug!("Done?");
    }

    pub fn update(&self) {
        self.input
            .borrow_mut()
            .as_mut()
            .unwrap()
            .dispatch_new_events()
            .unwrap();
    }

    pub fn present(&self) -> bool {
        match self.renderer.borrow().as_ref().unwrap().swap_buffers() {
            Ok(_) => true,
            Err(_) => {
                error!("Failed to present");
                false
            }
        }
    }

    pub fn make_current(&self) -> bool {
        unsafe {
            match self.renderer.borrow().as_ref().unwrap().make_current() {
                Ok(_) => true,
                Err(val) => {
                    error!("Failed to make current");
                    false
                }
            }
        }
    }

    pub fn clear_current(&self) -> bool {
        unsafe {
            self.display.borrow().as_ref().unwrap().release_context();
            true
        }
    }

    pub fn gl_proc_resolver(&self, proc: &str) -> *mut c_void {
        unsafe {
            self.renderer
                .borrow()
                .as_ref()
                .unwrap()
                .get_proc_address(proc) as *mut c_void
        }
    }

    pub fn get_framebuffer_size(&self) -> (u32, u32) {
        self.renderer
            .borrow()
            .as_ref()
            .unwrap()
            .get_framebuffer_dimensions()
    }
}
