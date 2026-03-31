pub struct FakeUserRepository;

impl FakeUserRepository {
    pub fn get(&self, user_id: &str) -> &str {
        user_id
    }
}
