# Ember

Ember is a small, statically typed programming language with tensors and automatic differentiation built in. It compiles ahead of time to native ARM64 code for the CPU and to Metal kernels for the GPU, and targets Apple Silicon first.

```
fn loss(m: MLP, x: f32[B, 784], y: i64[B]) -> f32 = cross_entropy(m.forward(x), y)

m := init() on gpu
for x, y in train.batches(64) {
    arena {
        l, g := grad(loss)(m, x, y)
        m -= lr * g
    }
}
```

## Design goals

- **Simple.** 23 keywords, one way to do each thing, one official formatter.
- **No hidden costs.** No garbage collector, no exceptions, no implicit copies or allocations, no operator overloading.
- **Native.** Compiled ahead of time. No VM and no interpreter at runtime.
- **Tensors are first-class.** Shapes are part of the type and checked at compile time.
- **`grad` is a compiler pass.** The backward pass is generated at compile time, so nothing is recorded while the program runs.
- **Memory through arenas.** Every allocation belongs to an arena that frees everything at once when its block ends.

## Status

Early development. Nothing compiles yet. The compiler is written in Rust and built in phases, each ending in a program that runs and a test that proves it:

| Phase | Result |
| --- | --- |
| 0 | Foundations: repository, CI, test harness |
| 1 | Smallest compiler: integers, functions, `if`, `for` to a native executable |
| 2 | Core language: floats, structs, slices, enums, errors, arenas, modules |
| 3 | Tensors on the CPU with compile-time shape checking |
| 4 | `grad`: compiler-generated backward pass |
| 5 | Optimizer: SSA and tensor op fusion |
| 6 | Metal GPU backend |
| 7 | Standard library and GPT-2 |
| 8 | Benchmarks, docs and release |

## Requirements

- macOS on Apple Silicon
- Rust (stable) and the Xcode command-line tools
