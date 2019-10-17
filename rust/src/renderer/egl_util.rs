use core::{mem, ptr};
use log::{debug, info};

use crate::renderer::gl;
use smithay::backend::egl::context::PixelFormatRequirements;
use smithay::backend::egl::ffi;
use smithay::backend::graphics::PixelFormat;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_int;

pub struct WrappedDisplay(ffi::egl::types::EGLDisplay);

impl Clone for WrappedDisplay {
    fn clone(&self) -> Self {
        WrappedDisplay { 0: self.0 }
    }
}

impl WrappedDisplay {
    pub unsafe fn new() -> Self {
        let display = ffi::egl::GetCurrentDisplay();

        if display == ptr::null() {
            panic!("Failed to fetch display");
        }

        info!("Current display was {:?}", display);
        WrappedDisplay(display)
    }

    pub unsafe fn release_context(&self) {
        let ret = ffi::egl::MakeCurrent(self.0, ptr::null(), ptr::null(), ptr::null());
    }
}

pub struct WrappedContext(ffi::egl::types::EGLContext);

impl WrappedContext {
    pub unsafe fn create_context() -> WrappedContext {
        debug!("Trying to initialize EGL with OpenGLES 3.0");
        create_extra_context_inner((3, 0))
        //    attributes.version = Some((3, 0));
        //    match EGLContext::<B, N>::new_internal(ptr, attributes, reqs, log.clone()) {
        //        Ok(x) => return Ok(x),
        //        Err(err) => {
        //            warn!(log, "EGL OpenGLES 3.0 Initialization failed with {}", err);
        //            debug!(log, "Trying to initialize EGL with OpenGLES 2.0");
        //            attributes.version = Some((2, 0));
        //            return EGLContext::<B, N>::new_internal(ptr, attributes, reqs, log);
        //        }
        //    }
    }

    pub unsafe fn current() -> Self {
        Self(ffi::egl::GetCurrentContext())
    }

    pub unsafe fn apply_context(&self, display: &WrappedDisplay) -> bool {
        let ret = ffi::egl::MakeCurrent(display.0, ptr::null(), ptr::null(), self.0);

        ret == 1
    }

    pub unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        let addr = CString::new(symbol.as_bytes()).unwrap();
        let addr = addr.as_ptr();
        ffi::egl::GetProcAddress(addr) as *const _
    }

    pub fn is_current(&self) -> bool {
        unsafe { ffi::egl::GetCurrentContext() == self.0 }
    }

    pub fn get_gl(&self) -> gl::Gl {
        gl::Gl::load_with(|s| unsafe { self.get_proc_address(s) })
    }
}

