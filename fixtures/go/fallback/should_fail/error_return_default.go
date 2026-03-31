package api

type UserPayload struct{}

func ParseUser(err error) UserPayload {
	if err != nil { return UserPayload{} }
	return UserPayload{}
}
