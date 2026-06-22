use std::fs;
use std::path::Path;

fn install_hook(repo_root: &Path, name: &str) {
    let hook_src = repo_root.join("scripts").join(name);
    let hook_dst = repo_root.join(".git").join("hooks").join(name);

    if !hook_src.exists() {
        return;
    }

    let src_content = fs::read_to_string(&hook_src).unwrap_or_default();
    let dst_content = fs::read_to_string(&hook_dst).unwrap_or_default();

    if src_content == dst_content {
        return;
    }

    let Some(hook_parent) = hook_dst.parent() else {
        return;
    };
    let _ = fs::create_dir_all(hook_parent);
    if fs::copy(&hook_src, &hook_dst).is_err() {
        return;
    }
    // racy on Windows but fine for dev machines
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&hook_dst, fs::Permissions::from_mode(0o755));
    }
    println!("cargo:warning=Installed git {name} hook");
}

fn main() {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Some(repo_root) = Path::new(&manifest_dir).parent() else {
        return;
    };

    for hook in ["pre-commit", "pre-push"] {
        install_hook(repo_root, hook);
    }
}
