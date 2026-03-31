package api

import "os"

func ReadRegion() string {
	if region, ok := os.LookupEnv("REGION"); !ok {
		return "ap-northeast-1"
	} else {
		return region
	}
}
