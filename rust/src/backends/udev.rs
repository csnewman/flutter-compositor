use std::{
    cell::RefCell,
    collections::HashMap,
    io::Error as IoError,
    os::unix::io::{AsRawFd, RawFd},
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use smithay::backend::egl::{EGLDisplay, EGLGraphicsBackend};
use smithay::{
    backend::{
        drm::{
            device_bind,
            egl::{EglDevice, EglSurface},
            gbm::{egl::Gbm as EglGbmBackend, GbmDevice},
            legacy::LegacyDrmDevice,
            DevPath, Device, DeviceHandler, Surface,
        },
        graphics::CursorBackend,
        input::InputBackend,
        libinput::{libinput_bind, LibinputInputBackend, LibinputSessionInterface},
        session::{
            auto::{auto_session_bind, AutoSession},
            notify_multiplexer, AsSessionObserver, Session, SessionNotifier,
        },
        udev::{primary_gpu, udev_backend_bind, UdevBackend, UdevHandler},
    },
    reexports::{
        drm::control::{
            connector::{Info as ConnectorInfo, State as ConnectorState},
            crtc,
            encoder::Info as EncoderInfo,
        },
        image::{ImageBuffer, Rgba},
        input::Libinput,
        nix::{fcntl::OFlag, sys::stat::dev_t},
        wayland_server::{
            calloop::{
                generic::{EventedFd, Generic},
                EventLoop, LoopHandle, Source,
            },
            protocol::{wl_output, wl_surface},
            Display,
        },
    },
    wayland::{
        compositor::CompositorToken,
        data_device::{
            default_action_chooser, init_data_device, set_data_device_focus, DataDeviceEvent,
        },
        output::{Mode, Output, PhysicalProperties},
        seat::{CursorImageStatus, Seat, XkbConfig},
        shm::init_shm_global,
    },
};

use crate::shell::{init_shell, MyWindowMap, Roles};

use crate::backends::input::handler::FlutterInputHandler;
use crate::backends::{CompositorBackend, CompositorBackendKind};
use crate::renderer::egl_util::{WrappedContext, WrappedDisplay};
use crate::FlutterCompositorWeakRef;
use chrono::Utc;
use log::{debug, error, info, trace, warn};
use smithay::backend::graphics::gl::GLGraphicsBackend;
use smithay::backend::session::auto::{AutoSessionNotifier, BoundAutoSession};
use std::ffi::c_void;

pub struct SessionFd(RawFd);
impl AsRawFd for SessionFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

type RenderDevice =
    EglDevice<EglGbmBackend<LegacyDrmDevice<SessionFd>>, GbmDevice<LegacyDrmDevice<SessionFd>>>;
type RenderSurface =
    EglSurface<EglGbmBackend<LegacyDrmDevice<SessionFd>>, GbmDevice<LegacyDrmDevice<SessionFd>>>;

pub struct UdevInner {
    compositor: RefCell<FlutterCompositorWeakRef>,
    active_egl_context: RefCell<Option<EGLDisplay>>,
    session: RefCell<Option<AutoSession>>,
    notifier: RefCell<Option<AutoSessionNotifier>>,
    surface: RefCell<Option<RenderSurface>>,
    display: RefCell<Option<WrappedDisplay>>,
    resource_context: RefCell<Option<WrappedContext>>,
    bound_session: RefCell<Option<BoundAutoSession>>,
}

impl Default for UdevInner {
    fn default() -> Self {
        Self {
            compositor: RefCell::new(FlutterCompositorWeakRef::default()),
            active_egl_context: RefCell::new(None),
            session: RefCell::new(None),
            notifier: RefCell::new(None),
            surface: RefCell::new(None),
            display: RefCell::new(None),
            resource_context: RefCell::new(None),
            bound_session: RefCell::new(None),
        }
    }
}

impl UdevInner {
    pub fn set_compositor(&self, compositor: FlutterCompositorWeakRef) {
        self.compositor.replace(compositor);
    }

    pub fn init_session(&self) {
        debug!("Initialising session");
        let (session, mut notifier) = AutoSession::new(None).unwrap();
        let (udev_observer, udev_notifier) = notify_multiplexer();
        let udev_session_id = notifier.register(udev_observer);

        self.session.replace(Some(session));
        self.notifier.replace(Some(notifier));
    }

    pub fn init_io(&self, compositor_token: CompositorToken<Roles>, event_loop: &EventLoop<()>) {
        let context = ::smithay::reexports::udev::Context::new().unwrap();
        let seat = self.session.borrow().as_ref().unwrap().seat();

        let primary_gpu = primary_gpu(&context, &seat).unwrap_or_default();

        // Init UDev backend
        let udev_backend = UdevBackend::new(
            &context,
            UdevHandlerImpl {
                compositor: self.compositor.borrow().clone(),
                compositor_token,
                session: self.session.borrow().as_ref().unwrap().clone(),
                //                display: display.clone(),
                primary_gpu,
                loop_handle: event_loop.handle(),
                //                notifier: udev_notifier,
            },
            seat.clone(),
            None,
        )
        .unwrap();

        let udev_event_source = udev_backend_bind(udev_backend, &event_loop.handle())
            .map_err(|e| -> IoError { e.into() })
            .unwrap();

        // Init libinput
        let mut libinput_context = Libinput::new_from_udev::<LibinputSessionInterface<AutoSession>>(
            self.session.borrow().as_ref().unwrap().clone().into(),
            &context,
        );
        let libinput_session_id = self
            .notifier
            .borrow_mut()
            .as_mut()
            .unwrap()
            .register(libinput_context.observer());
        libinput_context.udev_assign_seat(&seat).unwrap();
        let mut libinput_backend = LibinputInputBackend::new(libinput_context, None);
        libinput_backend.set_handler(FlutterInputHandler::new(self.compositor.borrow().clone()));

        let libinput_event_source = libinput_bind(libinput_backend, event_loop.handle())
            .map_err(|e| -> IoError { e.into() })
            .unwrap();

        info!("Done init_io")
    }

    pub fn seat_name(&self) -> String {
        self.session.borrow().as_ref().unwrap().seat()
    }

    pub fn run(&self, display: &RefCell<Option<Display>>, event_loop: &mut EventLoop<()>) {
        let session_event_source =
            auto_session_bind(self.notifier.replace(None).unwrap(), &event_loop.handle())
                .map_err(|(e, _)| e)
                .unwrap();

        self.bound_session.replace(Some(session_event_source));

        // Cleanup stuff
        //        window_map.borrow_mut().clear();
        //
        //        let mut notifier = session_event_source.unbind();
        //        notifier.unregister(libinput_session_id);
        //        notifier.unregister(udev_session_id);
        //
        //        libinput_event_source.remove();
        //        udev_event_source.remove();
        //
        //        Ok(())
    }

    pub fn present(&self) -> bool {
        match self.surface.borrow().as_ref().unwrap().swap_buffers() {
            Ok(_) => true,
            Err(_) => {
                error!("Failed to present");
                false
            }
        }
    }

    pub fn make_current(&self) -> bool {
        unsafe {
            match self.surface.borrow().as_ref().unwrap().make_current() {
                Ok(_) => true,
                Err(val) => {
                    error!("Failed to make current");
                    false
                }
            }
        }
    }

    pub fn make_resource_current(&self) -> bool {
        unsafe {
            if !self
                .resource_context
                .borrow()
                .as_ref()
                .unwrap()
                .apply_context(self.display.borrow().as_ref().unwrap())
            {
                error!("Failed to make resource current");
                return false;
            }
        }
        true
    }

    pub fn clear_current(&self) -> bool {
        unsafe {
            self.display.borrow().as_ref().unwrap().release_context();
            true
        }
    }

    pub fn gl_proc_resolver(&self, proc: &str) -> *mut c_void {
        unsafe {
            self.surface
                .borrow()
                .as_ref()
                .unwrap()
                .get_proc_address(proc) as *mut c_void
        }
    }

    pub fn get_framebuffer_size(&self) -> (u32, u32) {
        self.surface
            .borrow()
            .as_ref()
            .unwrap()
            .get_framebuffer_dimensions()
    }
}

struct UdevHandlerImpl<Data: 'static> /*<S: SessionNotifier, Data: 'static>*/ {
    compositor: FlutterCompositorWeakRef,
    compositor_token: CompositorToken<Roles>,
    session: AutoSession,
    //    display: Rc<RefCell<Display>>,
    primary_gpu: Option<PathBuf>,
    loop_handle: LoopHandle<Data>,
    //    notifier: S,
}

impl</*S: SessionNotifier,*/ Data: 'static> UdevHandlerImpl</*S,*/ Data> {
    pub fn scan_connectors(device: &mut RenderDevice) -> RenderSurface {
        // Get a set of all modesetting resource handles (excluding planes):
        let res_handles = device.resource_handles().unwrap();

        // Use first connected connector
        let connector_infos: Vec<ConnectorInfo> = res_handles
            .connectors()
            .iter()
            .map(|conn| device.resource_info::<ConnectorInfo>(*conn).unwrap())
            .filter(|conn| conn.connection_state() == ConnectorState::Connected)
            .inspect(|conn| info!("Connected: {:?}", conn.connector_type()))
            .collect();

        // very naive way of finding good crtc/encoder/connector combinations. This problem is np-complete
        for connector_info in connector_infos {
            let encoder_infos = connector_info
                .encoders()
                .iter()
                .flat_map(|encoder_handle| device.resource_info::<EncoderInfo>(*encoder_handle))
                .collect::<Vec<EncoderInfo>>();
            for encoder_info in encoder_infos {
                for crtc in res_handles.filter_crtcs(encoder_info.possible_crtcs()) {
                    //                    if !backends.contains_key(&crtc) {
                    info!("Found surface");
                    let surface = device.create_surface(crtc).unwrap();
                    return surface;
                    //                    }
                }
            }
        }

        panic!("No surface found");
    }
}

