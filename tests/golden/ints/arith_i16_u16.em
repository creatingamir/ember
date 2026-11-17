fn main() {
    i16_max: i16 = 32767
    i16_min: i16 = -32768
    i16_one: i16 = 1
    i16_neg_one: i16 = -1
    i16_two: i16 = 2
    i16_neg_two: i16 = -2
    i16_zero: i16 = 0
    i16_three: i16 = 3
    i16_seven: i16 = 7
    i16_half_min: i16 = -16384
    print(i16_max + i16_neg_one)
    print(i16_max - i16_one)
    print(i16_min + i16_one)
    print(i16_min - i16_neg_one)
    print(i16_max + i16_min)
    print(i16_max - i16_max)
    print(i16_min - i16_min)
    print((i16_max - i16_one) + i16_one)
    print(i16_half_min * i16_two)
    print(i16_max * i16_one)
    print(i16_min * i16_one)
    print(i16_max * i16_neg_one)
    print(i16_min / i16_one)
    print(i16_min / i16_two)
    print(i16_min / i16_neg_two)
    print(i16_max / i16_two)
    print(i16_max % i16_seven)
    print(i16_min % i16_seven)
    print(i16_max % i16_three)
    print(i16_min % i16_three)
    print(-i16_max)
    print(-i16_neg_one)
    print(i16_min < i16_max)
    print(i16_min < i16_zero)
    print(i16_max > i16_min)
    print(i16_max >= i16_max)
    print(i16_min <= i16_min)
    print(i16_max == i16_max)
    print(i16_max != i16_min)
    u16_max: u16 = 65535
    u16_zero: u16 = 0
    u16_one: u16 = 1
    u16_two: u16 = 2
    u16_three: u16 = 3
    u16_seven: u16 = 7
    u16_half: u16 = 32767
    print(u16_zero + u16_max)
    print(u16_max - u16_one)
    print(u16_max - u16_max)
    print(u16_zero - u16_zero)
    print(u16_max * u16_one)
    print(u16_zero * u16_max)
    print(u16_half * u16_two)
    print((u16_half + u16_half) + u16_one)
    print((u16_max - u16_half) - u16_half)
    print(u16_max / u16_two)
    print(u16_max / u16_max)
    print(u16_zero / u16_max)
    print(u16_max % u16_seven)
    print(u16_max % u16_two)
    print(u16_half % u16_three)
    print(u16_max / u16_three)
    print(u16_max > u16_zero)
    print(u16_zero < u16_max)
    print(u16_max >= u16_max)
    print(u16_zero <= u16_zero)
    print(u16_max == u16_max)
    print(u16_max != u16_zero)
    print(u16_half > u16_one)
    print(u16_one < u16_max)
}
