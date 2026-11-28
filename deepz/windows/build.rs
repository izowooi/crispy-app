use std::path::PathBuf;

fn main() {
    // 아이콘과 매니페스트(고해상도 대응, 최신 컨트롤 스타일)를 exe 에 넣는다.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        embed_resource::compile("deepz.rc", embed_resource::NONE);
    }

    // 무료 번역 서버의 서명용 값. 저장소에 넣지 않고 빌드할 때만 읽는다. 없으면 무료 번역이 빠진 앱이 된다.
    println!("cargo:rerun-if-env-changed=DEEPZ_APP_SECRET");
    let file = std::env::var("HOME").map(|home| PathBuf::from(home).join("git/.auth/deepz.env")).ok();
    if let Some(file) = &file {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    let secret = std::env::var("DEEPZ_APP_SECRET").ok().or_else(|| {
        let content = std::fs::read_to_string(file?).ok()?;
        content.lines().find_map(|line| line.strip_prefix("DEEPZ_APP_SECRET=").map(|v| v.trim().to_string()))
    });
    let secret = secret.unwrap_or_default();
    if secret.is_empty() {
        println!("cargo:warning=DEEPZ_APP_SECRET 이 없어 무료 번역 없이 빌드합니다.");
    }
    let masked: Vec<String> = secret.bytes().map(|b| (b ^ 90).to_string()).collect();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("app_secret.rs");
    std::fs::write(out, format!("const SECRET_PAD: u8 = 90;\nconst SECRET_MASKED: &[u8] = &[{}];\n", masked.join(", "))).unwrap();
}
