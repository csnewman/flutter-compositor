use self::utils::CStringVec;



use std::ffi::{CString};

use std::{
    cell::{Ref, RefCell},
    env,
    path::PathBuf,
    rc::Rc,
};



use crate::{FlutterCompositor, FlutterCompositorWeakRef};
use log::{debug, error, info, trace, warn};

use std::borrow::{Borrow, BorrowMut};



pub(crate) mod utils;



use crate::flutter::channel::{Channel, ChannelRegistry};


use parking_lot::{ReentrantMutexGuard};




use crate::flutter::ffi::{PlatformMessage, PlatformMessageResponseHandle};


use crate::flutter::textinput::TextInputManager;

#[macro_use]
pub mod macros;

mod callbacks;
pub(crate) mod ffi;

pub mod channel;
pub mod codec;

pub mod error;

pub mod textinput;

pub struct FlutterEngine {
    compositor: RefCell<FlutterCompositorWeakRef>,
    engine_ptr: RefCell<ffi::FlutterEngine>,
    pub channel_registry: ChannelRegistry,
    pub text_input: RefCell<TextInputManager>,
}

impl FlutterEngine {
    pub fn new() -> Self {
        Self {
            compositor: RefCell::new(Default::default()),
            engine_ptr: RefCell::new(std::ptr::null_mut()),
            channel_registry: ChannelRegistry::new(),
            text_input: RefCell::new(TextInputManager::new()),
        }
    }

    pub fn set_compositor(&self, compositor: FlutterCompositorWeakRef) {
        self.compositor.replace(compositor.clone());
        self.channel_registry.set_compositor(compositor.clone());
        self.text_input.borrow_mut().set_compositor(compositor);
    }

    pub fn run(guard: &mut ReentrantMutexGuard<FlutterCompositor>) {
        guard
            .engine
            .text_input
            .borrow_mut()
            .register_channels(&guard.engine.channel_registry);

        info!("Starting flutter engine");
        let args = vec!["flutter-compositor"];
        let arguments = CStringVec::new(&args);

        let app_dir = env::current_exe()
            .expect("Cannot get application dir")
            .parent()
            .expect("Cannot get application dir")
            .to_path_buf();

        let assets_path = match env::var("CARGO_MANIFEST_DIR") {
            Ok(proj_dir) => {
                info!("Running inside cargo project");
                let proj_dir = PathBuf::from(&proj_dir);
                proj_dir
                    .parent()
                    .unwrap()
                    .join("flutter")
                    .join("build")
                    .join("flutter_assets")
                    .to_str()
                    .unwrap()
                    .to_string()
            }
            Err(_) => app_dir.join("flutter_assets").to_str().unwrap().to_string(),
        };

        let icu_data_path = app_dir.join("icudtl.dat").to_str().unwrap().to_string();

        info!("Asset path: {}", &assets_path);
        info!("ICU path: {}", &icu_data_path);

        let renderer_config = ffi::FlutterRendererConfig {
            type_: ffi::FlutterRendererType::kOpenGL,
            __bindgen_anon_1: ffi::FlutterRendererConfig__bindgen_ty_1 {
                open_gl: ffi::FlutterOpenGLRendererConfig {
                    struct_size: std::mem::size_of::<ffi::FlutterOpenGLRendererConfig>(),
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
        let project_args = ffi::FlutterProjectArgs {
            struct_size: std::mem::size_of::<ffi::FlutterProjectArgs>(),
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
            shutdown_dart_vm_when_done: true,
            compositor: std::ptr::null(),
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
                let engine_ptr: ffi::FlutterEngine = std::ptr::null_mut();

                let result = ffi::FlutterEngineRun(
                    1,
                    &renderer_config,
                    &project_args,
                    user_data,
                    &engine_ptr as *const ffi::FlutterEngine as *mut ffi::FlutterEngine,
                );

                (result, engine_ptr)
            });

            if result != ffi::FlutterEngineResult::kSuccess || engine_ptr.is_null() {
                panic!("Engine creation failed {:?}", result);
            } else {
                info!("Engine started");
                guard.engine.engine_ptr.replace(engine_ptr);
            }
        }
    }

    pub fn send_window_metrics_event(&self, width: i32, height: i32, pixel_ratio: f64) {
        let event = ffi::FlutterWindowMetricsEvent {
            struct_size: std::mem::size_of::<ffi::FlutterWindowMetricsEvent>(),
            width: width as usize,
            height: height as usize,
            pixel_ratio,
        };
        unsafe {
            ffi::FlutterEngineSendWindowMetricsEvent(*self.engine_ptr.as_ptr(), &event);
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
    }*/

    pub fn send_platform_message(&self, message: PlatformMessage) {
        trace!("Sending message on channel {}", message.channel);
        unsafe {
            ffi::FlutterEngineSendPlatformMessage(*self.engine_ptr.borrow(), &message.into());
        }
    }

    pub fn send_platform_message_response(
        &self,
        response_handle: PlatformMessageResponseHandle,
        bytes: &[u8],
    ) {
        trace!("Sending message response");
        unsafe {
            ffi::FlutterEngineSendPlatformMessageResponse(
                *self.engine_ptr.borrow(),
                response_handle.into(),
                bytes.as_ptr(),
                bytes.len(),
            );
        }
    }

    pub fn shutdown(&self) {
        unsafe {
            ffi::FlutterEngineShutdown(*self.engine_ptr.borrow());
        }
    }

    pub fn register_texture(&self, texture_id: i64) {
        unsafe {
            ffi::FlutterEngineRegisterExternalTexture(*self.engine_ptr.borrow(), texture_id);
        }
    }

    pub fn unregister_texture(&self, texture_id: i64) {
        unsafe {
            ffi::FlutterEngineUnregisterExternalTexture(*self.engine_ptr.borrow(), texture_id);
        }
    }

    pub fn mark_texture_frame_available(&self, texture_id: i64) {
        unsafe {
            ffi::FlutterEngineMarkExternalTextureFrameAvailable(
                *self.engine_ptr.borrow(),
                texture_id,
            );
        }
    }
}
