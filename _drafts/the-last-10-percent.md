---
layout: post
title:  "Optimising path tracing: the last 10%"
categories: blog
---

In my last post on [optimising my Rust path tracer with SIMD]({{ site.baseurl }}{{% post_url 2018-06-04-simd-path-tracing %}}) I had got withing 10% of my performance target - Aras's C++ SSE4.1 path tracer. From profiling I had determined that the main differences were MSVC linking SSE versions of `sinf` and `cosf` and differences between Rayon and enkiTS thread pools. The first thing I did was implement an [SSE2 version of `sin_cos`](https://github.com/bitshifter/pathtrace-rs/blob/sse_sincos/src/simd.rs#L66) based off of [Julien Pommier's code](http://gruntthepeon.free.fr/ssemath/sse_mathfun.h) that I found via a bit of googling. This was enough to get my SSE4.1 implementation to match the performance of Aras's SSE4.1 code. I had a slight advantage in that I just call `sin_cos` as a single function versus separate `sin` and `cos` functions, but meh, I'm calling the performance target reached.

The other part of this post is about Rust's runtime and compile time CPU feature detection and some wrong turns I took along the way.

# Target feature selection


# Final performance results

Test results are from my laptop running Window 10 home. I'm compiling with `cargo build --release` of course with `rustc 1.28.0-nightly (71e87be38 2018-05-22)`. My performance numbers for each iteration of my path tracer are:

| Feature                   | Mrays/s |
| ------------------------- | -------:|
| Aras's C++ SSE4.1 version |    45.5 |
| Static SSE4.1 w/ LTO      |    45.8 |
| Static AVX2 w/ LTO        |    51.1 |
| Dynamic (AVX2) w/ LTO     |    47.6 |

The static branch lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_static) and the dynamic branch [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_dynamic). My machine supports AVX2 so that's what the dynamic verison ends up using.
