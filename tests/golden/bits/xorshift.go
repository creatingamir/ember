package main

import "fmt"

func main() {
	var x uint64 = 88172645463325252
	for i := 0; i < 10; i++ {
		x ^= x << 13
		x ^= x >> 7
		x ^= x << 17
		fmt.Println(x)
	}
}
