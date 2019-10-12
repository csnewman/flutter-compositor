use self::utils::CStringVec;
use smithay::backend::graphics::gl::GLGraphicsBackend;

use smithay::backend::egl::{BufferAccessError, EGLDisplay, Format};
use std::ffi::{c_void, CString};

use std::{
    cell::{Ref, RefCell},
    env,
    path::PathBuf,
    rc::Rc,
};

use crate::renderer::{egl_util, gl_util};
use crate::shell::{MyCompositorToken, MyWindowMap, SurfaceData};
use crate::{FlutterCompositor, FlutterCompositorWeakRef};
use log::{debug, error, info, trace, warn};
use smithay::backend::graphics::{PixelFormat, SwapBuffersError};
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefMut;
use std::sync::{Arc, Mutex, MutexGuard};

mod utils;

use smithay::{
    reexports::wayland_server::protocol::{wl_buffer, wl_surface},
    wayland::{
        compositor::{roles::Role, SubsurfaceRole, TraversalAction},
        shm::with_buffer_contents as shm_buffer_contents,
    },
};

use crate::flutter::channel::{Channel, ChannelRegistry, EventChannel, MethodChannel};
use crate::flutter::codec::{json_codec, Value};
use crate::renderer::egl_util::WrappedContext;
use parking_lot::{RawMutex, RawThreadId, ReentrantMutex, ReentrantMutexGuard};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Weak;

use rand::Rng;

#[macro_use]
mod macros;

mod callbacks;
mod ffi;

mod channel;
mod codec;

mod error;

pub struct FlutterEngine {
    compositor: RefCell<FlutterCompositorWeakRef>,
    engine_ptr: RefCell<flutter_engine_sys::FlutterEngine>,
    channel_registry: ChannelRegistry,
}

impl FlutterEngine {
    pub fn new() -> Self {
        Self {
            compositor: RefCell::new(Default::default()),
            engine_ptr: RefCell::new(std::ptr::null_mut()),
            channel_registry: ChannelRegistry::new(),
        }
    }

    pub fn set_compositor(&self, compositor: FlutterCompositorWeakRef) {
        self.compositor.replace(compositor.clone());
        self.channel_registry.set_compositor(compositor);
    }

    pub fn run(guard: &mut ReentrantMutexGuard<FlutterCompositor>) {
        info!("Starting flutter engine");
        let args = vec!["flutter-compositor"];
        let arguments = CStringVec::new(&args);

        let (assets_path, icu_data_path) = match env::var("CARGO_MANIFEST_DIR") {
            Ok(proj_dir) => {
                info!("Running inside cargo project");
                let proj_dir = PathBuf::from(&proj_dir);
                (
                    proj_dir
                        .parent()
                        .unwrap()
                        .join("flutter")
                        .join("build")
                        .join("flutter_assets")
                        .to_str()
                        .unwrap()
                        .to_string(),
                    proj_dir
                        .join("assets/icudtl.dat")
                        .to_str()
                        .unwrap()
                        .to_string(),
                )
            }
            Err(_) => {
                let res = env::current_exe()
                    .expect("Cannot get application dir")
                    .parent()
                    .expect("Cannot get application dir")
                    .to_path_buf();
                (
                    res.join("flutter_assets").to_str().unwrap().to_string(),
                    res.join("icudtl.dat").to_str().unwrap().to_string(),
                )
            }
        };

        info!("Asset path: {}", &assets_path);
        info!("ICU path: {}", &icu_data_path);

        let renderer_config = flutter_engine_sys::FlutterRendererConfig {
            type_: flutter_engine_sys::FlutterRendererType::kOpenGL,
            __bindgen_anon_1: flutter_engine_sys::FlutterRendererConfig__bindgen_ty_1 {
                open_gl: flutter_engine_sys::FlutterOpenGLRendererConfig {
                    struct_size: std::mem::size_of::<flutter_engine_sys::FlutterOpenGLRendererConfig>(
                    ),
                    make_current: Some(callbacks::make_current),
                    clear_current: Some(callbacks::clear_current),
                    present: Some(callbacks::present),
                    fbo_callback: Some(callbacks::fbo_callback),
                    make_resource_current: Some(callbacks::make_resource_current),
                    fbo_reset_after_present: false,
                    surface_transformation: None,
                    gl_proc_resolver: Some(callbacks::gl_proc_resolver),
                    gl_external_texture_frame_callback: Some(
                        callbacks::gl_external_texture_frame_callback,
                    ),
                },
            },
        };
        let project_args = flutter_engine_sys::FlutterProjectArgs {
            struct_size: std::mem::size_of::<flutter_engine_sys::FlutterProjectArgs>(),
            assets_path: CString::new(assets_path).unwrap().into_raw(),
            main_path__unused__: std::ptr::null(),
            packages_path__unused__: std::ptr::null(),
            icu_data_path: CString::new(icu_data_path).unwrap().into_raw(),
            command_line_argc: arguments.len() as i32,
            command_line_argv: arguments.into_raw(),
            platform_message_callback: Some(callbacks::platform_message_callback),
            vm_snapshot_data: std::ptr::null(),
            vm_snapshot_data_size: 0,
            vm_snapshot_instructions: std::ptr::null(),
            vm_snapshot_instructions_size: 0,
            isolate_snapshot_data: std::ptr::null(),
            isolate_snapshot_data_size: 0,
            isolate_snapshot_instructions: std::ptr::null(),
            isolate_snapshot_instructions_size: 0,
            root_isolate_create_callback: Some(callbacks::root_isolate_create_callback),
            update_semantics_node_callback: None,
            update_semantics_custom_action_callback: None,
            persistent_cache_path: std::ptr::null(),
            is_persistent_cache_read_only: false,
            vsync_callback: None,
            custom_dart_entrypoint: std::ptr::null(),
            custom_task_runners: std::ptr::null(),
        };

        unsafe {
            info!("Starting flutter engine");

            let user_data: *mut std::ffi::c_void = guard
                .engine
                .compositor
                .borrow()
                .upgrade()
                .unwrap()
                .to_mutex_ptr() as _;

            let (result, engine_ptr) = ReentrantMutexGuard::unlocked(guard, move || {
                let engine_ptr: flutter_engine_sys::FlutterEngine = std::ptr::null_mut();

                let result = flutter_engine_sys::FlutterEngineRun(
                    1,
                    &renderer_config,
                    &project_args,
                    user_data,
                    &engine_ptr as *const flutter_engine_sys::FlutterEngine
                        as *mut flutter_engine_sys::FlutterEngine,
                );

                (result, engine_ptr)
            });

            if result != flutter_engine_sys::FlutterEngineResult::kSuccess || engine_ptr.is_null() {
                panic!("Engine creation failed {:?}", result);
            } else {
                info!("Engine started");
                guard.engine.engine_ptr.replace(engine_ptr);
            }
        }
    }

