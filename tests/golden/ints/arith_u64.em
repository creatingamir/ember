fn main() {
    u64_max: u64 = 18446744073709551615
    u64_zero: u64 = 0
    u64_one: u64 = 1
    u64_two: u64 = 2
    u64_three: u64 = 3
    u64_seven: u64 = 7
    u64_half: u64 = 9223372036854775807
    u64_two_32: u64 = 4294967296
    u64_two_32_less_one: u64 = 4294967295
    u64_two_63: u64 = 9223372036854775808
    u64_ten: u64 = 10
    print(u64_zero + u64_max)
    print(u64_max - u64_one)
    print(u64_max - u64_max)
    print(u64_zero - u64_zero)
    print(u64_max * u64_one)
    print(u64_zero * u64_max)
    print(u64_half * u64_two)
    print((u64_half + u64_half) + u64_one)
    print((u64_max - u64_half) - u64_half)
    print(u64_max / u64_two)
    print(u64_max / u64_max)
    print(u64_zero / u64_max)
    print(u64_max % u64_seven)
    print(u64_max % u64_two)
    print(u64_half % u64_three)
    print(u64_max / u64_three)
    print(u64_max > u64_zero)
    print(u64_zero < u64_max)
    print(u64_max >= u64_max)
    print(u64_zero <= u64_zero)
    print(u64_max == u64_max)
    print(u64_max != u64_zero)
    print(u64_half > u64_one)
    print(u64_one < u64_max)
    print(u64_two_32 * u64_two_32_less_one)
    print(u64_two_63 + (u64_two_63 - u64_one))
    print(u64_two_63 - u64_two_63)
    print(u64_max / u64_two_63)
    print(u64_max % u64_ten)
    print(u64_two_63 % u64_seven)
    print(u64_two_63 * u64_one)
    print(u64_max - u64_two_63)
    print(u64_two_63 > u64_one)
    print(u64_two_32 < u64_two_63)
    print((u64_two_32 * u64_two_32_less_one) / u64_two_32)
}
