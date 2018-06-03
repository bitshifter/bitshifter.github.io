---
layout: post
title:  "Optimising path tracing with SIMD"
categories: blog
---

Following on from [path tracing in parallel with Rayon]({{ site.baseurl }}{% post_url 2018-05-07-path-tracing-in-parallel %}) I had a lot of other optimisations I wanted to try. In particular I want to see if I could match the CPU performance of [@aras_p](https://twitter.com/aras_p)'s [C++ path tracer](https://github.com/aras-p/ToyPathTracer) in Rust. He'd done a fair amount of optimising so it seemed like a good target to aim for. To get a better comparison I copied his scene and also added his light sampling approach which he talks about [here](http://aras-p.info/blog/2018/03/28/Daily-Pathtracer-Part-1-Initial-C--/). I also implemented a live render loop mimicing his.

![the final result](/public/img/output_lit.png)

My initial unoptimized code was processing 10Mrays/s on my laptop. Aras' code (with GPGPU disabled) was doing 45.5Mrays/s. I had a long way to go from here! My unoptimized code can be found on [this branch](https://github.com/bitshifter/pathtrace-rs/tree/emissive).

tl;dr did I match the C++ in Rust? Almost. My SSE4.1 version is doing 41.2Mrays/s about 10% slower than the target 45.5Mrays/s running on Windows on my laptop. The long answer is more complicated but I will go into that later. My fully (so far) optimised version lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_wrapped).

# Process

This is potentially going to turn into a long post, so as an outline my process went like:

* Read up on SIMD support in Rust
* Convert my array of sphere structs (AoS) to structure of arrays (SoA)
* Rewrite the math part of ray sphere intersection in SSE2 using scalar code to extract the result
* Examined how Aras' code extracted the hit result in SIMD and copied that with a few modifications
* Fixed a dumb performance bug
* Made my SoA spheres use aligned memory
* Wrote a wrapper for the SSE2 code
* Added an AVX2 implementation of my wrapper which is used if AVX2 is enabled at compile time
* Rewrote my Vec3 class to use SSE2 under the hood

At each step I was checking performance to see how far off the target I was.

# SIMD

A big motivation for me doing this was to write some SIMD code. I knew what SIMD was but I'd never actually written any, or none of significance at least.

If you are unfamiliar with SIMD, it stands for Single Instruction Multiple Data. What this means is there are registers on most common CPUs which contain multiple values, e.g. 4 floats, that can be processed with a single instruction.

