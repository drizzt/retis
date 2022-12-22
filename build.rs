use std::{
    env,
    fs::{create_dir_all, File},
    io::Write,
    process::Command,
};

use libbpf_cargo::SkeletonBuilder;
use memmap2::Mmap;

const FILTER_INCLUDE_PATH: &str = "src/core/filters/packets/bpf/include";
const BINDGEN_HEADER: &str = "src/core/bpf_sys/include/bpf-sys.h";
const INCLUDE_PATHS: &[&str] = &[
    "src/core/probe/kernel/bpf/include",
    "src/core/probe/user/bpf/include",
    "src/core/events/bpf/include",
    // Taking errno.h from libc instead of linux headers.
    // TODO: Remove when we fix proper header dependencies.
    "/usr/include/x86_64-linux-gnu",
];

// Not super fail safe (not using Path).
fn get_paths(source: &str) -> (String, String) {
    let (dir, file) = source.rsplit_once('/').unwrap();
    let (name, _) = file.split_once('.').unwrap();

    let out = format!("{dir}/.out");
    create_dir_all(out.as_str()).unwrap();

    (out, name.to_string())
}

fn build_hook(source: &str) {
    let (out, name) = get_paths(source);
    let output = format!("{}/{}.o", out.as_str(), name);
    let skel = format!("{}/{}.rs", out.as_str(), name);

    if let Err(e) = SkeletonBuilder::new()
        .source(source)
        .obj(output.as_str())
        .clang_args(
            INCLUDE_PATHS
                .iter()
                .map(|x| format!("-I{x} "))
                .collect::<String>(),
        )
        .build()
    {
        panic!("{}", e);
    }

    let obj = File::open(output).unwrap();
    let obj: &[u8] = &unsafe { Mmap::map(&obj).unwrap() };

    let mut rs = File::create(skel).unwrap();
    write!(
        rs,
        r#"
           pub(crate) const DATA: &[u8] = &{obj:?};
           "#
    )
    .unwrap();

    println!("cargo:rerun-if-changed={source}");
}

fn build_probe(source: &str) {
    let (out, name) = get_paths(source);
    let skel = format!("{out}/{name}.skel.rs");

    if let Err(e) = SkeletonBuilder::new()
        .source(source)
        .clang_args(
            INCLUDE_PATHS
                .iter()
                .map(|x| format!("-I{x} "))
                .collect::<String>(),
        )
        .build_and_generate(skel)
    {
        panic!("{}", e);
    }

    println!("cargo:rerun-if-changed={source}");
}

fn gen_bindings() {
    let (inc_path, _) = BINDGEN_HEADER.rsplit_once('/').unwrap();

    bindgen::Builder::default()
        .header(BINDGEN_HEADER)
        .clang_arg(format!("-I{inc_path}"))
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .layout_tests(cfg!(feature = "test_bindgen_layout"))
        .generate()
        .expect("Failed during bindings generation")
        .write_to_file(format!("{}/bpf_gen.rs", env::var("OUT_DIR").unwrap()))
        .expect("Failed writing bindings");
}

fn build_extract_stub(source: &str) {
    let (out, name) = get_paths(source);
    let output = format!("{}/{}.o", out.as_str(), name);
    let btf = format!("{out}/{name}.BTF");
    let stub_ext = format!("{}/{}.rs", out.as_str(), name);

    if let Err(e) = SkeletonBuilder::new()
        .source(source)
        .obj(output.as_str())
        .clang_args(format!("-I{FILTER_INCLUDE_PATH} "))
        .build()
    {
        panic!("{}", e);
    }

    if !Command::new("llvm-objcopy")
        .args([
            "-O",
            "binary",
            "--set-section-flags",
            ".BTF=alloc",
            "-j",
            ".BTF",
            output.as_str(),
        ])
        .arg(&btf)
        .status()
        .expect("Failed to extract .BTF from stub ELF")
        .success()
    {
        panic!("Binutils failed to generate BTF file");
    }

    let obj = File::open(&btf).unwrap();
    let obj: &[u8] = &unsafe { Mmap::map(&obj).unwrap() };

    let mut rs = File::create(stub_ext).unwrap();
    write!(rs, r#"pub(crate) const BTF: &[u8] = &{obj:?};"#).unwrap();

    println!("cargo:rerun-if-changed={source}");
}

fn main() {
    gen_bindings();

    // core::probe::kernel
    build_probe("src/core/probe/kernel/bpf/kprobe.bpf.c");
    build_probe("src/core/probe/kernel/bpf/raw_tracepoint.bpf.c");
    build_probe("src/core/probe/user/bpf/usdt.bpf.c");

    build_hook("src/module/skb/bpf/skb_hook.bpf.c");
    build_hook("src/module/skb_tracking/bpf/tracking_hook.bpf.c");
    build_hook("src/module/ovs/bpf/main_hook.bpf.c");

    build_extract_stub("src/core/filters/packets/bpf/stub.bpf.c");

    for inc in INCLUDE_PATHS.iter() {
        println!("cargo:rerun-if-changed={inc}");
    }

    println!("cargo:rerun-if-changed={BINDGEN_HEADER}");
}
