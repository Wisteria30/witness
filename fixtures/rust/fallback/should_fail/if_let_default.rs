pub fn read_region(region: Option<String>) -> String {
    if let None = region { return "ap-northeast-1".to_string(); }
    region.unwrap()
}