impl</*S: SessionNotifier,*/ Data: 'static> UdevHandler for UdevHandlerImpl</*S,*/ Data> {
    fn device_added(&mut self, _device: dev_t, path: PathBuf) {
        info!("device_added");
        // Try to open the device
        if let Some(mut device) = self
            .session
            .open(
                &path,
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            )
            .ok()
            .and_then(|fd| LegacyDrmDevice::new(SessionFd(fd), None).ok())
            .and_then(|drm| GbmDevice::new(drm, None).ok())
            .and_then(|gbm| EglDevice::new(gbm, None).ok())
        {
            info!(
                "primary_gpu={:?}   path={:?}",
                self.primary_gpu,
                path.canonicalize().ok()
            );

            //            if path.canonicalize().ok() != self.primary_gpu {
            //                return;
            //            }

            let compositor_ref = self.compositor.upgrade().unwrap();
            let compositor = compositor_ref.get();

            if let CompositorBackendKind::TtyUDev(inner) = &compositor.backend.kind {
                inner.active_egl_context.replace(Some(
                    device
                        .bind_wl_display(compositor.backend.display.borrow().as_ref().unwrap())
                        .unwrap(),
                ));

                let surface = UdevHandlerImpl::<Data>::scan_connectors(&mut device);

                debug!("Creating extra EGL contexts");
                unsafe {
                    surface.make_current();
                    let display = WrappedDisplay::new();

                    let resource_context = WrappedContext::create_context();
                    inner.resource_context.replace(Some(resource_context));

                    display.release_context();

                    inner.display.replace(Some(display));
                    //                    self.display.replace(Some(display));
                }

                inner.surface.replace(Some(surface));

                info!("Surface set ");

                // Set the handler.
                // Note: if you replicate this (very simple) structure, it is rather easy
                // to introduce reference cycles with Rc. Be sure about your drop order
                device.set_handler(DrmHandlerImpl {
                    compositor_token: self.compositor_token,
                    //                backends: backends.clone(),
                    //                window_map: self.window_map.clone(),
                    //                pointer_location: self.pointer_location.clone(),
                    //                cursor_status: self.cursor_status.clone(),
                    //                dnd_icon: self.dnd_icon.clone(),
                    //                logger: self.logger.clone(),
                });

                let device_session_id = inner
                    .notifier
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .register(device.observer());

                //                let device_session_id = self.notifier.register(device.observer());
                let dev_id = device.device_id();
                let event_source = device_bind(&self.loop_handle, device)
                    .map_err(|e| -> IoError { e.into() })
                    .unwrap();

                //            for renderer in backends.borrow_mut().values() {
                //                // create cursor
                //                renderer
                //                    .borrow()
                //                    .set_cursor_representation(&self.pointer_image, (2, 2))
                //                    .unwrap();
                //
                //                // render first frame
                //                {
                //                    let mut frame = renderer.draw();
                //                    frame.clear_color(0.8, 0.8, 0.9, 1.0);
                //                    frame.finish().unwrap();
                //                }
                //            }
                //
                //            self.backends
                //                .insert(dev_id, (device_session_id, event_source, backends));
            }
        }
    }

    fn device_changed(&mut self, device: dev_t) {
        info!("device_changed")
    }

    fn device_removed(&mut self, device: dev_t) {
        info!("device_removed")
    }
}

