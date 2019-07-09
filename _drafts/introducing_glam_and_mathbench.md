---
layout: post
comments: true
title:  "Introducing glam and mathbench"
excerpt_separator: <!--more-->
tags: rust simd math performance
---

[`glam`] is a simple and fast Rust linear algebra library for games and
graphics.

[`mathbench`] is a set of unit tests and benchmarks comparing the
performance of `glam` with the popular Rust linear algebra libraries [`cgmath`]
and [`nalgebra`].

The following is a table of benchmarks produced by `mathbench` comparing `glam`
performance to `cgmath` and `nalgebra` on `f32` data.

| benchmark           |         glam   |       cgmath   |     nalgebra   |
|:--------------------|---------------:|---------------:|---------------:|
| euler 2d            |     9.063 us   |   __8.977 us__ |     27.94 us   |
| euler 3d            |   __15.78 us__ |     29.52 us   |     195.4 us   |
| mat2 determinant    |    1.3140 ns   |  __1.0528 ns__ |    1.0825 ns   |
| mat2 inverse        |  __2.0524 ns__ |    2.6565 ns   |    2.7056 ns   |
| mat2 mul mat2       |  __2.1091 ns__ |    3.0495 ns   |    3.6305 ns   |
| mat2 transform vec2 |  __2.1865 ns__ |    2.4035 ns   |    6.8877 ns   |
| mat2 transpose      |  __0.7117 ns__ |    1.3231 ns   |    1.8297 ns   |
| mat3 determinant    |  __2.2130 ns__ |    2.5348 ns   |    2.5766 ns   |
| mat3 inverse        |  __7.7296 ns__ |    7.9519 ns   |    9.3100 ns   |
| mat3 mul mat3       |  __4.9371 ns__ |    9.4619 ns   |    8.3076 ns   |
| mat3 transform vec3 |  __2.2893 ns__ |    4.0822 ns   |    8.0838 ns   |
| mat3 transpose      |  __2.0035 ns__ |    3.5175 ns   |    8.9548 ns   |
| mat4 determinant    |  __7.9524 ns__ |   12.2581 ns   |   54.2954 ns   |
| mat4 inverse        | __21.0455 ns__ |   43.6983 ns   |   55.8788 ns   |
| mat4 mul mat4       |  __6.7517 ns__ |    9.2621 ns   |   16.2498 ns   |
| mat4 transform vec4 |  __2.5298 ns__ |    3.5734 ns   |    4.1222 ns   |
| mat4 transpose      |  __2.7026 ns__ |    8.0926 ns   |   11.5411 ns   |
| quat conjugate      |  __0.8866 ns__ |    1.8263 ns   |    1.7650 ns   |
| quat mul quat       |  __2.8647 ns__ |    5.6678 ns   |    5.4807 ns   |
| quat transform vec3 |  __4.2107 ns__ |    6.5798 ns   |    7.0398 ns   |
| vec3 cross          |  __2.0677 ns__ |    2.8890 ns   |    2.8725 ns   |
| vec3 dot            |  __1.3789 ns__ |    1.6669 ns   |    1.6666 ns   |
| vec3 length         |  __2.0412 ns__ |    2.0508 ns   |   88.1277 ns   |
| vec3 normalize      |  __4.0433 ns__ |    4.1260 ns   |   87.2455 ns   |

These benchmarks were performed on my [Intel i7-4710HQ] CPU on Linux. They were
compiled with the stable 1.36 Rust compiler. Lower (better) numbers are
highlighted. I hope it's clear that `glam` is outperforming `cgmath` and
`nalgebra` in most of these benchmarks.

Generally `glam` functionality is the same as the libraries for the functions
tested here, however `cgmath` and `nalgebra` matrix inverse functions return an
`Option` type which is `None` if the matrix wasn't invertible, `glam` assumes
the input was invertible and returns a `Mat4`. There may be other differences
I'm not aware of. `cgmath` also targets games and graphics, `nalgebra` is a
general purpose linear algebra library which has a much broader target audience
than just games and graphics programming.

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
space.

Another down side is while SIMD is great for adding two vectors
together, "horizontal operations" are awkward and not so fast. An example of a
horizontal operation would be to sum the values `A0`, `A1`, `A2` and `A3`, or
more commonly in a vector math library, performing a dot product is a horizontal
operation.

If your CPU supported wider instruction sets such as AVX2, a `Vec3`
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
scalar equivalent as you can see in the `mathbench` results above for `Vec3`
`dot`.

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

These getter and setter methods are using SSE2 intrinsics to load and store
scalar values to and from the y lane of the `__m128` vector.

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
Linux and does give some false negatives, but it's been pretty good for my
needs. I have it integrated into my [`travis-ci`] build and posting results to
[`coveralls.io`].

