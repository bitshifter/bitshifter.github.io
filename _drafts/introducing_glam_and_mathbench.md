---
layout: post
comments: true
title:  "Introducing glam and mathbench"
excerpt_separator: <!--more-->
tags: rust simd math performance
---

[`glam`] is a simple and fast Rust 2D and 3D math library for games and graphics.

[`mathbench`] is a set of unit tests and benchmarks comparing the
performance of `glam` with the popular Rust math libraries [`cgmath`] and
[`nalgebra`].

The following is a table of benchmarks produced by `mathbench` comparing `glam`
performance to `cgmath` and `nalgebra` on `f32` data.

| benchmark                 |         glam   |       cgmath   |     nalgebra   |
|:--------------------------|---------------:|---------------:|---------------:|
| euler 2d                  |     9.202 us   |   __9.059 us__ |     26.53 us   |
| euler 3d                  |   __14.37 us__ |     32.38 us   |     198.4 us   |
| mat2 determinant          |    1.3619 ns   |    1.0668 ns   |  __1.0637 ns__ |
| mat2 inverse (see notes)  |  __2.0673 ns__ |    2.7771 ns   |    3.9397 ns   |
| mat2 mul mat2             |  __2.1416 ns__ |    3.0682 ns   |    3.6766 ns   |
| mat2 transform vec2       |  __2.2084 ns__ |    2.4233 ns   |    7.0098 ns   |
| mat2 transpose            |  __0.7204 ns__ |    1.3371 ns   |    1.8440 ns   |
| mat3 determinant          |  __2.2374 ns__ |    2.6162 ns   |    2.6048 ns   |
| mat3 inverse              |  __7.8398 ns__ |    8.0758 ns   |   11.4357 ns   |
| mat3 mul mat3             |  __4.9372 ns__ |    9.5770 ns   |    8.1240 ns   |
| mat3 transform vec3       |  __2.3139 ns__ |    4.1042 ns   |    8.2158 ns   |
| mat3 transpose            |  __2.0043 ns__ |    3.5812 ns   |    5.9885 ns   |
| mat4 determinant          |  __8.0186 ns__ |   11.3334 ns   |   54.3724 ns   |
| mat4 inverse              | __21.2964 ns__ |   43.8595 ns   |   57.8703 ns   |
| mat4 mul mat4             |  __6.8366 ns__ |    9.4308 ns   |   15.5215 ns   |
| mat4 transform vec4       |  __2.5644 ns__ |    3.5984 ns   |    4.1611 ns   |
| mat4 transpose            |  __2.7450 ns__ |    7.8940 ns   |   11.0163 ns   |
| quat conjugate            |  __0.9036 ns__ |    1.8176 ns   |   11.2922 ns   |
| quat mul quat             |  __2.9802 ns__ |    5.4663 ns   |   11.0629 ns   |
| quat transform vec3       |  __4.2821 ns__ |    6.6963 ns   |   24.6107 ns   |
| vec3 cross                |  __2.0753 ns__ |    2.8664 ns   |    6.9884 ns   |
| vec3 dot                  |  __1.3448 ns__ |    1.7015 ns   |    1.6879 ns   |
| vec3 length               |    2.0720 ns   |  __2.0494 ns__ |   82.7194 ns   |
| vec3 normalize            |  __4.1169 ns__ |    4.1675 ns   |   84.2364 ns   |

These benchmarks were performed on my [Intel i7-4710HQ] CPU on Linux. They were
compiled with the stable 1.35 Rust compiler. Lower (better) numbers are
highlighted. I hope it's clear that `glam` is outperforming `cgmath` and
`nalgebra` in most of these benchmarks. Generally `glam` functionality is the
same as the others, however `cgmath` and `nalgebra` matrix inverse functions
return an `Option` type which is `None` if the matrix wasn't invertible, `glam`
assumes the input was invertible and returns a `Mat4`.

See the full [mathbench report] for more detailed results.

The reason `glam` is faster is primarily due to it using SIMD internally for all
types with the exception of `Vec2`.

You can see a similar table at the bottom of this post with `glam`'s SIMD
support disabled. TL;DR it's similar performance to `cgmath` which is expected.

## Why write another math library?

My goal writing `glam` and the trade offs I made are primarily being focused
on good `f32` performance by using SIMD when available and a simpler API. This
is at the expense of genericity, there is no `Vector3<T>` type, just `Vec3`
which is `f32` based. It would be possible to support `f64` or generic types in
the future but it's not a high priority right now, in my experience most games
use `f32` almost exclusively.

