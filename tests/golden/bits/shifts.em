fn main() {
    s8: i8 = -8
    u8v: u8 = 248
    one: i64 = 1
    three: u8 = 3
    n1: i32 = 1
    n3: i32 = 3
    n7: i32 = 7
    print(s8 >> n1)
    print(u8v >> n1)
    print(s8 >> n3)
    print(u8v >> n3)
    print(s8 >> n7)
    print(u8v >> n7)
    big: i64 = -9223372036854775808
    ubig: u64 = 18446744073709551615
    n63: u8 = 63
    print(big >> n63)
    print(ubig >> n63)
    m: i32 = -2147483648
    n31: i16 = 31
    print(m >> n31)
    print(one << three)
    print(u8v << 0)
    print(2 + 1 << 2)
    print(1 << 3 << 2)
    print(256 >> 2 >> 1)
    print(1 << 3 == 8)
}
