fn main() {
    println!("cargo:rerun-if-changed=../../pistonport/launcher/pistonport.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("../../pistonport/launcher/pistonport.ico")
            .set("ProductName", "PistonPort")
            .set("FileDescription", "PistonPort");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Could not embed the PistonPort icon: {error}");
        }
    }
}
