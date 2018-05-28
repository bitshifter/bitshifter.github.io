---
layout: post
title:  "Optimising path tracing with SIMD"
categories: blog
---

Motivations - try and match C++ performance and get some experience writing simd.

Benchmarking against Aras' optimised C++ path tracer. Copied his scene layout and his light emission and shadow ray changes so I have something to compare against. Also copied his render window so it's easier to see what's going on. Using glium cos it was easy to drop in.

Performance tl;dr: Is my Rust path tracer as fast as Aras' C++ one. If using SSE4.1 then almost. Using AVX2 it's faster but that's not a fair comparison. There's a couple of reasons why I think it's slower which I'll cover later.

# What is simd?

Instructions on CPU for number crunching. SSE2 has been on Intel for a long time. Can process 4 floats or 2 doubles at once. 

TODO: Example of add

# SIMD in Rust

FCP

Exposes Intel intrinsics to start. Look pretty much the same as C++.
Supports checking what SIMD features are available (e.g. SSE2, AVX2) at compile time and at runtime.

TODO: examples

# Switching Vec3 to SIMD

Most of the advice I've read around this is it's not the way to get performance out of SIMD, that you are better to just write SIMD math code. One big reason is a Vector3 is only using 3 of the available SIMD lanes, but even so I still saw a performance boost changing my Vec3 to use SIMD. TODO article link.

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
