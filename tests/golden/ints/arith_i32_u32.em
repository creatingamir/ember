fn main() {
    i32_max: i32 = 2147483647
    i32_min: i32 = -2147483648
    i32_one: i32 = 1
    i32_neg_one: i32 = -1
    i32_two: i32 = 2
    i32_neg_two: i32 = -2
    i32_zero: i32 = 0
    i32_three: i32 = 3
    i32_seven: i32 = 7
    i32_half_min: i32 = -1073741824
    print(i32_max + i32_neg_one)
    print(i32_max - i32_one)
    print(i32_min + i32_one)
    print(i32_min - i32_neg_one)
    print(i32_max + i32_min)
    print(i32_max - i32_max)
    print(i32_min - i32_min)
    print((i32_max - i32_one) + i32_one)
    print(i32_half_min * i32_two)
    print(i32_max * i32_one)
    print(i32_min * i32_one)
    print(i32_max * i32_neg_one)
    print(i32_min / i32_one)
    print(i32_min / i32_two)
    print(i32_min / i32_neg_two)
    print(i32_max / i32_two)
    print(i32_max % i32_seven)
    print(i32_min % i32_seven)
    print(i32_max % i32_three)
    print(i32_min % i32_three)
    print(-i32_max)
    print(-i32_neg_one)
    print(i32_min < i32_max)
    print(i32_min < i32_zero)
    print(i32_max > i32_min)
    print(i32_max >= i32_max)
    print(i32_min <= i32_min)
    print(i32_max == i32_max)
    print(i32_max != i32_min)
    u32_max: u32 = 4294967295
    u32_zero: u32 = 0
    u32_one: u32 = 1
    u32_two: u32 = 2
    u32_three: u32 = 3
    u32_seven: u32 = 7
    u32_half: u32 = 2147483647
    print(u32_zero + u32_max)
    print(u32_max - u32_one)
    print(u32_max - u32_max)
    print(u32_zero - u32_zero)
    print(u32_max * u32_one)
    print(u32_zero * u32_max)
    print(u32_half * u32_two)
    print((u32_half + u32_half) + u32_one)
    print((u32_max - u32_half) - u32_half)
    print(u32_max / u32_two)
    print(u32_max / u32_max)
    print(u32_zero / u32_max)
    print(u32_max % u32_seven)
    print(u32_max % u32_two)
    print(u32_half % u32_three)
    print(u32_max / u32_three)
    print(u32_max > u32_zero)
    print(u32_zero < u32_max)
    print(u32_max >= u32_max)
    print(u32_zero <= u32_zero)
    print(u32_max == u32_max)
    print(u32_max != u32_zero)
    print(u32_half > u32_one)
    print(u32_one < u32_max)
}