pub struct DrmHandlerImpl {
    compositor_token: CompositorToken<Roles>,
    //    backends: Rc<RefCell<HashMap<crtc::Handle, GliumDrawer<RenderSurface>>>>,
    //    window_map: Rc<RefCell<MyWindowMap>>,
    //    pointer_location: Rc<RefCell<(f64, f64)>>,
    //    cursor_status: Arc<Mutex<CursorImageStatus>>,
    //    dnd_icon: Arc<Mutex<Option<wl_surface::WlSurface>>>,
    //    logger: ::slog::Logger,
}

impl DeviceHandler for DrmHandlerImpl {
    type Device = RenderDevice;

    fn vblank(&mut self, crtc: crtc::Handle) {
        //        if let Some(drawer) = self.backends.borrow().get(&crtc) {
        //            {
        //                let (x, y) = *self.pointer_location.borrow();
        //                let _ = drawer
        //                    .borrow()
        //                    .set_cursor_position(x.trunc().abs() as u32, y.trunc().abs() as u32);
        //            }
        //
        //            // and draw in sync with our monitor
        //            let mut frame = drawer.draw();
        //            frame.clear(None, Some((0.8, 0.8, 0.9, 1.0)), false, Some(1.0), None);
        //            // draw the surfaces
        //            drawer.draw_windows(&mut frame, &*self.window_map.borrow(), self.compositor_token);
        //            let (x, y) = *self.pointer_location.borrow();
        //            // draw the dnd icon if applicable
        //            {
        //                let guard = self.dnd_icon.lock().unwrap();
        //                if let Some(ref surface) = *guard {
        //                    if surface.as_ref().is_alive() {
        //                        drawer.draw_dnd_icon(
        //                            &mut frame,
        //                            surface,
        //                            (x as i32, y as i32),
        //                            self.compositor_token,
        //                        );
        //                    }
        //                }
        //            }
        //            // draw the cursor as relevant
        //            {
        //                let mut guard = self.cursor_status.lock().unwrap();
        //                // reset the cursor if the surface is no longer alive
        //                let mut reset = false;
        //                if let CursorImageStatus::Image(ref surface) = *guard {
        //                    reset = !surface.as_ref().is_alive();
        //                }
        //                if reset {
        //                    *guard = CursorImageStatus::Default;
        //                }
        //                if let CursorImageStatus::Image(ref surface) = *guard {
        //                    drawer.draw_cursor(&mut frame, surface, (x as i32, y as i32), self.compositor_token);
        //                }
        //            }
        //
        //            if let Err(err) = frame.finish() {
        //                error!(self.logger, "Error during rendering: {:?}", err);
        //            }
        //        }
    }

    fn error(&mut self, error: <RenderSurface as Surface>::Error) {
        error!("{:?}", error);
    }
}
