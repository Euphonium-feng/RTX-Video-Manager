fn main() {
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=assets/rtx-hdr-on.ico");
    println!("cargo:rerun-if-changed=assets/rtx-hdr-off.ico");
    println!("cargo:rerun-if-changed=assets/rtx-hdr-error.ico");

    embed_resource::compile("resources.rc", embed_resource::NONE);
}
