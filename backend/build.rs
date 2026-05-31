use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = Path::new(&manifest_dir).parent().unwrap();
    let hook_src = repo_root.join("scripts").join("pre-commit");
    let hook_dst = repo_root.join(".git").join("hooks").join("pre-commit");

    if !hook_src.exists() {
        return;
    }

    let src_content = fs::read_to_string(&hook_src).unwrap_or_default();
    let dst_content = fs::read_to_string(&hook_dst).unwrap_or_default();

    if src_content != dst_content {
        let _ = fs::create_dir_all(hook_dst.parent().unwrap());
        if fs::copy(&hook_src, &hook_dst).is_ok() {
            // racy on Windows but fine for dev machines
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&hook_dst, fs::Permissions::from_mode(0o755));
            }
            println!("cargo:warning=Installed git pre-commit hook");
        }
    }
}
