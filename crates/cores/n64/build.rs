use std::{env, fs, path::Path, process::Command};

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dest = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            // Upstream make does not track every included C source/header.
            // Copy all timestamps on rerun so nested-source fixes rebuild too.
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

fn remove_objects(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            remove_objects(&path);
        } else if path.extension().is_some_and(|ext| ext == "o") {
            fs::remove_file(path).unwrap();
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=bridge.c");
    println!("cargo:rerun-if-env-changed=MAKE");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let (platform, filename) = match os.as_str() {
        "macos" => ("osx", "mupen64plus_next_libretro.dylib"),
        "windows" => ("win", "mupen64plus_next_libretro.dll"),
        "linux" => ("unix", "mupen64plus_next_libretro.so"),
        other => panic!("N64 native build is not yet configured for {other}"),
    };
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let source = out.join("mupen");
    copy_tree(Path::new("vendor"), &source);
    // macOS copy_file may preserve timestamps; upstream does not track includes.
    remove_objects(&source);
    fs::copy("bridge.c", source.join("revive_bridge.c")).unwrap();
    let makefile = fs::read_to_string(source.join("Makefile")).unwrap();
    fs::write(
        source.join("Makefile"),
        makefile.replace(
            "OBJECTS     +=",
            "SOURCES_C += ./revive_bridge.c\nOBJECTS     +=",
        ),
    )
    .unwrap();
    let cc = cc::Build::new().get_compiler();
    let cxx = cc::Build::new().cpp(true).get_compiler();
    let result = Command::new(env::var("MAKE").unwrap_or_else(|_| "make".into()))
        .current_dir(&source)
        .arg("-j")
        .arg(env::var("NUM_JOBS").unwrap_or_else(|_| "4".into()))
        .arg(format!("platform={platform}"))
        .arg(format!(
            "ARCH={}",
            env::var("CARGO_CFG_TARGET_ARCH").unwrap()
        ))
        .arg(format!("CC={}", cc.path().display()))
        .arg(format!("CXX={}", cxx.path().display()))
        .args([
            "SYSTEM_ZLIB=1",
            "HAVE_PARALLEL_RDP=0",
            "HAVE_PARALLEL_RSP=0",
            "HAVE_THR_AL=1",
            "LLE=1",
            "WITH_DYNAREC=",
            "GIT_VERSION=6752836-revive",
        ])
        .output()
        .expect("N64 requires GNU make and a C/C++ compiler");
    fs::write(out.join("native-build.log"), &result.stderr).unwrap();
    assert!(
        result.status.success(),
        "N64 native build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    // The private shared image isolates all libretro/dependency globals from PS1.
    // Embed it so installed binaries never need an external core download.
    println!(
        "cargo:rustc-env=REVIVE_N64_LIBRARY={}",
        source.join(filename).display()
    );
}
