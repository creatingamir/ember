fn exit(code: i64) {
    print(code)
}

fn printf(code: i64) -> i64 {
    return code + 1
}

fn main() {
    exit(1)
    print(printf(1))
}
