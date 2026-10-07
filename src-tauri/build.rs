fn main() {
    if std::env::var_os("CARGO_FEATURE_SPEEX").is_some() {
        let root = "../vendor/speexdsp";
        let mut build = cc::Build::new();
        build
            .include(format!("{root}/include"))
            .include(format!("{root}/libspeexdsp"))
            .include(format!("{root}/win32"))
            .define("HAVE_CONFIG_H", None)
            .define("_USE_MATH_DEFINES", None)
            .warnings(false);
        for source in [
            "preprocess.c",
            "fftwrap.c",
            "smallft.c",
            "filterbank.c",
            "mdf.c",
        ] {
            build.file(format!("{root}/libspeexdsp/{source}"));
        }
        build.compile("luna_speexdsp");
        println!("cargo:rerun-if-changed={root}/libspeexdsp");
        println!("cargo:rerun-if-changed={root}/include");
    }
    tauri_build::build()
}
