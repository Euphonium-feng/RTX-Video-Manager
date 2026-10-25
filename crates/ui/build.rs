fn main() {
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=../../assets/rtx-hdr-console.ico");
    println!("cargo:rerun-if-changed=../../assets/app.manifest");

    embed_resource::compile("resources.rc", embed_resource::NONE);
}
