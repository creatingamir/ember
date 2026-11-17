fn main() {
    v0: i8 = 0
    v1: i8 = -128
    v2: i8 = 127
    v3: u8 = 0
    v4: u8 = 255
    v5: u8 = 170
    v6: i16 = 1
    v7: u16 = 0
    v8: u16 = 65535
    v9: i32 = 5
    v10: u32 = 0
    v11: u32 = 4294967295
    v12: i64 = -9223372036854775808
    v13: i64 = 0
    v14: u64 = 0
    v15: u64 = 18446744073709551615
    print(~v0)
    print(~v1)
    print(~v2)
    print(~v3)
    print(~v4)
    print(~v5)
    print(~v6)
    print(~v7)
    print(~v8)
    print(~v9)
    print(~v10)
    print(~v11)
    print(~v12)
    print(~v13)
    print(~v14)
    print(~v15)
    print(~~v2)
    n: i32 = 5
    print(~n + 1)
    k: u8 = ~0
    print(k)
}
