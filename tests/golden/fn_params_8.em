fn weigh(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {
    return a * 10000000 + b * 1000000 + c * 100000 + d * 10000 + e * 1000 + f * 100 + g * 10 + h
}

fn main() {
    print(weigh(1, 2, 3, 4, 5, 6, 7, 8))
    print(weigh(8, 7, 6, 5, 4, 3, 2, 1))
    print(weigh(1 + 0, 2, 3, 4, 5, 6, 7, 8 - 0))
}
