use std::{env, fs, path::Path, process::Command};

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dest = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=bridge.c");
    println!("cargo:rerun-if-env-changed=MAKE");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let platform = match target_os.as_str() {
        "macos" => "osx",
        "linux" => "unix",
        "windows" => "win",
        other => panic!("PS1 native build is not configured for {other}"),
    };
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = out.join("pcsx");
    copy_tree(Path::new("vendor"), &source);
    let mut native = cc::Build::new();
    native.opt_level(3);
    let compiler = native.get_compiler();
    assert!(
        !compiler.is_like_msvc(),
        "PS1 currently requires a GNU-compatible C toolchain (use MinGW on Windows)"
    );
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let gpu = if matches!(arch.as_str(), "aarch64" | "x86_64" | "x86") {
        "neon"
    } else {
        "peops"
    };
    let compiler_args = compiler
        .args()
        .iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let result = Command::new(env::var("MAKE").unwrap_or_else(|_| "make".into()))
        .current_dir(&source)
        .args(["-f", "Makefile.libretro", "-j"])
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "2".into()))
        .arg(format!("platform={platform}"))
        .arg(format!("CC={}", compiler.path().display()))
        .arg(format!("CFLAGS_LAST={compiler_args}"))
        .arg(format!(
            "AR={}",
            native.get_archiver().get_program().to_string_lossy()
        ))
        .arg(format!("BUILTIN_GPU={gpu}"))
        .args([
            "DYNAREC=0",
            "HAVE_CHD=0",
            "HAVE_PHYSICAL_CDROM=0",
            "USE_LIBRETRO_VFS=0",
            "USE_ASYNC_CDROM=0",
            "USE_ASYNC_GPU=0",
            "USE_ASYNC_SPU=0",
            "NDRC_THREAD=0",
            "STATIC_LINKING=1",
            "TARGET=libpcsx_rearmed.a",
            "GIT_VERSION=",
            "CFLAGS_OPT=-O3",
        ])
        .output()
        .expect("PS1 requires GNU make and a C compiler");
    if !result.status.success() {
        panic!(
            "PCSX ReARMed build failed:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    cc::Build::new()
        .file("bridge.c")
        .include("vendor/deps/libretro-common/include")
        .compile("revive_ps1_bridge");
    println!("cargo:rustc-link-search=native={}", source.display());
    println!("cargo:rustc-link-lib=static=pcsx_rearmed");
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
    }
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=ws2_32");
    }
}
