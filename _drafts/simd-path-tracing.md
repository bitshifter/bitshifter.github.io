---
layout: post
title:  "Optimising path tracing with SIMD"
categories: blog
---

Following on from [path tracing in parallel with Rayon]({{ site.baseurl }}{% post_url 2018-05-07-path-tracing-in-parallel %}) I had a lot of other optimisations I wanted to try. In particular I want to see if I could match the CPU performance of [@aras_p](https://twitter.com/aras_p)'s [C++ path tracer](https://github.com/aras-p/ToyPathTracer) in Rust. He'd done a fair amount of optimising so it seemed like a good target to aim for. To get a better comparison I copied his scene and also added his light sampling approach which he talks about [here](http://aras-p.info/blog/2018/03/28/Daily-Pathtracer-Part-1-Initial-C--/). I also implemented a live render loop mimicking his.

![the final result](/public/img/output_lit.png)

My initial unoptimized code was processing 10Mrays/s on my laptop. Aras's code (with GPGPU disabled) was doing 45.5Mrays/s. I had a long way to go from here! My unoptimized code can be found on [this branch](https://github.com/bitshifter/pathtrace-rs/tree/emissive).

tl;dr did I match the C++ in Rust? Almost. My SSE4.1 version is doing 41.2Mrays/s about 10% slower than the target 45.5Mrays/s running on Windows on my laptop. The long answer is more complicated but I will go into that later. My fully (so far) optimised version lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_wrapped).

# My Process

This is potentially going to turn into a long post, so as an outline my process went like:

* Read up on SIMD support in Rust
* Convert my array of sphere structs (AoS) to structure of arrays (SoA)
* Rewrite the math part of ray sphere intersection in SSE2 using scalar code to extract the result
* Examined how Aras's code extracted the hit result in SIMD and copied that with a few modifications
* Fixed a dumb performance bug
* Made my SoA spheres use aligned memory
* Wrote a wrapper for the SSE2 code
* Added an AVX2 implementation of my wrapper which is used if AVX2 is enabled at compile time
* Rewrote my `Vec3` class to use SSE2 under the hood

At each step I was checking performance to see how far off the target I was.

# SIMD

A big motivation for me doing this was to write some SIMD code. I knew what SIMD was but I'd never actually written any, or none of significance at least.

If you are unfamiliar with SIMD, it stands for Single Instruction Multiple Data. What this means is there are registers on most common CPUs which contain multiple values, e.g. 4 floats, that can be processed with a single instruction.

