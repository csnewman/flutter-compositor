extern crate gl_generator;

use bindgen::EnumVariation;
use gl_generator::{Api, Fallbacks, Profile, Registry};
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, io};

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let root_dir = out_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let mut file_output = File::create(&out_path.join("gl_bindings.rs")).unwrap();
    generate_gl_bindings(&mut file_output);
    generate_flutter_bindings(&out_path, &root_dir);
}

fn generate_gl_bindings<W>(dest: &mut W)
where
    W: Write,
{
    let gl_registry = Registry::new(
        Api::Gl,
        (4, 5),
        Profile::Compatibility,
        Fallbacks::None,
        vec![
            "GL_AMD_depth_clamp_separate",
            "GL_APPLE_vertex_array_object",
            "GL_ARB_bindless_texture",
            "GL_ARB_buffer_storage",
            "GL_ARB_compute_shader",
            "GL_ARB_copy_buffer",
            "GL_ARB_debug_output",
            "GL_ARB_depth_texture",
            "GL_ARB_direct_state_access",
            "GL_ARB_draw_buffers",
            "GL_ARB_ES2_compatibility",
            "GL_ARB_ES3_compatibility",
            "GL_ARB_ES3_1_compatibility",
            "GL_ARB_ES3_2_compatibility",
            "GL_ARB_framebuffer_sRGB",
            "GL_ARB_geometry_shader4",
            "GL_ARB_gpu_shader_fp64",
            "GL_ARB_gpu_shader_int64",
            "GL_ARB_invalidate_subdata",
            "GL_ARB_multi_draw_indirect",
            "GL_ARB_occlusion_query",
            "GL_ARB_pixel_buffer_object",
            "GL_ARB_robustness",
            "GL_ARB_shader_image_load_store",
            "GL_ARB_shader_objects",
            "GL_ARB_texture_buffer_object",
            "GL_ARB_texture_float",
            "GL_ARB_texture_multisample",
            "GL_ARB_texture_rg",
            "GL_ARB_texture_rgb10_a2ui",
            "GL_ARB_transform_feedback3",
            "GL_ARB_vertex_buffer_object",
            "GL_ARB_vertex_shader",
            "GL_ATI_draw_buffers",
            "GL_ATI_meminfo",
            "GL_EXT_debug_marker",
            "GL_EXT_direct_state_access",
            "GL_EXT_framebuffer_blit",
            "GL_EXT_framebuffer_multisample",
            "GL_EXT_framebuffer_object",
            "GL_EXT_framebuffer_sRGB",
            "GL_EXT_gpu_shader4",
            "GL_EXT_packed_depth_stencil",
            "GL_EXT_provoking_vertex",
            "GL_EXT_texture_array",
            "GL_EXT_texture_buffer_object",
            "GL_EXT_texture_compression_s3tc",
            "GL_EXT_texture_filter_anisotropic",
            "GL_EXT_texture_integer",
            "GL_EXT_texture_sRGB",
            "GL_EXT_transform_feedback",
            "GL_GREMEDY_string_marker",
            "GL_KHR_robustness",
            "GL_NVX_gpu_memory_info",
            "GL_NV_conditional_render",
            "GL_NV_vertex_attrib_integer_64bit",
        ],
    );

    let gles_registry = Registry::new(
        Api::Gles2,
        (3, 2),
        Profile::Compatibility,
        Fallbacks::None,
        vec![
            "GL_ANGLE_framebuffer_multisample",
            "GL_APPLE_framebuffer_multisample",
            "GL_APPLE_sync",
            "GL_ARM_rgba8",
            "GL_EXT_buffer_storage",
            "GL_EXT_disjoint_timer_query",
            "GL_EXT_multi_draw_indirect",
            "GL_EXT_multisampled_render_to_texture",
            "GL_EXT_occlusion_query_boolean",
            "GL_EXT_primitive_bounding_box",
            "GL_EXT_robustness",
            "GL_KHR_debug",
            "GL_NV_copy_buffer",
            "GL_NV_framebuffer_multisample",
            "GL_NV_internalformat_sample_query",
            "GL_NV_pixel_buffer_object",
            "GL_OES_depth_texture",
            "GL_OES_draw_elements_base_vertex",
            "GL_OES_packed_depth_stencil",
            "GL_OES_primitive_bounding_box",
            "GL_OES_rgb8_rgba8",
            "GL_OES_texture_buffer",
            "GL_OES_texture_npot",
            "GL_OES_vertex_array_object",
            "GL_OES_vertex_type_10_10_10_2",
        ],
    );

    (gl_registry + gles_registry)
        .write_bindings(gl_generator::StructGenerator, dest)
        .unwrap();
}