`glam` also avoids baking mathematical correctness into the type system, there
are no `Point3` or `UnitQuaternion` types for example, it is up to the
programmer if they want their `Vector3` to behave as a point or a vector or if
their quaternion is normalised. This decision sacrifices enforced runtime
correctness via the type system for performance and a smaller, simpler API.

## SIMD

I noticed when optimising a path tracer was that I got some good performance
wins from implementing a `Vector3` with `SSE2` (see [SIMD path tracing]). I
wanted to take that further and build a full library that utilised SIMD for
vector, matrix and quaternion types. I'd seen Rust math libraries that use SIMD
for some functions but I hadn't see one that uses SIMD vectors for storage.

### SIMD primer

SIMD stands for Single Instruction Multiple Data. The multiple data part varies
depending on CPU. Recent Intel CPUs have SIMD instruction sets that can operate
on 128-bit (SSE), 256-bit (AVX) and 512-bit data sizes. That is 4, 8 and 16
`f32` values processed with a single instruction.

Using a simple summation as an example, the difference between the scalar and
SIMD operations is illustrated below.

![Scalar vs SIMD]

With conventional scalar operations, four add instructions must be executed one
after another to obtain the sums. Meanwhile, SIMD uses only one add instruction
to achieve the same result.  Requiring fewer instructions to process a given
mass of data, SIMD operations yield higher efficiency than scalar operations.

SIMD values are typically stored in a special type. In the case of SSE2, the
`__m128` type is used to store 4 floats. One significant difference between this
type and say `[f32; 4]` is `__m128` is 16 byte aligned. While you can load
unaligned data into SSE2 registers, this is slower than loading aligned data.

There are some down sides to using SIMD for a vector math library. One thing you
may notice with the above example is there are 4 values, while a `Vec3` only
contains 3 values. The 4th value is effectively ignored and is just wasted
space.  Another down side is while SIMD is great for adding two vectors
together, "horizontal operations" are awkward and not fast. An example of a
horizontal operation would be to sum the values `A0`, `A1`, `A2` and `A3`, or
more commonly in a vector math library, performing a dot product is a horizontal
operation. If your CPU supported wider instruction sets such as AVX2, a `Vec3`
can't take advantage of it, after all it only has 3 floats even if your
instruction set can operate on 8.

The most effective way to take advantage of SIMD is to structure your data is
Structure of Arrays (SoA), and feed that through the widest SIMD vectors you
have on your CPU.  What I mean by that is instead of:

```rust
struct Vec3AoS {
  xyz: Vec<(f32, f32, f32)>,
}
```

You store each component of the vector in it's own array:

```rust
struct Vec3SoA {
  x: Vec<f32>,
  y: Vec<f32>,
  z: Vec<f32>,
}
```

Then load 4 `x`, `y` and `z` values at a time and add them. `glam` does not help
you with that.

For more information on doing SIMD the "right way" I recommend reading through
this GDC2015 presentation on [SIMD at Insomniac Games].

You should always be able to out perform `glam` by rewriting your data
structures to be SoA and using SIMD instructions directly, I still think it's
valuable to have a math library that performs well if you haven't done that
work.

Although 25% of `glam::Vec3` is wasted space, it still performs better than the
equivalent scalar code since we are still operating on 3 float with one
instructions a lot of the time. And even though horizontal operations such as
dot products are awkward, they still have slightly better performance than the
scalar equivalent as you can see in the `mathbench` results about for vec3 dot.

## API design

### Built around SIMD

Many `glam` types use `__m128` for data storage internally to get the best SSE2
performance. This has a few implications for API design.

The `__m128` is opaque. You do not have direct access to the underlying
`f32` components. All component data must be read and written via accessors. For
example this is how the get and set of the `y` component of a `glam::Vec3` are
implemented:

```rust
impl Vec3 {
    #[inline]
    pub fn y(self) -> f32 {
        unsafe { _mm_cvtss_f32(_mm_shuffle_ps(self.0, self.0, 0b01_01_01_01)) }
    }

    #[inline]
    pub fn set_y(&mut self, y: f32) {
        unsafe {
            let mut t = _mm_move_ss(self.0, _mm_set_ss(y));
            t = _mm_shuffle_ps(t, t, 0b11_10_00_00);
            self.0 = _mm_move_ss(t, self.0);
        }
    }
  }
```

### Avoiding complexity

