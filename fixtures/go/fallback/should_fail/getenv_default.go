package api

import "os"

func ReadRegion() string {
	if region := os.Getenv("REGION"); region == "" {
		return "ap-northeast-1"
	}
	return os.Getenv("REGION")
}
