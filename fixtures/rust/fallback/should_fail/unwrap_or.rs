pub fn read_region(region: Option<String>) -> String {
    region.unwrap_or("ap-northeast-1".to_string())
}
