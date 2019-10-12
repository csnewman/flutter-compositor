use libc::{c_char, c_uint, c_void};
use log::{debug, error, info};
use smithay::backend::graphics::gl::GLGraphicsBackend;
use std::ffi::CStr;

use crate::flutter::ffi;
use core::ptr;
use std::sync::Mutex;

use smithay::backend::egl::ffi as egl_ffi;

use crate::renderer::gl;
use crate::FlutterCompositorRef;
use flutter_engine_sys::FlutterOpenGLTexture;

pub unsafe extern "C" fn present(user_data: *mut c_void) -> bool {
    debug!("present");
    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    instance.backend.present()
}

pub unsafe extern "C" fn make_current(user_data: *mut c_void) -> bool {
    debug!("make_current");
    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    debug!("got lock");

    let res = instance.backend.make_current();
    debug!("make_current: end");
    res
}

pub unsafe extern "C" fn clear_current(user_data: *mut c_void) -> bool {
    debug!("clear_current");
    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    instance.backend.clear_current()
}

pub extern "C" fn fbo_callback(_user_data: *mut c_void) -> c_uint {
    debug!("fbo_callback");
    0
}

pub unsafe extern "C" fn make_resource_current(user_data: *mut c_void) -> bool {
    debug!("make_resource_current");
    //    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    //
    //    if !instance.resource_context.apply_context(&instance.display) {
    //        error!("Failed to apply resource context");
    //        return false;
    //    }
    //    true

    false
}

pub unsafe extern "C" fn gl_proc_resolver(
    user_data: *mut c_void,
    proc: *const c_char,
) -> *mut c_void {
    //    debug!("gl_proc_resolver");
    let c_str: &CStr = CStr::from_ptr(proc as _);
    let str_slice: &str = c_str.to_str().unwrap();

    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    instance.backend.gl_proc_resolver(str_slice)
}

pub unsafe extern "C" fn platform_message_callback(
    platform_message: *const flutter_engine_sys::FlutterPlatformMessage,
    user_data: *mut c_void,
) {
    debug!("platform_message_callback");

    //    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    //    instance.channel_registry.borrow_mut().handle((*platform_message).into());
}

pub extern "C" fn root_isolate_create_callback(_user_data: *mut c_void) {
    debug!("root_isolate_create_callback");
}

//pub type VoidCallback =
//::std::option::Option<unsafe extern "C" fn(arg1: *mut ::std::os::raw::c_void)>;
//#[repr(C)]
//#[derive(Debug, Copy, Clone)]
//pub struct FlutterOpenGLTexture {
//    pub target: u32,
//    pub name: u32,
//    pub format: u32,
//    pub user_data: *mut ::std::os::raw::c_void,
//    pub destruction_callback: VoidCallback,
//}

pub unsafe extern "C" fn gl_external_texture_frame_callback(
    user_data: *mut c_void,
    texture_id: i64,
    width: usize,
    height: usize,
    texture: *mut FlutterOpenGLTexture,
) -> bool {
    //    info!("gl_external_texture_frame_callback texture_id={} width={} height={}", texture_id, width, height);

    //    let instance = FlutterCompositorRef::get_from_mutex_ptr(user_data as _);
    //
    ////    let value = instance.val.borrow().clone();
    //    let value = 0;
    //
    ////    info!("value {}", value);
    //
    //    (*texture) = FlutterOpenGLTexture {
    //        target: gl::TEXTURE_2D,  // 0x0DE1, //TEXTURE_2D
    //        name: value,
    //        format: gl::RGBA8, //RGBA8   Argb8888
    //        user_data,
    //        destruction_callback: None
    //    };
    //
    //    true

    false
}
