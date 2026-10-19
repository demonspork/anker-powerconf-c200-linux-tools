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
    for source in lib_sources {
        let path = format!("{c_dir}/{source}");
        println!("cargo:rerun-if-changed={path}");
        build.file(path);
    }
    println!("cargo:rerun-if-changed=csrc/awcap.c");
    println!("cargo:rerun-if-changed=csrc/awcap.h");
    build.file("csrc/awcap.c");
    build.compile("c200");
}
