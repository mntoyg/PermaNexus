fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let proto_file = proto_root.join("actuator.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());
    println!("cargo:rerun-if-env-changed=PROTOC");

    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(&[proto_file], &[proto_root])?;

    Ok(())
}
