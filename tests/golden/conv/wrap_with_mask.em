fn main() {
    x: i64 = 1000
    print(u8(x & 255))
    y: i64 = -1
    print(u8(y & 255))
    print(u16(y & 65535))
    print(u32(y & 4294967295))
    print(i8(x & 127))
    z: u32 = 4000000000
    print(u16(z & 65535))
    print(u8(z >> 24))
    big: u64 = 18446744073709551615
    print(u32(big & 4294967295))
    print(i32(big & 2147483647))
}