fn generate_flutter_bindings(out_path: &PathBuf, root_dir: &PathBuf) {
    // Try to find flutter
    let flutter_cmd = which::which("flutter").expect("Unable to find flutter");
    let flutter_root = flutter_cmd.parent().unwrap().parent().unwrap();
    let version_path = flutter_cmd
        .parent()
        .unwrap()
        .join("internal")
        .join("engine.version");
    println!("cargo:rerun-if-changed={}", flutter_root.to_str().unwrap());

    // Read version
    let version = fs::read_to_string(version_path.as_path())
        .expect("Failed to read version file")
        .trim()
        .to_string();

    // Download engine
    let engine_dir = dirs::cache_dir()
        .unwrap()
        .join("flutter-engine")
        .join(&version);
    let header_file = engine_dir.join("flutter_embedder.h");

    if !engine_dir.exists() {
        println!("Downloading flutter engine");
        fs::create_dir_all(&engine_dir);
        let target_zip = engine_dir.join("download.zip");
        let engine_url = format!(
            "https://storage.googleapis.com/flutter_infra/flutter/{}/linux-x64/linux-x64-embedder",
            &version
        );
        download_file(&engine_url, &target_zip);

        println!("Extracting flutter engine");
        extract_zip(&target_zip, &engine_dir);

        // Fetch latest header (one in embedder zip is very outdated)
        println!("Overwriting header");
        download_file("https://raw.githubusercontent.com/flutter/engine/master/shell/platform/embedder/embedder.h", &header_file);

        fs::remove_file(target_zip);
    }

    // Configure linker
    println!(
        "cargo:rustc-link-search=native={}",
        engine_dir.to_str().unwrap()
    );

    let config_dir = root_dir.join(".cargo");
    fs::create_dir(&config_dir);
    let config_file = config_dir.join("config");
    if !config_file.exists() {
        fs::write(
            config_file,
            r#"[target.x86_64-unknown-linux-gnu]
               rustflags = ["-C", "link-args=-Wl,-rpath=$ORIGIN"]
            "#,
        )
        .expect("Failed to write linker config in .cargo/config");
    }

    // Copy engine library
    fs::copy(
        engine_dir.join("libflutter_engine.so"),
        out_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("libflutter_engine.so"),
    )
    .expect("Failed to copy engine lib");

    fs::copy(
        flutter_cmd
            .parent()
            .unwrap()
            .join("cache")
            .join("artifacts")
            .join("engine")
            .join("linux-x64")
            .join("icudtl.dat"),
        out_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("icudtl.dat"),
    )
    .expect("Failed to copy icudtl.dat");

    // Generate header bindings
    let bindings = bindgen::Builder::default()
        .header(header_file.to_str().unwrap())
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("flutter_bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn download_file<P: AsRef<Path>>(url: &str, target: P) {
    let mut resp = reqwest::get(url).expect("Failed to fetch file");

    let mut out = File::create(target).expect("Failed to create download output");
    io::copy(&mut resp, &mut out).expect("failed to copy content");
}

fn extract_zip<P: AsRef<Path>>(source: P, target: &PathBuf) {
    let mut archive = zip::ZipArchive::new(fs::File::open(source).unwrap()).unwrap();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = target.join(file.sanitized_name());

        if (&*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath).unwrap();
        } else {
            println!(
                "File {} extracted to \"{}\" ({} bytes)",
                i,
                outpath.as_path().display(),
                file.size()
            );
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = fs::File::create(&outpath).unwrap();
            io::copy(&mut file, &mut outfile).unwrap();
        }

        // Get and Set permissions
        if let Some(mode) = file.unix_mode() {
            fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
        }
    }
}
