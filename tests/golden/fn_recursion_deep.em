fn sum_to(n: i64) -> i64 {
    if n == 0 {
        return 0
    }
    return n + sum_to(n - 1)
}

fn main() {
    print(sum_to(10000))
}
