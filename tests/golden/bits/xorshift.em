fn main() {
    x: u64 = 88172645463325252
    for _ in 0..10 {
        x = x ^ (x << 13)
        x = x ^ (x >> 7)
        x = x ^ (x << 17)
        print(x)
    }
}