According to `coveralls.io` `glam` has 87% test covereage, I think the real
figure is probably a bit higher due to `tarpaulin` reporting some lines being
untested when they actually are.

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

For example, if I wanted to profile the `glam` `Mat4` `inverse` method I could
run this command via my preferred profiler:

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

## The Rust ecosystem is awesome

Writing Rust is purely recreational for me. My day job for the last 14 years has
been writing C++ game engine and gameplay code.

It boggles my mind how much great tooling I have at my fingertips with Rust that
either comes with the Rust installer or is easily accessible. Things like:

* A common build system via `cargo build`
* A common packaging system via [crates.io] and `cargo update`
* Built in code formatting via `cargo fmt`
* Built in linting via `cargo clippy`
* Built in testing framework via `cargo test`
* High quality third party micro-benchmarking - `criterion` and `cargo bench`
* Code coverage - `tarpaulin` and `coveralls.io`
* Continuous integration - `travis-ci`

Setting up and maintaining this kind of infrastructure for a C++ project is a
huge amount of work, especially for something I'm just doing in my evenings.

Rust is pretty new and there are certainly a lot of gaps in the tooling compared
to established languages like C++, but the out of the box experience is far far
superior.

## glam without SSE2

`glam`'s performance is around on par with `cgmath` if SSE2 is disabled using
the `scalar-math` feature. Some `glam` functions got faster without SIMD.

| benchmark           |         glam   |       cgmath   |     nalgebra   |
|:--------------------|---------------:|---------------:|---------------:|
| euler 2d            |     9.033 us   |   __8.985 us__ |     26.62 us   |
| euler 3d            |   __28.89 us__ |     29.63 us   |     195.6 us   |
| mat2 determinant    |    1.0632 ns   |  __1.0544 ns__ |    1.0545 ns   |
| mat2 inverse        |  __2.2956 ns__ |    2.7002 ns   |    2.6893 ns   |
| mat2 mul mat2       |    3.0813 ns   |  __3.0256 ns__ |    3.6432 ns   |
| mat2 transform vec2 |    2.4044 ns   |  __2.4022 ns__ |    6.9133 ns   |
| mat2 transpose      |  __1.3312 ns__ |    1.3319 ns   |    1.8201 ns   |
| mat3 determinant    |    2.5810 ns   |  __2.5352 ns__ |    2.5799 ns   |
| mat3 inverse        |   10.5985 ns   |  __8.0039 ns__ |    9.3055 ns   |
| mat3 mul mat3       |    9.5327 ns   |    9.4730 ns   |  __7.9314 ns__ |
| mat3 transform vec3 |    4.0703 ns   |  __4.0665 ns__ |    8.0555 ns   |
| mat3 transpose      |    3.5448 ns   |  __3.5228 ns__ |    8.9625 ns   |
| mat4 determinant    |  __7.7088 ns__ |   12.5664 ns   |   54.2749 ns   |
| mat4 inverse        | __31.3339 ns__ |   43.7255 ns   |   55.6187 ns   |
| mat4 mul mat4       |  __9.2925 ns__ |    9.3097 ns   |   15.6004 ns   |
| mat4 transform vec4 |  __3.5887 ns__ |    3.6127 ns   |    4.1613 ns   |
| mat4 transpose      |  __7.6722 ns__ |    8.6266 ns   |   10.7375 ns   |
| quat conjugate      |  __1.7617 ns__ |    1.7622 ns   |    1.7684 ns   |
| quat mul quat       |  __5.1730 ns__ |    5.4205 ns   |    5.5644 ns   |
| quat transform vec3 |    7.1421 ns   |  __6.6010 ns__ |    7.0346 ns   |
| vec3 cross          |  __2.8412 ns__ |    2.8485 ns   |    3.2746 ns   |
| vec3 dot            |    1.6708 ns   |  __1.5822 ns__ |    1.6566 ns   |
| vec3 length         |  __2.0307 ns__ |    2.0315 ns   |   84.6475 ns   |
| vec3 normalize      |  __4.1255 ns__ |    4.1301 ns   |   85.3411 ns   |

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
[`cargo-asm`]: https://github.com/gnzlbg/cargo-asm
[`packed_simd`]: https://rust-lang-nursery.github.io/packed_simd/packed_simd/
[How to write a maths library in 2016]: http://www.codersnotes.com/notes/maths-lib-2016/
[Aras Pranckevičius's pathtracer blog series]: https://aras-p.info/blog/2018/04/10/Daily-Pathtracer-Part-7-Initial-SIMD/
[Realtime Math]: https://github.com/nfrechette/rtm
[DirectXMath]: https://docs.microsoft.com/en-us/windows/desktop/dxmath/directxmath-portal
[DirectX Tool Kit SimpleMath]: https://github.com/Microsoft/DirectXTK/wiki/SimpleMath
[GLM]: https://glm.g-truc.net/
[crates.io]: https://crates.io