    pub fn send_window_metrics_event(&self, width: i32, height: i32, pixel_ratio: f64) {
        let event = flutter_engine_sys::FlutterWindowMetricsEvent {
            struct_size: std::mem::size_of::<flutter_engine_sys::FlutterWindowMetricsEvent>(),
            width: width as usize,
            height: height as usize,
            pixel_ratio,
        };
        unsafe {
            flutter_engine_sys::FlutterEngineSendWindowMetricsEvent(
                *self.engine_ptr.as_ptr(),
                &event,
            );
        }
    }

    /*pub fn send_pointer_event(
        &self,
        phase: FlutterPointerPhase,
        x: f64,
        y: f64,
        signal_kind: FlutterPointerSignalKind,
        scroll_delta_x: f64,
        scroll_delta_y: f64,
        buttons: FlutterPointerMouseButtons,
    ) {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let buttons: flutter_engine_sys::FlutterPointerMouseButtons = buttons.into();
        let event = flutter_engine_sys::FlutterPointerEvent {
            struct_size: mem::size_of::<flutter_engine_sys::FlutterPointerEvent>(),
            timestamp: timestamp.as_micros() as usize,
            phase: phase.into(),
            x,
            y,
            device: 0,
            signal_kind: signal_kind.into(),
            scroll_delta_x,
            scroll_delta_y,
            device_kind: flutter_engine_sys::FlutterPointerDeviceKind::kFlutterPointerDeviceKindMouse,
            buttons: buttons as i64,
        };
        unsafe {
            flutter_engine_sys::FlutterEngineSendPointerEvent(self.engine_ptr, &event, 1);
        }
    }

    pub fn send_platform_message(&self, message: PlatformMessage) {
        trace!("Sending message on channel {}", message.channel);
        unsafe {
            flutter_engine_sys::FlutterEngineSendPlatformMessage(self.engine_ptr, &message.into());
        }
    }

    pub fn send_platform_message_response(
        &self,
        response_handle: PlatformMessageResponseHandle,
        bytes: &[u8],
    ) {
        trace!("Sending message response");
        unsafe {
            flutter_engine_sys::FlutterEngineSendPlatformMessageResponse(
                self.engine_ptr,
                response_handle.into(),
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }*/

    pub fn shutdown(&self) {
        unsafe {
            flutter_engine_sys::FlutterEngineShutdown(*self.engine_ptr.borrow());
        }
    }

    pub fn register_texture(&self, texture_id: i64) {
        unsafe {
            flutter_engine_sys::FlutterEngineRegisterExternalTexture(
                *self.engine_ptr.borrow(),
                texture_id,
            );
        }
    }

    pub fn unregister_texture(&self, texture_id: i64) {
        unsafe {
            flutter_engine_sys::FlutterEngineUnregisterExternalTexture(
                *self.engine_ptr.borrow(),
                texture_id,
            );
        }
    }

    pub fn mark_texture_frame_available(&self, texture_id: i64) {
        unsafe {
            flutter_engine_sys::FlutterEngineMarkExternalTextureFrameAvailable(
                *self.engine_ptr.borrow(),
                texture_id,
            );
        }
    }
}
