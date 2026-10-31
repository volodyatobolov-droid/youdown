use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let icon = manifest.join("assets").join("icon.ico");
    let rc = out.join("icon.rc");
    let obj = out.join("icon.o");

    let version = env!("CARGO_PKG_VERSION");
    let mut parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let v = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    let rc_text = format!(
        "1 ICON \"{icon}\"\n\
         1 VERSIONINFO\n\
         FILEVERSION {v}\n\
         PRODUCTVERSION {v}\n\
         FILEOS 0x40004\n\
         FILETYPE 0x1\n\
         BEGIN\n\
           BLOCK \"StringFileInfo\"\n\
           BEGIN\n\
             BLOCK \"040904b0\"\n\
             BEGIN\n\
               VALUE \"FileDescription\", \"YouDown\"\n\
               VALUE \"FileVersion\", \"{version}\"\n\
               VALUE \"ProductName\", \"YouDown\"\n\
               VALUE \"ProductVersion\", \"{version}\"\n\
               VALUE \"CompanyName\", \"volodyatobolov-droid\"\n\
               VALUE \"OriginalFilename\", \"YouDown.exe\"\n\
               VALUE \"LegalCopyright\", \"MIT\"\n\
             END\n\
           END\n\
           BLOCK \"VarFileInfo\"\n\
           BEGIN\n\
             VALUE \"Translation\", 0x409, 1200\n\
           END\n\
         END\n",
        icon = icon.display().to_string().replace('\\', "\\\\"),
        v = v,
        version = version,
    );
    std::fs::write(&rc, rc_text).expect("write icon.rc");

    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
    let status = std::process::Command::new(windres)
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status()
        .expect("run windres");
    assert!(status.success(), "windres failed");

    println!("cargo:rustc-link-arg={}", obj.display());
}
