//! Gives `matty.exe` its icon and version details on Windows: Explorer,
//! the taskbar and the window's title bar all read the icon from the
//! binary, and GPUI loads resource 1 as each window's icon. The icon is
//! `packaging/icons/matty.ico`, which `packaging/icons/make.py` draws.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let icon = manifest_dir.join("../../packaging/icons/matty.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let icon = icon.canonicalize().expect("packaging/icons/matty.ico exists");

    // A Windows version is four numbers, so a pre-release suffix is dropped.
    let version = env::var("CARGO_PKG_VERSION").expect("set by cargo");
    let mut parts = version
        .split(['.', '-', '+'])
        .take(3)
        .map(|part| part.parse::<u16>().unwrap_or(0));
    let [major, minor, patch] = [(); 3].map(|()| parts.next().unwrap_or(0));

    let rc = format!(
        r#"#include <winver.h>

1 ICON "{icon}"

VS_VERSION_INFO VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Karya Labs"
      VALUE "FileDescription", "Matty"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "matty"
      VALUE "OriginalFilename", "matty.exe"
      VALUE "ProductName", "Matty"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        // rc.exe reads a string's backslashes as escapes.
        icon = icon.display().to_string().replace(r"\\?\", "").replace('\\', r"\\"),
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("set by cargo")).join("matty.rc");
    fs::write(&out, rc).expect("write matty.rc");
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_optional()
        .expect("compile matty.rc");
}
