fn main() {
    let path = "../../kernel/target/x86_64/release/deps";
    let lib = "rcore-dyn";

    println!("cargo:rustc-link-search=all={}", path);

}