I wanted to come up with a simple API which is very low friction for developers
to use. Something that covers the common needs for someone working in games and
graphics and doesn't require much effort learn. I also wanted something to was
easy for me to write.

From the outset I wanted to avoid using traits and generics. Traits and generics
are wonderful language features, but I didn't see a great reason to use them in
`glam`.

Using SSE2 for storage would complicate generics, as I think 2 generic types
would be required, a scalar type (e.g. `f32`) and a generic for the storage type
(`__m128` if available), that already sounds complicated!

That's not to say that `glam` will never contain vectors of integer or generic
types, but for the sake of simplicity I wanted to avoid them for a while until
the API feels stable and there's a compelling reason to use them.

### Feeling Rusty

I also wanted to make `glam` "Rusty". One thing I've noticed about the Rust
standard library is everything is a method, for example `sin` is a method on
`f32` and `f64`. It feels a bit weird coming from other languages. It turns out
there are reasons for this, from the [Rust API guidelines]:

> Methods have numerous advantages over functions:
> * They do not need to be imported or qualified to be used: all you need is a
>   value of the appropriate type.
> * Their invocation performs auto-borrowing (including mutable borrows).
> * They make it easy to answer the question "what can I do with a value of type
>   T" (especially when using rustdoc).
> * They provide self notation, which is more concise and often more clearly
>   conveys ownership distinctions.

Things like `a.lerp(b, 0.5)` feel a bit odd, but I could always add functions
for this kind of thing in addition to the methods.

I've tried to follow the Rust API Guidelines in general.

### Mathematical conventions

`glam` interprets vectors as column matrices (also known as column vectors)
meaning when transforming a vector with a matrix the matrix goes on the left,
e.g. `v' = Mv`. DirectX uses row vectors, OpenGL uses column vectors. There are
pros and cons to both.

Matrices are stored in column major format. Each column vector is stored in
contiguous memory.

Rotations follow the left-hand rule.

Some libraries support both column and row vectors my preference with `glam` is
to pick one convention and stick with it (of course I didn't do that, I started
with row vectors and switched to column vectors).

## Test coverage

I wanted to aim for 100% test coverage, especially because there are multiple
implementations of many types depending on whether SIMD is available or not.

To determine if I actually did have 100% test coverage I've been using a cargo
plugin called [`tarpaulin`]. It only supports **x86_64** processors running
Linux and does give some false positives, but it's been pretty good for my
needs. I have it integrated into my [`travis-ci`] build and posting results to
[`coveralls.io`].

## Performance

If you want to say your library is fast, you better be measuring performance!

### Micro-benchmarking with Criterion.rs

I've primarily been monitoring the performance of `glam` using the [`criterion`]
crate. `criterion` fills a similar niche to Rust's built-in [`bench`] but unlike
`bench` it works on stable. It also has a bunch of other nice features.

Micro-benchmarking is a useful indication of performance, but it's not
necessarily going to tell you the full story of what code will perform like
under less artificial conditions (benchmarks tend to be quite limited in scope).
All the same, they're definitely better than no metrics and I've found them to
be a pretty reliable source of performance information when optimising some
code.

### Comparing with other libraries

`glam` has a bunch of criterion benchmarks that are useful for optimising `glam`
but they don't tell me how `glam` performs relative to other math libraries.

I created [`mathbench`] for this purpose. `mathbench` is a collection of unit
tests and benchmarks. The unit tests are to check that `glam` is producing the
same output as other libraries (with some floating point tolerance) and the
benchmarks compare performance. The table at the top of this post is produced
from these benchmarks.

`mathbench` could also be of use to other library authors to see how their
library is performing and for performing optimisations.

One of the nice features of `criterion` is you can set up one benchmark that
runs multiple functions on the same data and produces a report comparing the
results. Here's an example from `mathbench`:

```rust
fn bench_mat4_mul_mat4(c: &mut Criterion) {
    use criterion::Benchmark;
    use std::ops::Mul;
    c.bench(
        "mat4 mul mat4",
        Benchmark::new("glam", |b| {
            use glam::Mat4;
            bench_binop!(b, op => mul_mat4, ty1 => Mat4, ty2 => Mat4)
        })
        .with_function("cgmath", |b| {
            use cgmath::Matrix4;
            bench_binop!(b, op => mul, ty1 => Matrix4<f32>, ty2 => Matrix4<f32>)
        })
        .with_function("nalgebra", |b| {
            use nalgebra::Matrix4;
            bench_binop!(b, op => mul, ty1 => Matrix4<f32>, ty2 => Matrix4<f32>)
        }),
    );
}

```

