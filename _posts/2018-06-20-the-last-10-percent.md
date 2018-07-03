---
layout: post
title:  "Optimising path tracing: the last 10%"
excerpt_separator: <!--more-->
tags: rust simd raytracing
---

In my last post on [optimising my Rust path tracer with SIMD]({{ site.baseurl }}{% post_url 2018-06-04-simd-path-tracing %}) I had got withing 10% of my performance target, that is Aras's C++ SSE4.1 path tracer. From profiling I had determined that the main differences were MSVC using SSE versions of `sinf` and `cosf` and differences between Rayon and enkiTS thread pools. The first thing I tried was implement an [SSE2 version of `sin_cos`](https://github.com/bitshifter/pathtrace-rs/blob/sse_sincos/src/simd.rs#L66) based off of [Julien Pommier's code](http://gruntthepeon.free.fr/ssemath/sse_mathfun.h) that I found via a bit of googling. This was enough to get my SSE4.1 implementation to match the performance of Aras's SSE4.1 code. I had a slight advantage in that I just call `sin_cos` as a single function versus separate `sin` and `cos` functions, but meh, I'm calling my performance target reached. [Final performance results]({{ site.baseurl }}{% post_url 2018-06-20-the-last-10-percent %}#final-performance-results) are at the end of this post if you just want to skip to that.

The other part of this post is about Rust's runtime and compile time CPU feature detection and some wrong turns I took along the way.

<!--more-->

# Target feature selection

As I mentioned in my previous post, Rust is about to stabilise several SIMD related features in 1.27. Apart from the SIMD intrinsics themselves, this adds the ability to check for CPU target features (i.e. supported instruction sets) at both compile time and runtime.

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

If you look at the assembly listing on [Compiler Explorer](https://godbolt.org/g/9DGsCy) you can see both `add_scalar` and `add_sse2` are both in the assembly output (SSE2 is always available on x86-x64 targets) but `add_avx2` is not as AVX2 is not. The `add` function has inlined the SSE2 version. The target features available it compile time can be controlled via `rustc` flags.

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

Looking again at the assembly listing on [Compiler Explorer](https://godbolt.org/g/h4WyUG) you can see a call to `std::stdsimd::arch::detect::os::check_for` which depending on the result jumps to the AVX2 implementation or passes through to the inlined SSE2 implementation (again, because SSE2 is always available on x86-64).

I do not know what the Rust calling convention is but there seems to be a lot of saving and restoring registers to and from the stack around the call to `check_for`.

```
example::add:
  pushq %rbp
  pushq %r15
  pushq %r14
  pushq %r13
  pushq %r12
  pushq %rbx
  pushq %rax
  movq %r9, %r12
  movq %r8, %r14
  movq %rcx, %r13
  movq %rdx, %r15
  movq %rsi, %rbp
  movq %rdi, %rbx
  movl $15, %edi
  callq std::stdsimd::arch::detect::os::check_for@PLT
  testb %al, %al
  je .LBB3_1
  movq %rbx, %rdi
  movq %rbp, %rsi
  movq %r15, %rdx
  movq %r13, %rcx
  movq %r14, %r8
  movq %r12, %r9
  addq $8, %rsp
  popq %rbx
  popq %r12
  popq %r13
  popq %r14
  popq %r15
  popq %rbp
  jmp example::add_avx2@PLT
```

So there's definitely some overhead in performing this check at runtime and that's certainly noticeable comparing the results of my compile time AVX2 performance (51.1Mrays/s) to runtime checked AVX2 performance (47.6Mrays/s). If you know exactly what hardware your program is going to be running on it's better to use the compile time method. In my case, the `hit` method is called millions of times so this overhead adds up. If I could perform this check in an outer function then it should reduce the overhead of the runtime check.

## Update - 2018-06-23

There was some great discussion on [/r/rust](https://www.reddit.com/r/rust/comments/8sexqk/optimising_path_tracing_with_simd_the_last_10/) about this point. It seems like this check should be getting inlined and that it wasn't is probably a bug. A [PR](https://github.com/rust-lang-nursery/stdsimd/pull/480) has already landed in the `stdsimd` library to fix this, thanks to [gnzlbg](https://github.com/gnzlbg). It might take a while for that change to make it into stable Rust. In the interim there were a couple of other good suggestions on how I could minimise the overhead. One was to store the target feature detection result in an enum and just use a match to branch to the appropriate function. This was on the premise that the branch should get predicted and avoiding an indirect jump (function pointer) would allow the compiler to optimise better. This has certainly worked out and now my runtime target feature code performance is the same as the compile time version.

# Wrappers and runtime feature selection

In my previous post I also talked about my [SIMD wrapper]({{ site.baseurl }}{% post_url 2018-06-04-simd-path-tracing %}#simd-wrapper-with-avx2-support). The purpose of this wrapper was to provide an interface that uses the widest available registers to it, the idea being I write my ray spheres collision test function using my wrapped SIMD types. The main benefits being that I have single implementation of my `hit` function using the wrapper type and also I get some nice ergonomics like being able to use operators like `+` or `-` on my wrapper types. Not to mention enforcing some type safety! For example introducing a SIMD boolean type that is returned by comparison functions and is then passed into blend functions - making the implicit SSE conventions explicit through the type system.

That's all very nice, but it only worked at compile time. The catch being runtime `#[target_feature(enable)]` only works on functions. My compile time wrapper just imported a different module depending on what features were available. My wrapper types had a lot of functions. On top of that the `hit` function needed to know which version of the wrapper it needed to call.

I was interested in implementing a runtime option. My first attempt at SIMD was using the runtime `target_feature` method but I had dropped it to try and write a wrapper. I was skeptical that I would be able to get my wrapper working with runtime feature detection, at least not without things getting very complicated - but I thought I'd give it a try. Keep in mind, I haven't completed this code, I just went a long way down this path before I moved on to writing separate scalar, SSE4.1 and AVX2 implementations of my hit function.

I figured one approach would be to make the hit function generic and then specify the SSE4.1 or AVX2 implementations as type parameters. I could then choose the appropriate branch at runtime. For this to work I needed to make traits for my wrapper types and all the trait methods needed to be unsafe, due to the `target_feature` requirement. Possibly these traits could have been simplified, but this is what I ended up with:

```rust
pub trait Bool32xN<T>: Sized + Copy + Clone + BitAnd + BitOr {
    unsafe fn num_lanes() -> usize;
    unsafe fn unwrap(self) -> T;
    unsafe fn to_mask(self) -> i32;
}

pub trait Int32xN<T, BI, B: Bool32xN<BI>>: Sized + Copy + Clone + Add + Sub + Mul {
    unsafe fn num_lanes() -> usize;
    unsafe fn unwrap(self) -> T;
    unsafe fn splat(i: i32) -> Self;
    unsafe fn load_aligned(a: &[i32]) -> Self;
    unsafe fn load_unaligned(a: &[i32]) -> Self;
    unsafe fn store_aligned(self, a: &mut [i32]);
    unsafe fn store_unaligned(self, a: &mut [i32]);
    unsafe fn indices() -> Self;
    unsafe fn blend(lhs: Self, rhs: Self, cond: B) -> Self;
}

pub trait Float32xN<T, BI, B: Bool32xN<BI>>: Sized + Copy + Clone + Add + Sub + Mul + Div {
    unsafe fn num_lanes() -> usize;
    unsafe fn unwrap(self) -> T;
    unsafe fn splat(s: f32) -> Self;
    unsafe fn from_x(v: Vec3) -> Self;
    unsafe fn from_y(v: Vec3) -> Self;
    unsafe fn from_z(v: Vec3) -> Self;
    unsafe fn load_aligned(a: &[f32]) -> Self;
    unsafe fn load_unaligned(a: &[f32]) -> Self;
    unsafe fn store_aligned(self, a: &mut [f32]);
    unsafe fn store_unaligned(self, a: &mut [f32]);
    unsafe fn sqrt(self) -> Self;
    unsafe fn hmin(self) -> f32;
    unsafe fn eq(self, rhs: Self) -> B;
    unsafe fn gt(self, rhs: Self) -> B;
    unsafe fn lt(self, rhs: Self) -> B;
    unsafe fn dot3(x0: Self, x1: Self, y0: Self, y1: Self, z0: Self, z1: Self) -> Self;
    unsafe fn blend(lhs: Self, rhs: Self, cond: B) -> Self;
}
```

The single letter type parameters are a bit cryptic, I was thinking about better names but was deferring that problem until later. The `hit` function signature also got quite complicated:

```rust
pub unsafe fn hit_simd<BI, FI, II, B, F, I>(
    &self,
    ray: &Ray,
    t_min: f32,
    t_max: f32,
) -> Option<(RayHit, u32)>
where
    B: Bool32xN<BI> + BitAnd<Output = B>,
    F: Float32xN<FI, BI, B> + Add<Output = F> + Sub<Output = F> + Mul<Output = F>,
    I: Int32xN<II, BI, B> + Add<Output = I> {
    // impl
}
```

Again with the cryptic type parameter names. These basically represented the wrapper type for `f32`, `bool` and `i32` and the arch specific SIMD type, e.g. `__m128` and `__m256`. This got pretty close to compiling but at this point I the operators didn't compile and because the operators defined in `std::ops` they aren't labelled as `unsafe`. I guess I could have wrapped an unsafe function call from the safe op trait implementation but that seemed pretty unsafe. At that point I gave up and added an unwrapped AVX2 version of the hit function. It took about an hour compared to several hours I'd sunk into my generic approach.

There are obvious downsides to maintaining multiple implementations of the same function. Adding some tests would help mitigate that but if I end up writing more types of ray collisions or add support for another arch I'm obviously adding a lot more code and potential sources of (maybe duplicated) bugs. I did feel at this point things were getting too complicated for not much gain in the scheme of my little path tracing project.

I thought this story might be interesting to people wanting to use the runtime target feature selection in Rust. It is nice having one executable that can take advantage of the CPU features that it's running on but it also means some constraints on how you author your code. The unfinished runtime wrapper code is [here](https://github.com/bitshifter/pathtrace-rs/commits/spheres_simd_traits) if you are curious.

# Final performance results

Test results are from my laptop running Window 10 home. I'm compiling with `cargo build --release` of course with `rustc 1.28.0-nightly (71e87be38 2018-05-22)`. My performance numbers for each iteration of my path tracer are:

| Feature                   | Mrays/s |
| ------------------------- | -------:|
| Aras's C++ SSE4.1 version |    45.5 |
| Static SSE4.1 w/ LTO      |    45.8 |
| Static AVX2 w/ LTO        |    52.1 |
| Dynamic (AVX2) w/ LTO     |    52.1 |

*Update 2018-06-23* Updated performance numbers for static and dynamic versions of the code - thanks to some caching of the target feature value the performance is about this same.

The static branch lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_static) and the dynamic branch [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_dynamic). My machine supports AVX2 so that's what the dynamic version ends up using.

In summary, if you don't know what CPU your code is going to run on you could get a nice little boost by checking target features at runtime at the loss of a bit of flexibility around how you structure your code. If you have a fixed hardware target then it's better to compile for that hardware and avoid the overhead and code restrictions of `target_feature`.

As always if you have any comments you can reach me on Twitter at [@bitshifternz](https://twitter.com/bitshifternz) or on [/r/rust](https://www.reddit.com/r/rust/comments/8sexqk/optimising_path_tracing_with_simd_the_last_10/).
