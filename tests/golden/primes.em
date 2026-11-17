fn main() {
    count := 0
    last := 0
    for n in 2..100000 {
        is_prime := true
        d := 2
        for d * d <= n {
            if n % d == 0 {
                is_prime = false
                break
            }
            d += 1
        }
        if is_prime {
            count += 1
            last = n
            if count <= 10 {
                print(n)
            }
        }
    }
    print(last)
    print(count)
}
