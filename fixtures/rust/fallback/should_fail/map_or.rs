pub fn read_region(region: Option<String>) -> usize {
    region.map_or(0, |value| value.len())
}
