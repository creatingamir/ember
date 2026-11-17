fn low() -> i64 {
    print(100)
    return 1
}

fn high() -> i64 {
    print(200)
    return 4
}

fn main() {
    for i in low()..high() {
        print(i)
    }
}
