pub fn read_region(region: Result<String, ()>) -> String {
    match region { Err(_) => "ap-northeast-1".to_string(), Ok(value) => value }
}
