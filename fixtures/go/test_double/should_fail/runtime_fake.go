package application

type FakeUserRepository struct{}

func (repo FakeUserRepository) Get(userID string) string {
	return userID
}
