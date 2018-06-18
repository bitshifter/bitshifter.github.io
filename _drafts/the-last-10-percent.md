---
layout: post
title:  "Optimising path tracing: the last 10%"
categories: blog
---

In my last post on [optimising my Rust path tracer with SIMD]({{ site.baseurl }}{% post_url 2018-06-04-simd-path-tracing %}) I had got withing 10% of my performance target, that is Aras's C++ SSE4.1 path tracer. From profiling I had determined that the main differences were MSVC using SSE versions of `sinf` and `cosf` and differences between Rayon and enkiTS thread pools. The first thing I tried was implement an [SSE2 version of `sin_cos`](https://github.com/bitshifter/pathtrace-rs/blob/sse_sincos/src/simd.rs#L66) based off of [Julien Pommier's code](http://gruntthepeon.free.fr/ssemath/sse_mathfun.h) that I found via a bit of googling. This was enough to get my SSE4.1 implementation to match the performance of Aras's SSE4.1 code. I had a slight advantage in that I just call `sin_cos` as a single function versus separate `sin` and `cos` functions, but meh, I'm calling the performance target reached.

The other part of this post is about Rust's runtime and compile time CPU feature detection and some wrong turns I took along the way.

# Target feature selection

As I mentioned in my previous post, Rust is about to stabilise several SIMD related features in 1.27.Apart from the SIMD intrinsics themselves, the ability to check for CPU target features (i.e. supported instruction sets) at both compile time and runtime.

The static check involves using `#[cfg(target_feature = "<feature>")]` around blocks which turns code on or off at compile time. Here's an example of compile time feature selection:

```rust
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn add_scalar(a: &[f32], b: &[f32], c: &mut [f32]) {
    for ((a, b), c) in a.iter().zip(b.iter()).zip(c.iter_mut()) {
        *c = a + b;
    }
}

#[cfg(target_feature = "sse2")]
pub fn add_sse2(a: &[f32], b: &[f32], c: &mut [f32]) {
    // for simplicity assume length is a multiple of chunk size
    for ((a, b), c) in a.chunks(4).zip(b.chunks(4)).zip(c.chunks_mut(4)) {
        unsafe {
            _mm_storeu_ps(
                c.as_mut_ptr(),
                _mm_add_ps(
                    _mm_loadu_ps(a.as_ptr()),
                    _mm_loadu_ps(b.as_ptr())));
        }
    }
}

#[cfg(target_feature = "avx2")]
pub fn add_avx2(a: &[f32], b: &[f32], c: &mut [f32]) {
    // for simplicity assume length is a multiple of chunk size
    for ((a, b), c) in a.chunks(8).zip(b.chunks(8)).zip(c.chunks_mut(8)) {
        unsafe {
            _mm256_storeu_ps(
                c.as_mut_ptr(),
                _mm256_add_ps(
                    _mm256_loadu_ps(a.as_ptr()),
                    _mm256_loadu_ps(b.as_ptr())));
        }
    }
}

pub fn add(a: &[f32], b: &[f32], c: &mut [f32]) {
    #[cfg(target_feature = "avx2")]
    return add_avx2(a, b, c);
    #[cfg(target_feature = "sse2")]
    return add_sse2(a, b, c);
    #[cfg(not(any(target_feature = "avx2", target_feature = "sse2")))]
    add_scalar(a, b, c);
}
```

If you look at the assembly listing on [godbolt](https://godbolt.org/g/9DGsCy) you can see both `add_scalar` and `add_sse2` are both in the assembly output (SSE2 is always available on x86-x64 targets) but `add_avx2` is not as AVX2 is not. The `add` function has inlined the SSE2 version. The target features available it compile time can be controlled via `rustc` flags.

The runtime check uses the `#[target_feature = "<feature>"]` attribute that can be used on functions to emit code using that feature. The functions must be marked as `unsafe` as if they were executed on a CPU that didn't support that feature the program would crash. The `is_x86_feature_detected!` can be used to determine if the feature is available at runtime and then call the appropriate function. Here's an example of runtime feature selection:

```rust
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn add_scalar(a: &[f32], b: &[f32], c: &mut [f32]) {
    for ((a, b), c) in a.iter().zip(b.iter()).zip(c.iter_mut()) {
        *c = a + b;
    }
}

#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), target_feature(enable = "sse2"))]
pub unsafe fn add_sse2(a: &[f32], b: &[f32], c: &mut [f32]) {
    // for simplicity assume length is a multiple of chunk size
    for ((a, b), c) in a.chunks(4).zip(b.chunks(4)).zip(c.chunks_mut(4)) {
        _mm_storeu_ps(
            c.as_mut_ptr(),
            _mm_add_ps(
                _mm_loadu_ps(a.as_ptr()),
                _mm_loadu_ps(b.as_ptr())));
    }
}

#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), target_feature(enable = "avx2"))]
pub unsafe fn add_avx2(a: &[f32], b: &[f32], c: &mut [f32]) {
    // for simplicity assume length is a multiple of chunk size
    for ((a, b), c) in a.chunks(8).zip(b.chunks(8)).zip(c.chunks_mut(8)) {
        _mm256_storeu_ps(
            c.as_mut_ptr(),
            _mm256_add_ps(
                _mm256_loadu_ps(a.as_ptr()),
                _mm256_loadu_ps(b.as_ptr())));
    }
}

pub fn add(a: &[f32], b: &[f32], c: &mut [f32]) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { add_avx2(a, b, c) };
        }
        if is_x86_feature_detected!("sse2") {
            return unsafe { add_sse2(a, b, c) };
        }
    }
    add_scalar(a, b, c);
}
```

Looking again at the assembly listing on [godbolt](https://godbolt.org/g/h4WyUG) you can see a call to `std::stdsimd::arch::detect::os::check_for` which depending on the result jumps to the AVX2 implementation or passes through to the inlined SSE2 implementation (again, because SSE2 is always available on x86-64).

I do not know what the Rust calling convention is but there seems to be a lot of saving and restoring registers to and from the stack around the call to `check_for`. So there's definitely some overhead in performing this check at runtime and that's certainly noticeable comparing the results of my compile time AVX2 performance (51.1Mrays/s) to runtime checked AVX2 performance (47.6Mrays/s). If you know exactly what hardware your program is going to be running on it's better to use the compile time method. In my case, the `hit` method is called millions of times so this overhead adds up. If I could perform this check in an outer function then it should reduce the overhead of the runtime check.

# Final performance results

Test results are from my laptop running Window 10 home. I'm compiling with `cargo build --release` of course with `rustc 1.28.0-nightly (71e87be38 2018-05-22)`. My performance numbers for each iteration of my path tracer are:

| Feature                   | Mrays/s |
| ------------------------- | -------:|
| Aras's C++ SSE4.1 version |    45.5 |
| Static SSE4.1 w/ LTO      |    45.8 |
| Static AVX2 w/ LTO        |    51.1 |
| Dynamic (AVX2) w/ LTO     |    47.6 |

The static branch lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_static) and the dynamic branch [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_dynamic). My machine supports AVX2 so that's what the dynamic version ends up using.