If you have `gnuplot` installed `criterion` will produce [reports] with charts
like the violin plot below:

![violin plot]

### Viewing assembly

While `mathbench` is a library, most of it's utility is from the tests and
benchmarks. One use I have had for the library though is creating public
functions for inspecting assembly with [`cargo-asm`]. This is primarily because
it was the easiest way to get `cargo-asm` to show me something.

For example, in `mathbench` I've defined a simple wrapper function for
`glam::Mat4 * glam::Vec4`:

```rust
pub fn glam_mat4_mul_vec4(lhs: &glam::Mat4, rhs: &glam::Vec4) -> glam::Vec4 {
    *lhs * *rhs
}
```

And then run `cargo asm mathbench::glam_mat4_mul_vec4` to get the following
output:

```
 mov     rax, rdi
 movaps  xmm0, xmmword, ptr, [rdx]
 movaps  xmm1, xmm0
 shufps  xmm1, xmm0, 0
 mulps   xmm1, xmmword, ptr, [rsi]
 movaps  xmm2, xmm0
 shufps  xmm2, xmm0, 85
 mulps   xmm2, xmmword, ptr, [rsi, +, 16]
 addps   xmm2, xmm1
 movaps  xmm1, xmm0
 shufps  xmm1, xmm0, 170
 mulps   xmm1, xmmword, ptr, [rsi, +, 32]
 addps   xmm1, xmm2
 shufps  xmm0, xmm0, 255
 mulps   xmm0, xmmword, ptr, [rsi, +, 48]
 addps   xmm0, xmm1
 movaps  xmmword, ptr, [rdi], xmm0
 ret
```

It's a pretty convenient way of viewing assembly, with the catch that you can't
see `#[inline]` blocks.

### Profiling benchmarks

You can run individual benchmarks through a profiler to get a better
understanding of their performance. When you run `cargo bench` it prints the
path to each benchmark executable that it is running. You can pass the same
parameters to this executable that you pass to `cargo bench`.

For example, if I wanted to profile `glam`'s `Mat4` `inverse` I could run this
command via my preferred profiler:

```
target/release/deps/mat4bench-e528128e2a9ccbc9 "mat4 inverse/glam"
```

There will be a bit of noise from the benchmarking code but it's usually not too
hard to work out what is what. If your code is being inlined then it will
probably appear inside `criterion::Bencher::iter`.

## What next for glam

I consider `glam` to be a minimum viable math library right now, so I'll slowly
add more functionality over time. That will depend a bit on if other people
adopt it and if so what features they are missing.

The main outstanding thing for me right now is documenting what is there.

There are of course more optimisations that could be done.

I'm interested in using the [`packed_simd`] crate instead of using SSE2
directly as `packed_simd` supports multiple architectures out of the box.
However `packed_simd` currently requires nightly and I'm not sure what the
development status of this crate is right now.

## Inspirations

There were many inspirations for the interface and internals of `glam` from the
Rust and C++ worlds. In no particular order:

