fn main() {
    println!("cargo:rerun-if-changed=native/native.c");
    cc::Build::new().file("native/native.c").compile("native");
}