![SIMD add](https://mirrors.edge.kernel.org/pub/linux/kernel/people/geoff/cell/ps3-linux-docs/CellProgrammingTutorial/CellProgrammingTutorial.files/image009.jpg)

The image above shows adding two SIMD vectors which each contain 4 elements together in the single instruction. This increases the throughput of math heavy code. On Intel SSE2 has been available since 2001, providing 128 bit registers which can hold 4 floats or 2 doubles. More recent chips have AVX which added 256 bit registers and AVX512 which added 512 bit registers.

In this path tracer we're performing collision detection of a ray against an array of spheres, one sphere at a time. With SIMD we could check 4 or even 8 spheres at a time. That sounds like a good performance boost!

There's a few good references I'm using for this (aside from Aras's code):
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

Another thing worth mentioning is the SIMD intrinsics are all labelled unsafe, using intrinsics is a bit like calling FFI functions, they aren't playing by Rust's rules and are thus unsafe. Optimised code often sacrifices a bit of safety and readability for speed, this will be no exception.

# Converting Vec3 to SIMD

Most of the advice I've heard around using SIMD is just making your math types use it not the way to get performance and that you are better to just write SIMD math code without wrappers. One reason is `Vec3` is only using 3 of the available SIMD lanes, so even on SSE you're only 75% occupancy and you won't get any benefit from larger registers. Another reason is components in a `Vec3` are related but values in SIMD vector lanes have no semantic relationship. In practice what this means is doing operations across lanes like a dot product is cumbersome and not that efficient. See "The Awkward Vec4 Dot Product" slide on page 43 of this [GDC presentation](https://deplinenoise.files.wordpress.com/2015/03/gdc2015_afredriksson_simd.pdf).

Given all of the above it's not surprising that Aras [blogged](http://aras-p.info/blog/2018/04/10/Daily-Pathtracer-Part-7-Initial-SIMD/) that he didn't see much of a gain from converting his `float3` struct to SIMD. It's actually one of the last things I implemented to try and reach my performance target. I followed the same post he did on ["How to write a maths library in 2016"](http://www.codersnotes.com/notes/maths-lib-2016/) except for course in Rust rather than C++. This actually gave me a pretty big boost, from 10Mrays/s to 20.7Mrays/s. That's a large gain so why did I see this when Aras's C++ version only saw a slight change?

I think the answer has to do with my next topic.

# Floating point in Rust

I think every game I've ever worked on has enabled to the fast math compiler setting, `-ffast-math` on GCC/Clang or `/fp:fast` on MSVC. This setting sacrifices IEEE 754 conformance to allow the compiler to optimise floating point operations more aggressively. Agner Fog recently released a paper on [NaN propagation](http://www.agner.org/optimize/nan_propagation.pdf) which talks about the specifics of what `-ffast-math` actually enables. There is currently no equivalent of `-ffast-math` for the Rust compiler and that doesn't look like something that will change any time soon. There's a bit of discussion about how Rust might support it, there are currently [intrinsics](https://doc.rust-lang.org/core/intrinsics/index.html) for fast float operations which you can use in nightly but no plans for stable Rust. I found a bit of discussion on the Rust internals forums like [this post](https://internals.rust-lang.org/t/avoiding-partialord-problems-by-introducing-fast-finite-floating-point-types) but it seems like early days.

My theory on why I saw a large jump going from precise IEEE 754 floats to SSE was in part due to the lack of `-ffast-math` optimisations. This is speculation on my part though, I haven't done any digging to back it up.

A more well known floating point wart in Rust is the distinction between `PartialOrd` and `Ord` traits. This distinction exists because floating point can be `Nan` which means that it doesn't support [total order](https://en.wikipedia.org/wiki/Total_order) which is required by `Ord` but not `PartialOrd`. As a consequence you can't use a lot of standard library functions like [sort](https://doc.rust-lang.org/std/primitive.slice.html#method.sort) with floats. This is an ergonomics issue rather than a performance issue but it is one I ran into working on this so I thought I'd mention it.

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

Reasonably straight forward, the first five lines are simple math operations before some conditionals to determine if the ray hit or not. To convert this to SSE2 first we need to load the data into the SIMD registers, do the math bit and finally extract which one of the SIMD lanes contains the result we're after (the nearest hit point).

First we need to splat x, y, and z components of the ray origin and direction into SSE registers. They we need to iterate over 4 spheres at a time, loading 4 x, y and z components of the sphere's centre points into registers containing 4 x components, 4 y components and so on.

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

# Structure of Arrays

There are 3 ways that I'm aware of to load the sphere data into SSE vectors:

* `_mm_set_ps` which takes 4 `f32` parameters
* `_mm_loadu_ps` which takes an unaligned `*const f32` pointer
* `_mm_load_ps` which takes a 16 byte aligned `*const f32` pointer

I did a little experiment to see what the assembly generated by these might look like ([playground](https://play.rust-lang.org/?gist=d4dcd07c3fea0139e6617cf6b83ba634&version=nightly&mode=release)):

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

The first option has to do a lot of work to get data into registers before it can perform the `addps`. The last option appears to be doing the least work based on instruction count, the `addps` can take one of it's operands as a memory address because it's aligned correctly. 

My initial implementation was an array of `Sphere` objects. The `x` values for 4 spheres are not in contiguous memory, so I would be forced to use `_mm_set_ps` to load the data.  To use the `_mm_loadu_ps` I need to store my `Sphere` data in Structure of Arrays (SoA) format. SoA means instead an array of individual `Sphere` objects I have a struct containing arrays for each `Sphere` data member. My `SpheresSoA` struct looks like so:

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

There are other advantages to SoA. In the above structure I'm storing precalculated values for `radius * radius` and `1.0 / radius` as if you look at the original `hit` function `radius` on it's own is never actually used. Before switching to SoA adding precalculated values to my `Sphere` struct actually made things slower, presumably because I'd increased the size of the `Sphere` struct, reducing my CPU cache utilisation. With `SpheresSoA` if the `determinant > 0.0` check doesn't pass then the `radius_inv` data will never be accessed, so it's not wastefully pulled into CPU cache. Switching to SoA did speed up my scalar code too although I didn't note down the performance numbers at the time.

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

# Getting the result out again

I won't paste the full SSE2 (I lie, I'm using a little SSE4.1) ray spheres intersection test source here because it's quite verbose, but it can be found [here](https://github.com/bitshifter/pathtrace-rs/blob/spheres_simd/src/collision.rs#L173) if you're interested. I mostly wrote this from scratch using the [Intel Intrinsics Guide](https://software.intel.com/sites/landingpage/IntrinsicsGuide/#) to match my scalar code to the SSE2 equivalent and using Aras's [post on SSE sphere collision](http://aras-p.info/blog/2018/04/11/Daily-Pathtracer-8-SSE-HitSpheres/) and subsequent post on [ryg's optimisations](http://aras-p.info/blog/2018/04/13/Daily-Pathtracer-9-A-wild-ryg-appears/) as a guide. I think I had got pretty close to completion at the end of one evening but wasn't sure how to extract the minimum hit result out of the SSE vector so just went for the scalar option:

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

All the above code is doing is finding the minimum value in the `hit_t` SIMD lanes to see if we have a valid hit. Aras's code what able to do this in SSE2 with his `h_min` function (h standing for horizontal meaning across the SIMD register lanes), but I wanted to understand how it was working before I used it. The main reason I didn't understand `h_min` is I'd never used a shuffle before. I guess they're familiar to people who write a lot of shaders since swizzling is pretty common but I haven't done a lot of that either.

My Rust `hmin` looks like:

```rust
v = _mm_min_ps(v, _mm_shuffle_ps(v, v, _mm_shuffle!(0, 0, 3, 2)));
v = _mm_min_ps(v, _mm_shuffle_ps(v, v, _mm_shuffle!(0, 0, 0, 1)));
_mm_cvtss_f32(v)
```

What this is doing is combining `_mm_shuffle_ps` and `_mm_min_ps` to compare all of the lanes with each other and shuffle the minimum value into lane 0 (the lowest 32 bits of the vector).

The shuffle is using a macro that encodes where lanes are being shuffled from, it's a Rust implementation of the `_MM_SHUFFLE` macro found in `xmmintrin.h`:

```rust
macro_rules! _mm_shuffle {
    ($z:expr, $y:expr, $x:expr, $w:expr) => {
        ($z << 6) | ($y << 4) | ($x << 2) | $w
    };
}
```

So for example to reverse the order of the SIMD lanes you would do:

```rust
_mm_shuffle_ps(a, a, _mm_shuffle!(0, 1, 2, 3))
```

Which is saying move lane 0 to lane 3, lane 1 to lane 2, lane 2 to lane 1 and lane 3 to lane 0. Lane 3 is the left most lane and lane 0 is the rightmost, which seems backwards but I assume it's due to x86 being little endian. As you can see `_mm_shuffle_ps` takes two operands, so you can interleave lanes from two values into the result but I haven't found a use for that so far.

So given that, if we put some values into `hmin`:

```rust
let a = _mm_loadu_ps([A, B, C, D]);
// a = [A, B, C, D]
let b = _mm_shuffle_ps(a, a, _mm_shuffle!(0, 0, 3, 2));
// b = [D, D, A, B];
let c = mm_min_ps(a, b);
// c = [min(A, D), min(B, D), min(C, A), min(D, B)];
let d = _mm_shuffle_ps(c, c, _mm_shuffle!(0, 0, 0, 1));
// d = [min(A, D), min(A, D), min(A, D), min(C, A)];
let e = _mm_min_ps(c, d);
// e = [min(A, D), min(A, B, D), min(A, C, D), min(A, B, C, D)]
let f = _mm_cvtss_f32(e);
// f = min(A, B, C, D)
```

The result in lane 0 has been compared with all of the values in the 4 lanes. I thought this was a nice example of the kind of problems that need to be solved when working with SIMD.

# Alignment

As I mentioned earlier that the best way that I'm aware of to load data into SSE registers is with a `f32` array aligned to 16 bytes so you can use the `_mm_load_ps` intrinsic. Unfortunately Rust's `std::Vec` doesn't support custom allocators so there's no way to specify a specific alignment for my `Vec<f32>`.

Another way would be to store an aligned arrays of floats that are the same size as our SIMD vector. I did this with a `[repr(align(16)]` wrapper around a float array which looks like:

```rust
pub mod m128 {
    #[repr(C, align(16))]
    pub struct ArrayF32xN(pub [f32; 4]);
}

pub mod m256 {
    #[repr(C, align(32))]
    pub struct ArrayF32xN(pub [f32; 8]);
}
```

I actually just wanted to use `[repr(align(n))]` because I implemented it in the compiler and I'd never had an excuse to use it. Probably it would be easier to make a union of an `f32` array and the SIMD vector type.

In any case my `SpheresSoA` now looks like:

```rust
pub struct SpheresSoA {
    centre_x: Vec<ArrayF32xN>,
    centre_y: Vec<ArrayF32xN>,
    centre_z: Vec<ArrayF32xN>,
    radius_sq: Vec<ArrayF32xN>,
    radius_inv: Vec<ArrayF32xN>,
    num_chunks: u32,
    num_spheres: u32,
}
```

So I can remove the `chunks(4)` from my ray spheres intersection loop as they spheres are already in `ArrayF32xN` sized chunks.

# SIMD wrapper and AVX2

I had intended to write a wrapper later after getting some hands on experience with SSE intrinsics. One reason being that 

```rust
let discr = nb * nb - c;
```

is a lot easier to read and write than

```rust
let discr = _mm_sub_ps(_mm_mul_ps(nb, nb), c);
```

The other reason was I had noticed that mostly the SSE code for the ray spheres collision wasn't specific to the number of lanes. So I thought if I wrote a wrapper then it wouldn't be that difficult to add AVX support which has 8 vectors in a float but still have exactly the same collision code.

The way I implemented the wrapper was inside different modules for the 128 bit, 256 bit and 32 bit scalar fallback versions. Then I just use the one that is available for target features enabled by the compiler. This means I'm using compile time target feature selection but not runtime target feature selection. I haven't worked out exactly how I was to implement the runtime version yet.

In a very abridged version for example I have the following modules:

```rust
pub mod simd {
  #[cfg(target_feature = "sse2")]
  pub mod m128 {
    pub struct f32xN(pub __m128);
  }

  #[cfg(target_feature = "avx2")]
  pub mod m256 {
    pub struct f32xN(pub __m256);
  }

  pub mod m32 {
    pub struct f32xN(pub f32);
  }

  // re-export fallback scalar code
  #[cfg(not(any(target_feature = "sse2", target_feature = "avx2")))]
  pub use self::m32::*;

  // re-export sse2 if no avx2
  #[cfg(all(target_feature = "sse2", not(target_feature = "avx2")))]
  pub use self::m128::*;

  // re-export avx2
  #[cfg(target_feature = "avx2")]
  pub use self::m256::*;
}
```

So user code can `use simd::f32xN` and it will use the widest SIMD registers available to the compiler. I have a bunch of other types I'm creating like `ArrayF32xN` which I use to align data for loading into `f32xN`, there's `i32xN` for integer SIMD and `b32xN` for boolean values. I haven't wrapped every possible instruction, just what I was using for ray sphere intersections. I'm not using traits for these to define a common interface for the different bit sizes although I possibly should.

My wrapper code can be found [here](https://github.com/bitshifter/pathtrace-rs/blob/spheres_simd_wrapped/src/simd.rs) and the ray sphere intersection test using it is [here](https://github.com/bitshifter/pathtrace-rs/blob/spheres_simd_wrapped/src/collision.rs#L157).

For x86_64 targets `rustc` enables SSE2 by default. There are a couple of different ways to enable other target features, one is `RUSTFLAGS` and the other is `.cargo/config`. The latter was most useful for me as it meant I didn't need to either set flags globally or remember to set them on every invocation of the compiler. My `.cargo/config` to enable AVX2 looks like this:

```yaml
[target.'cfg(any(target_arch = "x86", target_arch = "x86_64"))']
rustflags = ["-C", "target-feature=+avx2"]
```

# Final performance results

I primarily testing performance on my laptop running Window 10 home. My performance numbers for each iteration of my path tracer are:

| Feature                   | Mrays/s |
| ------------------------- | -------:|
| Aras's C++ SSE4.1 version |    45.5 |
| My starting point         |    10.0 |
| SSE2 Vec3                 |    20.7 |
| Plain SSE4.1              |    38.1 |
| Wrapped SSE4.1            |    38.7 |
| Wrapped AVX2              |    41.9 |
| Wrapped SSE4.1 w/ LTO     |    41.2 |
| Wrapped AVX2 w/ LTO       |    45.0 |

The difference between the `Plain SSE4.1` and `Wrapped SSE4.1` are because I added the `hmin` changes and aligned `SpheresSoA` data at the same time as I added the SIMD wrappers. LTO stands for link time optimisation. It did give a nice increase in performance, but at a large increase in build time.

I do most of my development on Linux and was confused for a long time because my Linux performance was much higher than what I was seeing in Windows on the same laptop. At the same time the Linux peaks were higher but it fluctuated a lot as the path tracer ran. The Windows numbers were lower but stable. This difference is very likely due to CPU frequency scaling differences between Linux and Windows. It does make benchmarking on a Linux laptop a bit unreliable though.

I did occasionally test on a desktop machine too, which gave the following results:

| Feature                   | Mrays/s |
| ------------------------- | -------:|
| Aras's C++ SSE4.1 version |    47.7 |
| Wrapped SSE4.1            |    43.0 |
| Wrapped AVX2              |    47.0 |
| Wrapped SSE4.1 w/ LTO     |    45.8 |
| Wrapped AVX2 w/ LTO       |    51.0 |

So in this case my AVX2 version was faster, which is nice but not really an apples versus apples comparison.

So what differences are remaining? A couple of things stood out in my profiling:

The C++ compiler appeared to be linking in an SSE2 implementation of sin and cos, Rust is using the f64 sin and cos in the MSVC toolchain version of the compiler

Rayon was higher overhead than enki - the C thread pool Aras was using. There were some pretty deep callstacks in Rayon and the hot function that was showing up in VTune was pretty large. Enki by comparison is pretty simple.

Addressing these things might get me a bit closer.

If you made it this far well done, thanks for reading. I promise my next post will be shorter!
