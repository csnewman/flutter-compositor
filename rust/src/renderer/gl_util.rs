use smithay::wayland::shm::BufferData;

use crate::renderer::gl;
use log::info;
use std::borrow::Cow;
use std::ffi::c_void;
use std::mem;
use std::os::raw::c_uint;

pub fn upload_texture(gl: gl::Gl, data: BufferData, pool: &[u8]) -> u32 {
    unsafe {
        let offset = data.offset as usize;
        let width = data.width as usize;
        let height = data.height as usize;
        let stride = data.stride as usize;

        // TODO: compute from data.format
        let pixelsize = 4;
        assert!(offset + (height - 1) * stride + width * pixelsize <= pool.len());

        info!(
            "data= offset={} width={} height={} stride={} format={:?} len={}",
            data.offset,
            data.width,
            data.height,
            data.stride,
            data.format,
            pool.len()
        );

        let id: gl::types::GLuint = 0;
        gl.GenTextures(1, mem::transmute(&id));
        info!("stage 1 = {}", gl.GetError());

        gl.BindTexture(gl::TEXTURE_2D, id);

        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_R, gl::CLAMP_TO_EDGE as i32);

        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);

        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_BASE_LEVEL, 0);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAX_LEVEL, 0);

        gl.PixelStorei(gl::UNPACK_ROW_LENGTH, data.stride / 4);

        gl.TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA8 as i32,
            data.width,
            data.height,
            0,
            gl::RGBA as u32,
            gl::UNSIGNED_BYTE,
            pool.as_ptr() as *const c_void,
        );

        gl.PixelStorei(gl::UNPACK_ROW_LENGTH, 0);

        info!("stage 3 = {}", gl.GetError());

        id
    }
}
