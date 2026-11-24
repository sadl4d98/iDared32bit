/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::path::Path;

fn main() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = package_root.join("../../..");
    let mgba_root = workspace_root.join("vendor/mgba_arm");

    let mut build = cc::Build::new();
    build
        .std("c11")
        // VFPv3 does not fuse multiply-accumulate, so VMLA must round the
        // product before adding it. GCC contracts by default.
        .flag_if_supported("-ffp-contract=off")
        // mGBA's ALU relies on signed arithmetic wrapping, as its own build
        // does with this flag. Without it, overflow is undefined behaviour
        // and the compiler may fold away the overflow checks behind the V
        // flag: NEGS of 0x80000000 came out with V clear.
        .flag("-fwrapv")
        // vfp.c changes the host's rounding mode to match FPSCR's, which the
        // compiler must not assume is always round-to-nearest.
        .flag_if_supported("-frounding-math")
        .include(mgba_root.join("include"))
        .file(package_root.join("lib.c"))
        .file(package_root.join("vfp.c"))
        .file(package_root.join("neon.c"))
        .file(package_root.join("cp15.c"))
        .file(mgba_root.join("src/arm.c"))
        .file(mgba_root.join("src/isa-arm.c"))
        .file(mgba_root.join("src/isa-thumb.c"))
        .file(mgba_root.join("src/isa-thumb2.c"));

    // Profile-guided optimisation of the interpreter, which is where nearly
    // all of a game's time goes. TOUCHHLE_PGO_GENERATE=1 builds it to record
    // which paths are hot (to a .profraw file per run, see
    // dev-scripts/pgo-profile.sh); a merged profile in pgo/ is then used
    // whenever the compiler is clang, as Xcode's is.
    let profile = package_root.join("pgo/interpreter.profdata");
    let is_clang = build.get_compiler().is_like_clang();
    println!("cargo:rerun-if-env-changed=TOUCHHLE_PGO_GENERATE");
    println!("cargo:rerun-if-changed={}", profile.to_str().unwrap());
    if std::env::var_os("TOUCHHLE_PGO_GENERATE").is_some() {
        assert!(is_clang, "TOUCHHLE_PGO_GENERATE needs clang (set CC=clang)");
        build
            .flag("-fprofile-instr-generate")
            .define("TOUCHHLE_PGO_GENERATE", None);
        link_profile_runtime(&build);
    } else if is_clang && profile.exists() {
        build
            .flag(format!("-fprofile-instr-use={}", profile.to_str().unwrap()))
            // A profile from another build of the same source still applies
            // to the functions that match; the rest are left as they are.
            .flag("-Wno-profile-instr-out-of-date")
            .flag("-Wno-profile-instr-unprofiled")
            .flag_if_supported("-Wno-backend-plugin");
    }

    build.compile("mgba_wrapper");

    for file in ["lib.c", "vfp.c", "cp15.c", "wrapper.h"] {
        println!(
            "cargo:rerun-if-changed={}",
            package_root.join(file).to_str().unwrap()
        );
    }
    println!("cargo:rerun-if-changed={}", mgba_root.to_str().unwrap());
}

/// The instrumented interpreter needs clang's profiling runtime linked in.
fn link_profile_runtime(build: &cc::Build) {
    let output = build
        .get_compiler()
        .to_command()
        .arg("--print-libgcc-file-name")
        .arg("--rtlib=compiler-rt")
        .output()
        .expect("couldn't ask clang where its runtime is");
    let builtins = String::from_utf8(output.stdout).unwrap();
    let dir = Path::new(builtins.trim()).parent().unwrap();
    let name = Path::new(builtins.trim())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace("builtins", "profile");
    let lib = name.trim_start_matches("lib").trim_end_matches(".a");
    println!("cargo:rustc-link-search=native={}", dir.to_str().unwrap());
    println!("cargo:rustc-link-lib=static={}", lib);
}
