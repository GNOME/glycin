use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub fn bench_name(path: &Path) -> String {
    path.file_name().unwrap().display().to_string()
}

pub fn test_images() -> BTreeSet<std::path::PathBuf> {
    let mut paths = vec![
        PathBuf::from("test-images/images/color/color.avif"),
        PathBuf::from("test-images/images/color/color.exr"),
        PathBuf::from("test-images/images/color/color.jpg"),
        PathBuf::from("test-images/images/color/color.jxl"),
        PathBuf::from("test-images/images/color/color.png"),
        PathBuf::from("test-images/images/color/color.svg"),
        PathBuf::from("test-images/images/color/color.webp"),
        PathBuf::from("test-images/images/tiny/tiny.png"),
    ];

    let download = [
        (
            "gnome-bg-50-blendpills-l.jxl",
            "https://gitlab.gnome.org/GNOME/gnome-backgrounds/-/raw/00bdf1cb/backgrounds/blendpills-l.jxl",
        ),
        (
            "gnome-bg-50-morphogenesis-l.svg",
            "https://gitlab.gnome.org/GNOME/gnome-backgrounds/-/raw/00bdf1cb/backgrounds/morphogenesis-l.svg",
        ),
    ];

    if !Path::new("cache").is_dir() {
        std::fs::create_dir("cache").unwrap();
    }

    for (filename, url) in download {
        let path = PathBuf::from(format!("cache/{filename}"));
        if !Path::new(&path).is_file() {
            eprintln!("Downloading image from <{url}> …");
            std::process::Command::new("curl")
                .args([url, "--output"])
                .arg(&path)
                .status()
                .unwrap();
        }

        paths.push(path);
    }

    BTreeSet::from_iter(paths)
}
