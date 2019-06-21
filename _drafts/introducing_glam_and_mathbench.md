---
layout: post
title:  "Introducing glam and mathbench"
excerpt_separator: <!--more-->
tags: rust simd math performance
---

`glam` is a simple and fast Rust 3D math library for games and graphics.
`mathbench` is a set of unit tests and criterion benchmarks comparing the
performance of `glam` with the popular Rust math libraries `cgmath` and
`nalgebra`.

# Why write another math library?

I had a couple of guiding principles for writing a new library, speed and
simplicity:

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