unsafe fn create_extra_context_inner(version: (u8, u8)) -> WrappedContext {
    let reqs: PixelFormatRequirements = Default::default();

    let display = ffi::egl::GetCurrentDisplay();
    info!("Current display was {:?}", display);

    let egl_version = {
        let mut major: ffi::egl::types::EGLint = mem::uninitialized();
        let mut minor: ffi::egl::types::EGLint = mem::uninitialized();

        if ffi::egl::Initialize(display, &mut major, &mut minor) == 0 {
            panic!("Display reinit failed");
        }

        info!("EGL Version: {:?}", (major, minor));

        (major, minor)
    };

    let extensions = if egl_version >= (1, 2) {
        let p = CStr::from_ptr(ffi::egl::QueryString(display, ffi::egl::EXTENSIONS as i32));
        let list = String::from_utf8(p.to_bytes().to_vec()).unwrap_or_else(|_| String::new());
        list.split(' ').map(|e| e.to_string()).collect::<Vec<_>>()
    } else {
        vec![]
    };

    info!("EGL Extensions: {:?}", extensions);

    if egl_version >= (1, 2) && ffi::egl::BindAPI(ffi::egl::OPENGL_ES_API) == 0 {
        panic!("OpenGLES not supported by the underlying EGL implementation");
    }

    let descriptor = {
        let mut out: Vec<c_int> = Vec::with_capacity(37);

        if egl_version >= (1, 2) {
            debug!("Setting COLOR_BUFFER_TYPE to RGB_BUFFER");
            out.push(ffi::egl::COLOR_BUFFER_TYPE as c_int);
            out.push(ffi::egl::RGB_BUFFER as c_int);
        }

        debug!("Setting SURFACE_TYPE to WINDOW");

        out.push(ffi::egl::SURFACE_TYPE as c_int);
        // TODO: Some versions of Mesa report a BAD_ATTRIBUTE error
        // if we ask for PBUFFER_BIT as well as WINDOW_BIT
        out.push((ffi::egl::WINDOW_BIT) as c_int);

        match version {
            (3, _) => {
                if egl_version < (1, 3) {
                    panic!("OpenglES 3.* is not supported on EGL Versions lower then 1.3");
                }
                debug!("Setting RENDERABLE_TYPE to OPENGL_ES3");
                out.push(ffi::egl::RENDERABLE_TYPE as c_int);
                out.push(ffi::egl::OPENGL_ES3_BIT as c_int);
                debug!("Setting CONFORMANT to OPENGL_ES3");
                out.push(ffi::egl::CONFORMANT as c_int);
                out.push(ffi::egl::OPENGL_ES3_BIT as c_int);
            }
            (2, _) => {
                if egl_version < (1, 3) {
                    panic!("OpenglES 2.* is not supported on EGL Versions lower then 1.3");
                }
                debug!("Setting RENDERABLE_TYPE to OPENGL_ES2");
                out.push(ffi::egl::RENDERABLE_TYPE as c_int);
                out.push(ffi::egl::OPENGL_ES2_BIT as c_int);
                debug!("Setting CONFORMANT to OPENGL_ES2");
                out.push(ffi::egl::CONFORMANT as c_int);
                out.push(ffi::egl::OPENGL_ES2_BIT as c_int);
            }
            (_, _) => unreachable!(),
        };

        if let Some(hardware_accelerated) = reqs.hardware_accelerated {
            out.push(ffi::egl::CONFIG_CAVEAT as c_int);
            out.push(if hardware_accelerated {
                debug!("Setting CONFIG_CAVEAT to NONE");
                ffi::egl::NONE as c_int
            } else {
                debug!("Setting CONFIG_CAVEAT to SLOW_CONFIG");
                ffi::egl::SLOW_CONFIG as c_int
            });
        }

        if let Some(color) = reqs.color_bits {
            debug!("Setting RED_SIZE to {}", color / 3);
            out.push(ffi::egl::RED_SIZE as c_int);
            out.push((color / 3) as c_int);
            debug!(
                "Setting GREEN_SIZE to {}",
                color / 3 + if color % 3 != 0 { 1 } else { 0 }
            );
            out.push(ffi::egl::GREEN_SIZE as c_int);
            out.push((color / 3 + if color % 3 != 0 { 1 } else { 0 }) as c_int);
            debug!(
                "Setting BLUE_SIZE to {}",
                color / 3 + if color % 3 == 2 { 1 } else { 0 }
            );
            out.push(ffi::egl::BLUE_SIZE as c_int);
            out.push((color / 3 + if color % 3 == 2 { 1 } else { 0 }) as c_int);
        }

        if let Some(alpha) = reqs.alpha_bits {
            debug!("Setting ALPHA_SIZE to {}", alpha);
            out.push(ffi::egl::ALPHA_SIZE as c_int);
            out.push(alpha as c_int);
        }

        if let Some(depth) = reqs.depth_bits {
            debug!("Setting DEPTH_SIZE to {}", depth);
            out.push(ffi::egl::DEPTH_SIZE as c_int);
            out.push(depth as c_int);
        }

        if let Some(stencil) = reqs.stencil_bits {
            debug!("Setting STENCIL_SIZE to {}", stencil);
            out.push(ffi::egl::STENCIL_SIZE as c_int);
            out.push(stencil as c_int);
        }

        if let Some(multisampling) = reqs.multisampling {
            debug!("Setting SAMPLES to {}", multisampling);
            out.push(ffi::egl::SAMPLES as c_int);
            out.push(multisampling as c_int);
        }

        if reqs.stereoscopy {
            panic!("Stereoscopy is currently unsupported (sorry!)");
        }

        out.push(ffi::egl::NONE as c_int);
        out
    };

    // calling `eglChooseConfig`
    let mut config_id = mem::uninitialized();
    let mut num_configs = mem::uninitialized();
    if ffi::egl::ChooseConfig(
        display,
        descriptor.as_ptr(),
        &mut config_id,
        1,
        &mut num_configs,
    ) == 0
    {
        panic!("Config failed");
    }
    if num_configs == 0 {
        panic!("No matching color format found");
    }

    // analyzing each config
    macro_rules! attrib {
        ($display:expr, $config:expr, $attr:expr) => {{
            let mut value = mem::uninitialized();
            let res = ffi::egl::GetConfigAttrib(
                $display,
                $config,
                $attr as ffi::egl::types::EGLint,
                &mut value,
            );
            if res == 0 {
                panic!("Config failed");
            }
            value
        }};
    };

    let desc = PixelFormat {
        hardware_accelerated: attrib!(display, config_id, ffi::egl::CONFIG_CAVEAT)
            != ffi::egl::SLOW_CONFIG as i32,
        color_bits: attrib!(display, config_id, ffi::egl::RED_SIZE) as u8
            + attrib!(display, config_id, ffi::egl::BLUE_SIZE) as u8
            + attrib!(display, config_id, ffi::egl::GREEN_SIZE) as u8,
        alpha_bits: attrib!(display, config_id, ffi::egl::ALPHA_SIZE) as u8,
        depth_bits: attrib!(display, config_id, ffi::egl::DEPTH_SIZE) as u8,
        stencil_bits: attrib!(display, config_id, ffi::egl::STENCIL_SIZE) as u8,
        stereoscopy: false,
        double_buffer: true,
        multisampling: match attrib!(display, config_id, ffi::egl::SAMPLES) {
            0 | 1 => None,
            a => Some(a as u16),
        },
        srgb: false, // TODO: use EGL_KHR_gl_colorspace to know that
    };

    info!("Selected color format: {:?}", desc);

    let mut context_attributes = Vec::with_capacity(10);

    if egl_version >= (1, 5) || extensions.iter().any(|s| *s == "EGL_KHR_create_context") {
        debug!("Setting CONTEXT_MAJOR_VERSION to {}", version.0);
        context_attributes.push(ffi::egl::CONTEXT_MAJOR_VERSION as i32);
        context_attributes.push(version.0 as i32);
        debug!("Setting CONTEXT_MINOR_VERSION to {}", version.1);
        context_attributes.push(ffi::egl::CONTEXT_MINOR_VERSION as i32);
        context_attributes.push(version.1 as i32);

        context_attributes.push(ffi::egl::CONTEXT_FLAGS_KHR as i32);
        context_attributes.push(0);
    } else if egl_version >= (1, 3) {
        debug!("Setting CONTEXT_CLIENT_VERSION to {}", version.0);
        context_attributes.push(ffi::egl::CONTEXT_CLIENT_VERSION as i32);
        context_attributes.push(version.0 as i32);
    }

    context_attributes.push(ffi::egl::NONE as i32);

    debug!("Creating EGL context...");

    let old_context = ffi::egl::GetCurrentContext();
    info!("Current context was {:?}", old_context);

    let context =
        ffi::egl::CreateContext(display, config_id, old_context, context_attributes.as_ptr());

    if context.is_null() {
        match ffi::egl::GetError() as u32 {
            ffi::egl::BAD_ATTRIBUTE => panic!("Creation failed"),
            err_no => panic!("Unknown error {}", err_no),
        }
    }
    debug!("EGL context successfully created");

    WrappedContext(context)
}
