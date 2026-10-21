fn main() {
    let c_dir = "../src";
    let lib_sources = [
        "c200_vendor.c",
        "c200_controls.c",
        "c200_fov.c",
        "c200_area_metering.c",
        "c200_enable_u8.c",
        "c200_device.c",
    ];

    let mut build = cc::Build::new();
    build
        .include(c_dir)
        .include("csrc")
        .flag("-std=c11")
        .warnings(true);
    // Distro build flags (Arch makepkg, Fedora) add -flto=auto to CFLAGS. GCC LTO
    // objects are GIMPLE bytecode that rustc's linker cannot consume, so the
    // static library would link with every symbol undefined. The cc crate puts
    // environment flags last, so strip the LTO tokens from the environment
    // before it reads them; the Rust side keeps its own LTO.
    strip_lto_from_env();
    build.flag_if_supported("-fno-lto");
    for source in lib_sources {
        let path = format!("{c_dir}/{source}");
        println!("cargo:rerun-if-changed={path}");
        build.file(path);
    }
    println!("cargo:rerun-if-changed=csrc/awcap.c");
    println!("cargo:rerun-if-changed=csrc/compat.c");
    println!("cargo:rerun-if-changed=csrc/awcap.h");
    build.file("csrc/awcap.c");
    build.file("csrc/compat.c");
    build.compile("c200");
}

fn strip_lto_from_env() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let target_upper = target.replace('-', "_").to_uppercase();
    let names = [
        "CFLAGS".to_string(),
        "TARGET_CFLAGS".to_string(),
        format!("CFLAGS_{target}"),
        format!("{target_upper}_CFLAGS"),
    ];
    for name in names {
        println!("cargo:rerun-if-env-changed={name}");
        if let Ok(value) = std::env::var(&name) {
            let kept: Vec<&str> = value
                .split_whitespace()
                .filter(|flag| !flag.starts_with("-flto") && *flag != "-ffat-lto-objects")
                .collect();
            std::env::set_var(&name, kept.join(" "));
        }
    }
}