* [How to write a maths library in 2016] via
  [Aras Pranckevičius's pathtracer blog series]
* [Realtime Math] - header only C++11 with SSE and NEON SIMD intrinsic support
* [DirectXMath] - header only SIMD C++ linear algebra library for use in games
  and graphics apps
* [DirectX Tool Kit SimpleMath] - simplified C++ wrapper around DirectXMath
* [GLM] - header only C++ library based on the GLSL specification
* [`packed_simd`] - mentioned above, I like the interface
* [`cgmath`] - the Rust library I'm most familiar with
* [`nalgebra`] - a very comprehensive and well documented Rust library

## glam without SSE2

`glam`'s performance is around on par with `cgmath` if SSE2 is disabled using
the `scalar-math` feature. Some `glam` functions got faster without SIMD.

| benchmark                 |         glam   |       cgmath   |     nalgebra   |
|:--------------------------|---------------:|---------------:|---------------:|
| euler 2d                  |     9.074 us   |   __8.966 us__ |     26.22 us   |
| euler 3d                  |   __28.86 us__ |      29.7 us   |     195.3 us   |
| mat2 determinant          |  __1.0548 ns__ |    1.0603 ns   |    1.0600 ns   |
| mat2 inverse (see notes)  |  __2.3650 ns__ |    2.6464 ns   |    2.6712 ns   |
| mat2 mul mat2             |    3.0660 ns   |  __3.0278 ns__ |    3.6211 ns   |
| mat2 transform vec2       |  __2.4026 ns__ |    2.4059 ns   |    6.8847 ns   |
| mat2 transpose            |    1.3356 ns   |  __1.3324 ns__ |    1.8256 ns   |
| mat3 determinant          |    2.5755 ns   |    2.5934 ns   |  __2.5752 ns__ |
| mat3 inverse              |   10.5764 ns   |  __7.9405 ns__ |    9.3305 ns   |
| mat3 mul mat3             |    9.4757 ns   |    9.4536 ns   |  __7.9284 ns__ |
| mat3 transform vec3       |    4.0669 ns   |  __4.0666 ns__ |    8.0513 ns   |
| mat3 transpose            |    3.5409 ns   |  __3.5194 ns__ |    8.9463 ns   |
| mat4 determinant          |  __7.6512 ns__ |   11.1750 ns   |   54.2360 ns   |
| mat4 inverse              | __31.3069 ns__ |   43.3849 ns   |   55.4980 ns   |
| mat4 mul mat4             |    9.5908 ns   |  __9.2905 ns__ |   15.8365 ns   |
| mat4 transform vec4       |  __3.5640 ns__ |    3.5750 ns   |    4.1351 ns   |
| mat4 transpose            |  __7.6544 ns__ |    9.7473 ns   |   10.6901 ns   |
| quat conjugate            |    1.8292 ns   |  __1.7632 ns__ |    1.7671 ns   |
| quat mul quat             |  __5.3086 ns__ |    5.3542 ns   |    5.4800 ns   |
| quat transform vec3       |    7.1021 ns   |  __6.5746 ns__ |    7.0461 ns   |
| vec3 cross                |    2.8486 ns   |    2.8410 ns   |  __2.8401 ns__ |
| vec3 dot                  |  __1.5912 ns__ |    1.6773 ns   |    1.6324 ns   |
| vec3 length               |  __2.0224 ns__ |    2.0248 ns   |   81.8765 ns   |
| vec3 normalize            |    4.1265 ns   |  __4.1232 ns__ |   83.5038 ns   |

[`glam`]: https://docs.rs/crate/glam
[`mathbench`]: https://github.com/bitshifter/mathbench-rs
[`cgmath`]: https://docs.rs/crate/cgmath
[`nalgebra`]: https://www.nalgebra.org
[Intel i7-4710HQ]: https://ark.intel.com/content/www/us/en/ark/products/78930/intel-core-i7-4710hq-processor-6m-cache-up-to-3-50-ghz.html
[mathbench report]: https://bitshifter.github.io/mathbench/criterion/report/index.html
[SIMD path tracing]: https://bitshifter.github.io/2018/06/04/simd-path-tracing/#converting-vec3-to-sse2
[Scalar vs SIMD]: http://ftp.cvut.cz/kernel/people/geoff/cell/ps3-linux-docs/CellProgrammingTutorial/CellProgrammingTutorial.files/image008.jpg
[SIMD at Insomniac Games]: https://deplinenoise.files.wordpress.com/2015/03/gdc2015_afredriksson_simd.pdf
[Rust API Guidelines]: https://rust-lang-nursery.github.io/api-guidelines/predictability.html#c-method
[`tarpaulin`]: https://github.com/xd009642/tarpaulin
[`travis-ci`]: https://travis-ci.org/bitshifter/glam-rs
[`coveralls.io`]: https://coveralls.io/github/bitshifter/glam-rs
[`criterion`]: https://bheisler.github.io/criterion.rs/book/index.html
[`bench`]: https://doc.rust-lang.org/test/struct.Bencher.html
[reports]: https://bitshifter.github.io/mathbench/criterion/mat4%20mul%20mat4/report/index.html
[violin plot]: https://bitshifter.github.io/mathbench/criterion/mat4%20mul%20mat4/report/violin.svg
[`packed_simd`]: https://rust-lang-nursery.github.io/packed_simd/packed_simd/
[How to write a maths library in 2016]: http://www.codersnotes.com/notes/maths-lib-2016/
[Aras Pranckevičius's pathtracer blog series]: https://aras-p.info/blog/2018/04/10/Daily-Pathtracer-Part-7-Initial-SIMD/
[Realtime Math]: https://github.com/nfrechette/rtm
[DirectXMath]: https://docs.microsoft.com/en-us/windows/desktop/dxmath/directxmath-portal
[DirectX Tool Kit SimpleMath]: https://github.com/Microsoft/DirectXTK/wiki/SimpleMath
[GLM]: https://glm.g-truc.net/
