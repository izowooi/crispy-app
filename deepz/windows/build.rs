fn main() {
    // 아이콘과 매니페스트(고해상도 대응, 최신 컨트롤 스타일)를 exe 에 넣는다.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        embed_resource::compile("deepz.rc", embed_resource::NONE);
    }
}
