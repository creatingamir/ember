fn add8(a: i8, b: i8) -> i8 {
    return a + b
}

fn sub_u8(a: u8, b: u8) -> u8 {
    return a - b
}

fn mul16(a: i16, b: i16) -> i16 {
    return a * b
}

fn half_u16(a: u16) -> u16 {
    return a / 2
}

fn neg32(a: i32) -> i32 {
    return -a
}

fn id_u32(a: u32) -> u32 {
    return a
}

fn big() -> u64 {
    return 18446744073709551615
}

fn fact(n: u32) -> u32 {
    if n < 2 {
        return 1
    }
    return n * fact(n - 1)
}

fn is_small(a: i8) -> bool {
    return a < 10
}

fn show12(a: i8, b: u8, c: i16, d: u16, e: i32, f: u32, g: i64, h: u64, i: i8, j: u8, k: i16, l: u16) {
    print(a)
    print(b)
    print(c)
    print(d)
    print(e)
    print(f)
    print(g)
    print(h)
    print(i)
    print(j)
    print(k)
    print(l)
}

fn main() {
    print(add8(100, 27))
    print(add8(add8(60, 60), 7))
    print(add8(-100, -28))
    print(sub_u8(200, 56))
    print(sub_u8(5, 5) + 1)
    print(mul16(181, 181))
    print(half_u16(65535))
    print(neg32(2147483647))
    print(id_u32(4294967295))
    print(big())
    print(fact(12))
    print(is_small(-100))
    print(is_small(10))
    show12(-100, 200, -30000, 60000, -2000000000, 4000000000, -9000000000000000000, 18000000000000000000, -128, 255, -32768, 65535)
}
