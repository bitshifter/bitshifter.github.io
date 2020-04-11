---
layout: post
title: Measuring build timings with mathbench
excerpt_separator: <!--more-->
tags: rust
---

Iteration time is something that a lot of game developers consider to be of
utmost importance. Keeping build times short is an important component of fast
iteration for a programmer. Aside from the actual time spent compiling, any time
you have to wait long enough that you start to lose focus on the activity you
are working on you can start to get distracted or lose track of what you were
doing which costs you more time.

Thus one of my goals when writing `glam` was to ensure it was fast to compile.
Rust compile times are known to be a bit slow compared to many other languages,
and I didn't want to pour fuel on to that particular fire.

I also always wanted to perform build time comparisons as part of `mathbench`
and I've finally got around to doing that with a new tool in `mathbench` called
`buildbench`.

# Introducing build bench

`buildbench` tool uses the `cargo -Z timings` feature of the nightly build of
`cargo`, thus you need a nightly build to run it. See [cargo -Z timings] for
more details on this feature.

`buildbench` generates a `Cargo.toml` and empty `src/lib.rs` in a temporary
directory for each library, recording some build time information which is
included in the summary table below. The temporary directory is created every
time the tool is run so this is a full build from a clean state.

Each library is only built once so you may wish to run `buildbench` multiple
times to ensure results are consistent.

By default crates are built using the `release` profile with default features
enabled. There are options for building the `dev` profile or without default
features, see `buildbench --help` for more information.

The columns outputted include the total build time, the self build time which is
the time it took to build the crate on it's own excluding dependencies, and the
number of units which is the number of dependencies (this will be 2 at minimum).

When comparing build times keep in mind that each library has different feature
sets and that naturally larger libraries will take longer to build. For many
crates tested the dependencies take longer than the math crate. Also keep in
mind if you are already building one of the dependencies in your project you
won't pay the build cost twice (unless it's a different version).

| crate               | total (s) | self (s) | units | report                     |
|:--------------------|----------:|---------:|------:|:---------------------------|
| cgmath              |       7.0 |      2.9 |    17 | [cgmath build timings]     |
| euclid              |       3.2 |      1.1 |     4 | [euclid build timings]     |
| glam                |       0.8 |      0.5 |     3 | [glam build timings]       |
| nalgebra            |      22.9 |     16.5 |    22 | [nalgebra build timings]   |
| pathfinder_geometry |       2.7 |      0.3 |     8 | [pathfinder build timings] |
| vek                 |      37.9 |     10.7 |    16 | [vek build timings]        |

These benchmarks were performed on an [Intel i7-4710HQ] CPU with 16GB RAM and a
Toshiba MQ01ABD100 HDD (SATA 3Gbps 5400RPM) on Linux.

# Considering the results

It seems I achieved my goal of making `glam` fast to build! As `glam` grows and
gets more features build times will of course increase, but a few seconds is the
ballpark I'm hoping to stay in.

As I mentioned before one big difference between all of these crates is their
features. Most are oriented towards game development with the exception of
`nalgebra` which has much broader design goals and supports many more features
than `glam`. `glam` is about 8.5K lines of Rust, `nalgebra` is more like 40K.
Feature wise `glam` is much closer to `cgmath` but without `generics`.
Aside from the use of generics one big different between `glam` and `cgmath` and
indeed most of the other crates is what is included by default.

# Default dependencies

One decision I made with `glam` was to make all features opt-in rather than opt
out. That is a large part of why `glam`'s build time is lower than the others,
by default it doesn't pull any other crates in. `glam` does has have support for
`serde`, `mint` and `rand`, but you have to enable those featues if you want to
use them. `cgmath` for example has `mint` and `serde` as optional dependencies
but `rand`, `approx` and `num-traits` are not. `num-traits` is probably pretty
essential to `cgmath`, it's commonly used by generic math libraries,  but I'm
not so sure about `rand`.

`vek` stands out as taking a significant amount of time compared to the others.
One thing to note though is the self time at 10.7 seconds is a lot less than
16.5 seconds of `nalgebra`, a large portion of the time building `vek` is
dependencies, mostly `serde` and `serde_derive`.

# Building with no default dependencies

In fairness I should be building all these crates with `default-features =
false` you might argue. That is true, but unfortunately it's not that simple. A
lot of libraries that support `no_std` do so by making `std` support a default
feature which is then disabled with `default-features = false`. This complicates
things.

# In conclusion

Something to consider when choosing a library though is are you paying for the
features you aren't using?


[cargo build timings]: https://internals.rust-lang.org/t/exploring-crate-graph-build-times-with-cargo-build-ztimings/10975
[cgmath build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-cgmath-release-defaults.html
[euclid build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-euclid-release-defaults.html
[glam build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-glam-release-defaults.html
[nalgebra build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-nalgebra-release-defaults.html
[pathfinder build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-pathfinder_geometry-release-defaults.html
[vek build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-vek-release-defaults.html
