fn main() {
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=../../assets/rtx-video-manager.ico");

    embed_resource::compile("resources.rc", embed_resource::NONE);
}
