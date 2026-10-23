#[path = "build/ffmpeg.rs"]
mod ffmpeg;
#[path = "build/onnxruntime.rs"]
mod onnxruntime;

fn main() {
    // GPU Acceleration Detection and Build Guidance
    detect_and_report_gpu_capabilities();

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Foundation");

        // Let the enhanced_macos crate handle its own Swift compilation
        // The swift-rs crate build will be handled in the enhanced_macos crate's build.rs
    }

    // Download and bundle FFmpeg binary at build-time to eliminate runtime download delays
    ffmpeg::ensure_ffmpeg_binary();
    onnxruntime::ensure_onnxruntime_runtime();
    embed_google_oauth_client();

    tauri_build::build()
}

/// Builds the Google OAuth client into the app, so "Connect Google Calendar" works
/// without users creating their own. Read from SEGRA_GOOGLE_CLIENT_ID /
/// SEGRA_GOOGLE_CLIENT_SECRET, or from google-oauth.local.json (git-ignored):
/// {"client_id": "...apps.googleusercontent.com", "client_secret": "GOCSPX-..."}.
/// Without either, users enter their own client in Settings.
fn embed_google_oauth_client() {
    const FILE: &str = "google-oauth.local.json";
    println!("cargo:rerun-if-changed={}", FILE);
    println!("cargo:rerun-if-env-changed=SEGRA_GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=SEGRA_GOOGLE_CLIENT_SECRET");

    let from_env = std::env::var("SEGRA_GOOGLE_CLIENT_ID")
        .ok()
        .zip(std::env::var("SEGRA_GOOGLE_CLIENT_SECRET").ok());
    let from_file = || {
        let text = std::fs::read_to_string(FILE).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        Some((
            json.get("client_id")?.as_str()?.to_string(),
            json.get("client_secret")?.as_str()?.to_string(),
        ))
    };

    match from_env.or_else(from_file) {
        Some((id, secret)) if !id.trim().is_empty() && !secret.trim().is_empty() => {
            println!("cargo:rustc-env=SEGRA_GOOGLE_CLIENT_ID={}", id.trim());
            println!("cargo:rustc-env=SEGRA_GOOGLE_CLIENT_SECRET={}", secret.trim());
        }
        _ => println!("cargo:warning=No built-in Google OAuth client: users must enter their own in Settings"),
    }
}

/// Detects GPU acceleration capabilities and provides build guidance
fn detect_and_report_gpu_capabilities() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    println!("cargo:warning=🚀 Building Meetily for: {}", target_os);

    match target_os.as_str() {
        "macos" => {
            println!("cargo:warning=✅ macOS: Metal GPU acceleration ENABLED by default");
            #[cfg(feature = "coreml")]
            println!("cargo:warning=✅ CoreML acceleration ENABLED");
        }
        "windows" => {
            if cfg!(feature = "cuda") {
                println!("cargo:warning=✅ Windows: CUDA GPU acceleration ENABLED");
            } else if cfg!(feature = "vulkan") {
                println!("cargo:warning=✅ Windows: Vulkan GPU acceleration ENABLED");
            } else if cfg!(feature = "openblas") {
                println!("cargo:warning=✅ Windows: OpenBLAS CPU optimization ENABLED");
            } else {
                println!("cargo:warning=⚠️  Windows: Using CPU-only mode (no GPU or BLAS acceleration)");
                println!("cargo:warning=💡 For NVIDIA GPU: cargo build --release --features cuda");
                println!("cargo:warning=💡 For AMD/Intel GPU: cargo build --release --features vulkan");
                println!("cargo:warning=💡 For CPU optimization: cargo build --release --features openblas");

                // Try to detect NVIDIA GPU
                if which::which("nvidia-smi").is_ok() {
                    println!("cargo:warning=🎯 NVIDIA GPU detected! Consider rebuilding with --features cuda");
                }
            }
        }
        "linux" => {
            if cfg!(feature = "cuda") {
                println!("cargo:warning=✅ Linux: CUDA GPU acceleration ENABLED");
            } else if cfg!(feature = "vulkan") {
                println!("cargo:warning=✅ Linux: Vulkan GPU acceleration ENABLED");
            } else if cfg!(feature = "hipblas") {
                println!("cargo:warning=✅ Linux: AMD ROCm (HIP) acceleration ENABLED");
            } else if cfg!(feature = "openblas") {
                println!("cargo:warning=✅ Linux: OpenBLAS CPU optimization ENABLED");
            } else {
                println!("cargo:warning=⚠️  Linux: Using CPU-only mode (no GPU or BLAS acceleration)");
                println!("cargo:warning=💡 For NVIDIA GPU: cargo build --release --features cuda");
                println!("cargo:warning=💡 For AMD GPU: cargo build --release --features hipblas");
                println!("cargo:warning=💡 For other GPUs: cargo build --release --features vulkan");
                println!("cargo:warning=💡 For CPU optimization: cargo build --release --features openblas");

                // Try to detect NVIDIA GPU
                if which::which("nvidia-smi").is_ok() {
                    println!("cargo:warning=🎯 NVIDIA GPU detected! Consider rebuilding with --features cuda");
                }

                // Try to detect AMD GPU
                if which::which("rocm-smi").is_ok() {
                    println!("cargo:warning=🎯 AMD GPU detected! Consider rebuilding with --features hipblas");
                }
            }
        }
        _ => {
            println!("cargo:warning=ℹ️  Unknown platform: {}", target_os);
        }
    }

    // Performance guidance
    if !cfg!(feature = "cuda") && !cfg!(feature = "vulkan") && !cfg!(feature = "hipblas") && !cfg!(feature = "openblas") && target_os != "macos" {
        println!("cargo:warning=📊 Performance: CPU-only builds are significantly slower than GPU/BLAS builds");
        println!("cargo:warning=📚 See README.md for GPU/BLAS setup instructions");
    }
}
