fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "F4");
        res.set("FileDescription", "F4 Text Editor");
        res.compile().unwrap();
    }
}