![SIMD add](https://mirrors.edge.kernel.org/pub/linux/kernel/people/geoff/cell/ps3-linux-docs/CellProgrammingTutorial/CellProgrammingTutorial.files/image009.jpg)

The image above shows adding two SIMD vectors which each contain 4 elements together in the single instruction. This increases the througput of math heavy code. On Intel SSE2 has been avaiable since 2001, providing 128 bit registers which can hold 4 floats or 2 doubles. More recent chips have AVX which added 256 bit registers and AVX512 which added 512 bit registers.

In ths path tracer we're performing collision detection of a ray against an array of spheres, one sphere at a time. With SIMD we could check 4 or even 8 spheres at a time. That sounds like a good performance boost!

There's a few good references I'm using for this (aside from Aras' code):
 * [Intel Intrinsics Guide](https://software.intel.com/sites/landingpage/IntrinsicsGuide/#)
 * [Rust std::arch Module documentation](https://doc.rust-lang.org/std/arch/index.html)
 * [Agner Fog's Instruction tables with latencies, throughput and micro-ops](http://www.agner.org/optimize/instruction_tables.pdf)

# SIMD in Rust

Rust SIMD support is available in the nightly compiler and should make it into a stable Rust release soon. There are a couple of related RFCs around SIMD support.

* Target feature [RFC#2045](https://github.com/rust-lang/rfcs/blob/master/text/2045-target-feature.md) adds the ability to:
  * determine which features (CPU instructions) are available at compile time
  * determine which features are available at run-time
  * embed code for different sets of features in the same binary

* Stable SIMD [RFC#2325](https://github.com/rust-lang/rfcs/blob/master/text/2325-stable-simd.md) adds support for:
  * 128 and 256 bit vector types and intrinsics for Intel chipsets, e.g. SSE and AVX - other instruction sets will be added later
  * the ability to branch on target feature type at runtime so you can use a feature if it's available on the CPU the program is running on

One thing that is not currently being stabilised is cross platform abstraction around different architecture's SIMD instructions. I'm sticking with SSE for now as I want to get experience with a native instruction set rather than an abstraction.

Another thing worth mentioning is the SIMD instrinsics are all labelled unsafe, using intrinsics is a bit like calling FFI functions, they aren't playing by Rust's rules and are thus unsafe. Optimised code often sacrifices a bit of safety and readability for speed, this will be no exception.

# Converting `Vec3` to SIMD

Most of the advice I've heard around using SIMD is just making your math types use it not the way to get performance and that you are better to just write SIMD math code without wrappers. One reason is `Vec3` is only using 3 of the available SIMD lanes, so even on SSE you're only 75% occupancy and you won't get any benefit from larger registers. Another reason is components in a Vec3 are related but values in SIMD vector lanes have no semantic relationship. In practice what this means is doing operations across lanes like a dot product is cumbersome and not that efficient. See "The Awkward Vec4 Dot Product" slide on page 43 of this [GDC presentation](https://deplinenoise.files.wordpress.com/2015/03/gdc2015_afredriksson_simd.pdf).

Given all of the above it's not surprising that Aras [blogged](http://aras-p.info/blog/2018/04/10/Daily-Pathtracer-Part-7-Initial-SIMD/) that he didn't see much of a gain from converting his `float3` struct to SIMD. It's actually one of the last things I implemented to try and reach my performance target. I followed the same post he did on ["How to write a maths library in 2016"](http://www.codersnotes.com/notes/maths-lib-2016/) except for course in Rust rather than C++. This actually gave me a pretty big boost, from 10Mrays/s to 20.7Mrays/s. That's a large gain so why did I see this when Ara's C++ version only saw a slight change?

I think the answer has to do with my next topic.

# Floating point in Rust

I think every game I've ever worked on has enabled to the fast math compiler setting, `-ffast-math` on GCC/Clang or `/fp:fast` on MSVC. This setting sacrifices IEEE 754 conformance to allow the compiler to optimise floating point operations more aggressively. Agner Fog recently released a paper on [NaN propogation](http://www.agner.org/optimize/nan_propagation.pdf) which talks about the specifics of what `-ffast-math` actually enables. There is currently no equivalent of `-ffast-math` for the Rust compiler and that doesn't look like something that will change any time soon. There's a bit of discussion about how Rust might support it, there are currently [intrinsics](https://doc.rust-lang.org/core/intrinsics/index.html) for fast float operations which you can use in nightly but no plans for stable Rust. I found a bit of discussion on the Rust internals forums like [this post](https://internals.rust-lang.org/t/avoiding-partialord-problems-by-introducing-fast-finite-floating-point-types) but it seems like early days.

My theory on why I saw a large jump going from precise IEEE 754 floats to SSE was in part due to the lack of `-ffast-math` optimisations. This is speculation on my part though, I haven't done any digging to back it up.

A more well known floating point wart in Rust is the distinction between `PartialOrd` and `Ord` traist. This distinction exists because floating point can be `Nan` which means that it doesn't support [total order](https://en.wikipedia.org/wiki/Total_order) which is required by `Ord` but not `PartialOrd`. As a consequence you can't use a lot of standard library functions like [sort](https://doc.rust-lang.org/std/primitive.slice.html#method.sort) with floats. This is an ergonomics issue rather than a performance issue but it is one I ran into working on this so I thought I'd mention it.

# SIMD ray sphere hit test

As I mentioned earlier, SSE2 should allow testing a ray against 4 spheres at a time, sounds simple enough. The original scalar hit test code looks like so:

```rust
impl Sphere {
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<RayHit> {
        let oc = ray.origin - self.centre;
        let a = dot(ray.direction, ray.direction);
        let b = dot(oc, ray.direction);
        let c = dot(oc, oc) - self.radius * self.radius;
        let discriminant = b * b - a * c;
        if discriminant > 0.0 {
            let discriminant_sqrt = discriminant.sqrt();
            let t = (-b - discriminant_sqrt) / a;
            if t < t_max && t > t_min {
                let point = ray.point_at_parameter(t);
                let normal = (point - self.centre) / self.radius;
                return Some(RayHit { t, point, normal });
            }
            let t = (-b + discriminant_sqrt) / a;
            if t < t_max && t > t_min {
                let point = ray.point_at_parameter(t);
                let normal = (point - self.centre) / self.radius;
                return Some(RayHit { t, point, normal });
            }
        }
        None
    }
}
```

Reasonably straight forward, the first five lines are simple math operations before some conditionals to determine if the ray hit or not. To convert this to SSE2 first we need to load the data into the SSE `__m128` format, do the math bit and finally extract which one of the SIMD lanes contains the result we're after (the nearest hit point).

First we need to splat x, y, and z components of the ray origin and direction into SSE `__m128` variables. They we need to iterate over 4 spheres at a time, loading 4 x, y and z components of the sphere's centre points into SIMD variables containing 4 x components, 4 y components and so on.

We use the `_mm_set_ps1` intrinsic to splat each ray origin and direction component into across 4 lanes of a SIMD variable.

```rust
// load ray origin
let ro_x = _mm_set_ps1(ray.origin.get_x());
let ro_y = _mm_set_ps1(ray.origin.get_y());
let ro_z = _mm_set_ps1(ray.origin.get_z());
// load ray direction
let rd_x = _mm_set_ps1(ray.direction.get_x());
let rd_y = _mm_set_ps1(ray.direction.get_y());
let rd_z = _mm_set_ps1(ray.direction.get_z());
```

This is probably a good time to talk about SoA.

# Structure of arrays

There are 3 ways that I'm aware of to load the sphere data into SSE vectors:

* `_mm_set_ps` which takes 4 f32 parameters
* `_mm_loadu_ps` which takes an unaligned pointer to 4 floats 
* `_mm_load_ps` which takes a 16 byte aligned pointer to 4 floats

I did a little experiement to see what the assembly generated by these might look like ([playground](https://play.rust-lang.org/?gist=d4dcd07c3fea0139e6617cf6b83ba634&version=nightly&mode=release)):

```rust
pub fn set_and_add(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32, g: f32, h: f32) -> __m128 {
  unsafe {
    _mm_add_ps(
        _mm_set_ps(a, b, c, d),
        _mm_set_ps(e, f, g, h))
  }
}

pub fn load_unaligned_and_add(a: &[f32;4], b: &[f32;4]) -> __m128 {
  unsafe {
      _mm_add_ps(
          _mm_loadu_ps(a as *const f32),
          _mm_loadu_ps(b as *const f32))
  }
}

pub fn load_aligned_and_add(a: &[f32;4], b: &[f32;4]) -> __m128 {
  unsafe {
      _mm_add_ps(
          _mm_load_ps(a as *const f32),
          _mm_load_ps(b as *const f32))
  }
}
```

And the output

```
playground::set_and_add:
  unpcklps	%xmm0, %xmm1
  unpcklps	%xmm2, %xmm3
  movlhps	%xmm1, %xmm3
  unpcklps	%xmm4, %xmm5
  unpcklps	%xmm6, %xmm7
  movlhps	%xmm5, %xmm7
  addps	%xmm3, %xmm7
  movaps	%xmm7, (%rdi)
  movq	%rdi, %rax
  retq

playground::load_unaligned_and_add:
  movups	(%rsi), %xmm0
  movups	(%rdx), %xmm1
  addps	%xmm0, %xmm1
  movaps	%xmm1, (%rdi)
  movq	%rdi, %rax
  retq

playground::load_aligned_and_add:
  movaps	(%rsi), %xmm0
  addps	(%rdx), %xmm0
  movaps	%xmm0, (%rdi)
  movq	%rdi, %rax
  retq
```

The first option has to do a lot of work to get data into registers before it can peform the `addps`. The last option appears to be doing the least work based on instruction count, the `addps` can take one of it's operands as a memory address because it's aligned correctly. 

My initial implementation was an array of `Sphere` objects. The `x` values for 4 spheres are not in contiguous memory, so I would be forced to use `_mm_set_ps` to load the data.  To use the `_mm_loadu_ps` I need to store my `Sphere` data in Structure of Arrays format:

```rust
pub struct SpheresSoA {
    centre_x: Vec<f32>,
    centre_y: Vec<f32>,
    centre_z: Vec<f32>,
    radius_sq: Vec<f32>,
    radius_inv: Vec<f32>,
    len: usize,
}
```

Now to iterator over four spheres at a time my loop looks like:

```rust
// loop over 4 spheres at a time
for (((centre_x, centre_y), centre_z), radius_sq) in self
  .centre_x
  .chunks(4)
  .zip(self.centre_y.chunks(4))
  .zip(self.centre_z.chunks(4))
  .zip(self.radius_sq.chunks(4))
{
  // load sphere centres
  let c_x = _mm_loadu_ps(centre_x.as_ptr());
  let c_y = _mm_loadu_ps(centre_y.as_ptr());
  let c_z = _mm_loadu_ps(centre_z.as_ptr());
  // load radius_sq
  let r_sq = _mm_loadu_ps(radius_sq.as_ptr());
  // ... everything else
}
```

I did try using `get_unchecked` on each array instead of `chunks(4)` and `zip` but at the time it appeared to be slower. I might try it again though as the iterator version is a bit obtuse and generates a lot of code. Note that I'm using `_mm_loadu_ps` because my `Vec<f32>` arrays are not 16 byte aligned. I'll talk about that later.

There are other advantages to SoA. In the above structure I'm storing precalculated values for `radius * radius` and `1.0 / radius` as if you look at the original `hit` function `radius` on it's own is never actually used. Before switching to SoA adding precalculated values to my `Sphere` struct actually made things slower, presumably because I'd increased the size of the `Sphere` struct, reducing my CPU cache utilisation. With `SpheresSoA` if the `determinant > 0.0` check doesn't pass then the `radius_inv` data will never be accessed, so it's not wastefully pulled into CPU cache. Switching to SoA did speed up my scalar code too although I didn't note down the performance numbers at the time.

# Getting the result out again

I won't paste the full SSE2 hit source here because it's quite verbose, but it can be found [here](https://github.com/bitshifter/pathtrace-rs/blob/spheres_simd/src/collision.rs#L173) if you're interested. I mostly wrote this from scratch using the [Intel Intrinsics Guide](https://software.intel.com/sites/landingpage/IntrinsicsGuide/#) to match my scalar code to the SSE2 equivalent and using Aras' code as a reference. I think I had got pretty close to completion at the end of one evening but wasn't sure how to extract the minimum hit result out of the SSE vector so just went for the scalar option:

```rust
// copy results out into scalar code to get return value (if any)
let mut hit_t_array = [t_max, t_max, t_max, t_max];
_mm_storeu_ps(hit_t_array.as_mut_ptr(), hit_t);
// this is more complicated that it needs to be because I couldn't
// just use 'min` as f32 doesn't support `Ord` the trait
let (hit_t_lane, hit_t_scalar) = hit_t_array.iter().enumerate().fold(
    (self.len, t_max),
    |result, t| if *t.1 < result.1 { (t.0, *t.1) } else { result },
);
if hit_t_scalar < t_max {
  // get the index of the result in the SpheresSoA vectors and return it
}
```

All the above code is doing is finding the minimum value in the `hit_t` SIMD vector to see if we have a valid hit. Aras's code what able to do this in SSE2 with his `h_min` function (h stands for horizontal meaning acrsso the SIMD vector lanes), but I wanted to understand how it was working before I used it. The main reason I didn't understand `h_min` is I'd never used a shuffle before. I guess they're familiar to people who write a lot of shaders since swizzling is pretty common but I haven't done a lot of that either.

My Rust `hmin` looks like:

```rust
v = _mm_min_ps(v, _mm_shuffle_ps(v, v, _mm_shuffle!(0, 0, 3, 2)));
v = _mm_min_ps(v, _mm_shuffle_ps(v, v, _mm_shuffle!(0, 0, 0, 1)));
_mm_cvtss_f32(v)
```

What this is doing is combining `_mm_shuffle_ps` and `_mm_min_ps` to compare all of the lanes with each other and shuffle the minimum value into lane 0 (the lowest 32 bits of the vector).

The shuffle is using a macro that encodes where lanes are being shuffled from, it's a Rust implementation of the `_MM_SHUFFLE` macro found in `xmmintrin.h`:

```
#[macro_export]
macro_rules! _mm_shuffle {
    ($z:expr, $y:expr, $x:expr, $w:expr) => {
        ($z << 6) | ($y << 4) | ($x << 2) | $w
    };
}
```

This macro works kind of backwards from what I'd expect, so to reverse the order of the SIMD lanes you would do:

```rust
_mm_shuffle_ps(a, a, _mm_shuffle!(0, 1, 2, 3))
```

Which is saying move lane 0 to lane 3, lane 1 to lane 2, lane 2 to lane 1 and lane 3 to lane 0.

So given that, if we put some values into `hmin`:

```
let a = _mm_set_ps(0.0, 1.0, 2.0, 3.0);
// a = __m128(3.0, 2.0, 1.0, 0.0)
let b = _mm_shuffle_ps(a, a, _mm_shuffle!(0, 0, 3, 2));
// b = __m128(1.0, 0.0, 3.0, 3.0)
let c = _mm_min_ps(a, b);
// c = __m128(1.0, 0.0, 1.0, 0.0)
let d = _shuffle_ps(c, c, _mm_shuffle!(0, 0, 0, 1));
// d = __m128(0.0, 1.0, 1.0, 1.0)
let e = _mm_min_ps(c, d);
// e = __m128(0.0, 0.0, 1.0, 0.0)
let f = _mm_cvtss_f32(e);
// f = 0
```

# Alignment

repr align wrapper. Probably could just use a union...

# SIMD wrapper and AVX2

The hit spheres code doesn't really care about width, easy to support AVX2.

Wrapper

How it's included

No compile time switching yet and not sure how to do it.

# Floats in Rust

No -ffast-math

Agner link

PartialOrd and Ord

sin and cos on MSVC

Precision and smallpt scene

Rayon versus Enki

# Final performance results

Laptop weirdness

Approx numbers
