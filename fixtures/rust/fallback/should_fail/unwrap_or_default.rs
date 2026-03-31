pub fn read_region(region: Option<String>) -> String {
    region.unwrap_or_default()
}
