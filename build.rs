fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {}", e);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::path::Path;
        let info_plist_path = Path::new("Info.plist").canonicalize().expect("Info.plist must exist");
        println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{}", info_plist_path.display());
    }
}
