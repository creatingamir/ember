package main

import "fmt"

func main() {
	var count int64 = 0
	var last int64 = 0
	for n := int64(2); n < 100000; n++ {
		isPrime := true
		d := int64(2)
		for d*d <= n {
			if n%d == 0 {
				isPrime = false
				break
			}
			d++
		}
		if isPrime {
			count++
			last = n
			if count <= 10 {
				fmt.Println(n)
			}
		}
	}
	fmt.Println(last)
	fmt.Println(count)
}
