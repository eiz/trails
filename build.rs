use anyhow::bail;
use eiz::com::ComPtr;
use std::{env, ffi::OsStr, fs, os::windows::prelude::OsStrExt, path::Path, ptr};
use winapi::um::{
    d3dcommon::ID3DBlob,
    d3dcompiler::{
        D3DCompileFromFile, D3DCOMPILE_ENABLE_STRICTNESS, D3DCOMPILE_WARNINGS_ARE_ERRORS,
        D3D_COMPILE_STANDARD_FILE_INCLUDE,
    },
};

fn osstr_to_wide<S: AsRef<OsStr>>(str: S) -> Vec<u16> {
    str.as_ref()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<u16>>()
}

fn compile_shader<P: AsRef<Path>>(path: P, target: &str, entry: &str) -> anyhow::Result<()> {
    let path = path.as_ref();
    let target = format!["{}\0", target];
    let entry0 = format!["{}\0", entry];
    let mut shader: *mut ID3DBlob = ptr::null_mut();
    let mut errors: *mut ID3DBlob = ptr::null_mut();

    println!["{:?}", path];
    let hr = unsafe {
        D3DCompileFromFile(
            osstr_to_wide(path).as_ptr(),
            ptr::null(),
            D3D_COMPILE_STANDARD_FILE_INCLUDE,
            entry0.as_ptr() as *const _,
            target.as_ptr() as *const _,
            D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_WARNINGS_ARE_ERRORS,
            0,
            &mut shader,
            &mut errors,
        )
    };

    let shader = if shader.is_null() {
        None
    } else {
        unsafe { Some(ComPtr::from_raw_unchecked(shader)) }
    };
    let errors = if errors.is_null() {
        None
    } else {
        unsafe { Some(ComPtr::from_raw_unchecked(errors)) }
    };

    if hr != 0 || errors.is_some() {
        if let Some(errors) = errors {
            let err_slice = unsafe {
                std::slice::from_raw_parts(
                    errors.GetBufferPointer() as *const u8,
                    errors.GetBufferSize() - 1,
                )
            };
            let err = String::from_utf8_lossy(err_slice);

            eprintln!["=== SHADER ERROR ===\nPath: {:?}\n{}", path, err];
            bail!["Failed to compile shader {:?}", path];
        }

        bail![
            "Failed to compile shader {:?} with COM hr=0x{:08X}",
            path,
            hr
        ];
    }

    let shader = shader.unwrap();
    let mut cso_name = path.to_path_buf();
    let cso_dir = Path::new(&env::var("OUT_DIR").unwrap()).join(path.parent().unwrap());

    cso_name.set_file_name(format![
        "{}.{}.cso",
        cso_name.file_stem().unwrap().to_str().unwrap(),
        entry
    ]);
    fs::create_dir_all(&cso_dir)?;
    fs::write(cso_dir.join(cso_name.file_name().unwrap()), unsafe {
        std::slice::from_raw_parts(
            shader.GetBufferPointer() as *const u8,
            shader.GetBufferSize(),
        )
    })?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    compile_shader("shader/slime.hlsl", "cs_5_0", "advance_agents")?;
    compile_shader("shader/slime.hlsl", "cs_5_0", "decay_and_diffuse")?;
    compile_shader("shader/scrgb_to_hdr10.hlsl", "cs_5_0", "convert")?;

    let build_files = &[
        "shader/common.inc",
        "scrgb_to_hdr10.hlsl",
        "shader/slime.hlsl",
    ];

    for file in build_files {
        println!["cargo:rerun-if-changed={}", file];
    }

    Ok(())
}
