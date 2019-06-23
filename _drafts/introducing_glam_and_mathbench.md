---
layout: post
title:  "Introducing glam and mathbench"
excerpt_separator: <!--more-->
tags: rust simd math performance
---

`glam` is a simple and fast Rust 2D and 3D math library for games and graphics.
`mathbench` is a set of unit tests and criterion benchmarks comparing the
performance of `glam` with the popular Rust math libraries `cgmath` and
`nalgebra`.

The following is a table of benchmarks produced by `mathbench` comparing `glam`
peformance to `cgmath` and `nalgebra`.

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

These benchmarks were performed on my [Intel i7-4710HQ
CPU](https://ark.intel.com/content/www/us/en/ark/products/78930/intel-core-i7-4710hq-processor-6m-cache-up-to-3-50-ghz.html) under Linux.

# Why write another math library?

I had a couple of guiding principles for writing a new library, speed and
simplicity.


## Speed

One thing I noticed when optimising a path tracer was that I got some good
performance wins from implementing a `Vector3` with `SSE2`. I wanted to take
that further and build a full library that utilised SIMD for vector, matrix and
quaternion types. I'd seen Rust math libraries that use SIMD for some functions
but I hadn't see one that uses SIMD vectors for storage.

## Simplicity

I wanted to come up with a very simple API which is very low friction for
developers to use. Something that covers the common needs for someone working in
games and graphics and doesn't require much effort learn. I also wanted
something to was easy low friction to create.

From the outset I wanted to avoid using traits and generics. Traits and generics
are wonderful language features, but I didn't see a great reason to use them in
`glam`.

For one I wanted to use SSE2 for storage, which means a generic for the scalar
type (e.g. `f32`) and a generic for the storage type (`__m128` if available),
that already sounds complicated! In 15 years of working in games I can only
think of one occasion where a generic vector type would have been useful which
was porting code to some prototype hardware that didn't have an FPU, not exactly
a common occurence.  Vectors of `i32` would have a reduced set of operations,
e.g. normalizing an vector of integers is going to result in a zero vector
except for the unit axes, which isn't super useful. One way of getting around
that is using traits for operations that require real numbers, but that
introduces complexity that your users need to learn for functionality that they
may never use.

That's not to say that `glam` will never contain vectors of integer or generic
types, but for the sake of simplicity I wanted to avoid them for a while until
the API feels stable and there's a compelling reason to use them.

I also wanted to make `glam` "Rusty". One thing I've noticed about the Rust
standard library is everything is a method, for example `sin` is a method on
`f32` and `f64`. It feels a bit weird coming from other languages. It turns out
there are some reasons for this, from the [Rust API guidelines]:

> Methods have numerous advantages over functions:
> * They do not need to be imported or qualified to be used: all you need is a
>   value of the appropriate type.
> * Their invocation performs autoborrowing (including mutable borrows).
> * They make it easy to answer the question "what can I do with a value of type
>   T" (especially when using rustdoc).
> * They provide self notation, which is more concise and often more clearly
>   conveys ownership distinctions.

Things like `a.lerp(b, 0.5)` feel a bit odd, but I could always add functions
for this kind of thing in addition to the methods.

I've tried to follow the Rust API Guidelines in general.

## SSE2 and Scalar support

One of the nice things about generics and traits is having a consistent
interface not matter what the type is. In `glam` I wanted to have `SSE2`
and scalar implementations. For the moment I have these defined in separate
modules, one of which will be imported depending on what target features are
available:

```rust
mod vec4;
#[cfg(any(not(target_feature = "sse2"), feature = "scalar-math"))]
mod vec4_f32;
#[cfg(all(target_feature = "sse2", not(feature = "scalar-math")))]
mod vec4_sse2;
```

This avoids having `#[cfg]` blocks littered through all of the implementation,
but it does have it's down sides. All of the interface is duplicated between the
different implementations, which means they could get out of sync. To try and
mitigate that I've tried to reach 100% test coverage. This structure does
confuse `rustdoc` a little too. A may move to a single implementation with all
the `#[cfg]` noise long term though, it doesn't make a difference to the
external API.

## Test Coverage

To determine if I actually did have 100% test coverage I've been using a cargo
plugin called [`tarpaulin`]. It only supports **x86_64** processors running
Linux and does give some false positives, but it's been pretty good for my
needs. I have it integrated into my [`travis-ci`] build and posting results to
[`coveralls.io`].

## Performance

If you want to say your library is fast, you better be measuring perfomance.

[Rust API Guidelines]: https://rust-lang-nursery.github.io/api-guidelines/predictability.html#c-method
[`tarpaulin`]: https://github.com/xd009642/tarpaulin
[`travis-ci`]: https://travis-ci.org/bitshifter/glam-rs
[`coveralls.io`]: https://coveralls.io/github/bitshifter/glam-rs
