---
layout: post
title:  "Optimising path tracing with SIMD"
categories: blog
---

Following on from [path tracing in parallel with Rayon]({{ site.baseurl }}{% post_url 2018-05-07-path-tracing-in-parallel %}) I had a lot of other optimisations I wanted to try. In particular I want to see if I could match the CPU performance of [@aras_p](https://twitter.com/aras_p)'s [C++ path tracer](https://github.com/aras-p/ToyPathTracer) in Rust. He'd done a fair amount of optimising so it seemed like a good target to aim for. To get a better comparison I copied his scene and also added his light sampling approach which he talks about [here](http://aras-p.info/blog/2018/03/28/Daily-Pathtracer-Part-1-Initial-C--/). I also implemented a live render loop mimicing his.

![the final result](/public/img/output_lit.png)

My initial unoptimized code was processing 10Mrays/s on my laptop. Aras' code (with GPGPU disabled) was doing 45.5Mrays/s. I had a long way to go from here! My unoptimized code can be found on [this branch](https://github.com/bitshifter/pathtrace-rs/tree/emissive).

tl;dr did I match the C++ in Rust? Almost. My SSE4.1 version is doing 41.2Mrays/s about 10% slower than the target 45.5Mrays/s running on Windows on my laptop. The long answer is more complicated but I will go into that later. My fully (so far) optimised version lives [here](https://github.com/bitshifter/pathtrace-rs/tree/spheres_simd_wrapped).

# SIMD

A big motivation for me doing this was to write some SIMD code. I knew what SIMD was but I'd never actually written any, or none of significance at least.

If you are unfamiliar with SIMD, it stands for Single Instruction Multiple Data. What this means is there are registers on most common CPUs which contain multiple values, e.g. 4 floats, which can be processed with a single instruction, increasing the througput of math heavy code. On Intel SSE2 has been avaiable since 2001, providing 128 bit registers which can hold 4 floats or 2 doubles. More recent chips have AVX which added 256 bit registers and AVX512 which added 512 bit registers.

![SIMD add](https://mirrors.edge.kernel.org/pub/linux/kernel/people/geoff/cell/ps3-linux-docs/CellProgrammingTutorial/CellProgrammingTutorial.files/image009.jpg)

In ths path tracer we're performing collision detection of a ray against an array of spheres, one sphere at a time. With SIMD we could check 4 or even 8 spheres at a time. That sounds like a good performance boost!

# SIMD in Rust

Rust SIMD support is available in the nightly compiler and should make it into a stable Rust release soon. There are a couple of related RFCs around SIMD support.

* Target feature [RFC#2045](https://github.com/rust-lang/rfcs/blob/master/text/2045-target-feature.md) adds the ability to:
  * determine which features (CPU instructions) are available at compile time
  * determine which features are available at run-time
  * embed code for different sets of features in the same binary

* Stable SIMD [RFC#2325](https://github.com/rust-lang/rfcs/blob/master/text/2325-stable-simd.md) adds support for:
  * 128 and 256 bit vector types and intrinsics for Intel chipsets, e.g. SSE and AVX - other instruction sets will be added later
  * the ability to branch on target feature type at runtime so you can use a feature if it's available on the CPU the program is running on

One thing that is not currently being stabilised yet is cross platform abstraction around different architecture's SIMD instructions.

# Converting Vec3 to SIMD

Most of the advice I've heard around using SIMD is just making your math types use it not the way to get performance and that you are better to just write SIMD math code without wrappers. One reason is `Vec3` is only using 3 of the available SIMD lanes, so even on SSE you're only 75% occupancy and you won't get any benefit from larger registers. Another reason is components in a Vec3 are related but values in SIMD vector lanes have no semantic relationship. In practice what this means is doing operations across lanes like a dot product is cumbersome and not that efficient. See "The Awkward Vec4 Dot Product" slide on page 43 of this [GDC presentation](https://deplinenoise.files.wordpress.com/2015/03/gdc2015_afredriksson_simd.pdf).

Given all of the above it's not surprising that Aras [blogged](http://aras-p.info/blog/2018/04/10/Daily-Pathtracer-Part-7-Initial-SIMD/) that he didn't see much of a gain from converting his `float3` struct to SIMD. I followed the same post he did on ["How to write a maths library in 2016"](http://www.codersnotes.com/notes/maths-lib-2016/) except for course in Rust rather than C++. This actually gave me a pretty big boost, from 10Mrays/s to 20.7Mrays/s. That's a large gain so why did I see this when Ara's C++ version only saw a slight change?

I think the answer has to do with my next topic.

# Floating point in Rust

I think every game I've ever worked on has enabled to the fast math compiler setting, `-ffast-math` on GCC/Clang or `/fp:fast` on MSVC. This setting sacrifices IEEE 754 conformance to allow the compiler to optimise floating point operations more aggressively. Agner Fog recently released a paper on [NaN propogation](http://www.agner.org/optimize/nan_propagation.pdf) which talks about the specifics of what `-ffast-math` actually enables. There is currently no equivalent of `-ffast-math` for the Rust compiler and that doesn't look like something that will change any time soon. There's a bit of discussion about how Rust might support it, there are currently [intrinsics](https://doc.rust-lang.org/core/intrinsics/index.html) for fast float operations which you can use in nightly but no plans for stable Rust. I found a bit of discussion on the Rust internals forums like [this post](https://internals.rust-lang.org/t/avoiding-partialord-problems-by-introducing-fast-finite-floating-point-types) but it seems like early days.

My theory on why I saw a large jump going from precise IEEE 754 floats to SSE was in part due to the lack of `-ffast-math` optimisations. This is speculation on my part though, I haven't done any digging to back it up.

A more well known floating point wart in Rust is the distinction between `PartialOrd` and `Ord` traist. This distinction exists because floating point can be `Nan` which means that it doesn't support [total order](https://en.wikipedia.org/wiki/Total_order) which is required by `Ord` but not `PartialOrd`. As a consequence you can't use a lot of standard library functions like [sort](https://doc.rust-lang.org/std/primitive.slice.html#method.sort) with floats. This is an ergonomics issue rather than a performance issue but it is one I ran into working on this so I thought I'd mention it.

# Structure of arrays

SSE2 has 4 float lanes, this means we can convert our hitspheres from testing against 1 sphere at a time to 4 spheres at a time. This means we need to load 4 x, y and z positions into 3 SSE2 vectors. Switching to SoA makes this a lot easier, especially if we implement AVX later. It also means we can precalculate things like radius_sq without worrying about bloating the sphere struct size.

TODO: example of SoA

Switching to SoA should provide a perf benefit on it's own, especially since we can precalc fields, but it also allows us to more easily load data into SIMD registers. 

# Getting the result out again

Doing math is easy, getting which lane contains our result is where things get a big tricky.

TODO: naive example

TODO: ryg's hmin

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
