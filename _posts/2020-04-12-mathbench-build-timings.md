---
layout: post
title: Measuring build timings with mathbench
excerpt_separator: <!--more-->
tags: rust
---

Fast iteration times are something that many game developers consider to be of
utmost importance. Keeping build times short is a major component of quick
iteration for a programmer. Aside from the actual time spent compiling, any time
you have to wait long enough that you start to lose focus on the activity you
are working on, or you start to get distracted or lose track of what you were
doing which costs you more time.

Thus one of my goals when writing [`glam`] was to ensure it was fast to compile.
Rust compile times are known to be a bit slow compared to many other languages,
and I didn't want to pour fuel on to that particular fire.

As part of writing `glam` I also wrote [`mathbench`] so I could compare
performance with similar libraries. I also always wanted to include build time
comparisons as part of `mathbench` and I've finally got around to doing that
with a new tool called [`buildbench`].

<!--more-->

# Introducing build bench

`buildbench` uses the unstable [`-Z timings`] feature of `cargo`, thus you
need a nightly build installed to use it.

`buildbench` generates a `Cargo.toml` and empty `src/lib.rs` in a temporary
directory for each bench crate, recording some build timing information from
`cargo` which is included in the summary table below. The temporary directory is
created every time the tool is run so this is a full build from a clean state.

Each bench is only built once so you may wish to run `buildbench` multiple
times to ensure results are consistent.

By default crates are built using the `release` profile with default features
enabled. There are options for building the `dev` profile or without default
features, see `buildbench --help` for more information.

The columns outputted include the total build time, the self build time which is
the time it took to build the crate excluding dependencies, and the number of
units which is the number of dependencies (this will be 2 at minimum).

When comparing build times keep in mind that each library has different feature
sets and that naturally larger libraries will take longer to build. For many
crates tested the dependencies take longer than the main crate. Also keep in
mind if you are already using one of the dependencies in your project you won't
pay the build cost twice (unless it's a different version).

| crate               | version | total (s) | self (s) | units | full report link           |
|:--------------------|:--------|----------:|---------:|------:|:---------------------------|
| cgmath              | 0.17.0  |       9.5 |      3.0 |    17 | [cgmath build timings]     |
| euclid              | 0.20.5  |       3.2 |      1.1 |     4 | [euclid build timings]     |
| glam                | 0.8.6   |       0.8 |      0.5 |     3 | [glam build timings]       |
| nalgebra            | 0.21.0  |      32.1 |     17.9 |    29 | [nalgebra build timings]   |
| pathfinder_geometry | 0.4.0   |       2.9 |      0.3 |     8 | [pathfinder build timings] |
| vek                 | 0.10.1  |      38.3 |     10.8 |    16 | [vek build timings]        |

These benchmarks were performed on an [Intel i7-4710HQ] CPU with 16GB RAM and a
Toshiba MQ01ABD100 HDD (SATA 3Gbps 5400RPM) on Linux.

# Considering the results

It seems I achieved my goal of making `glam` fast to build! As `glam` grows and
gets more features build times will of course increase, but a few seconds is the
ballpark I'm hoping to stay in.

# Non-optional features

As I mentioned before one big difference between all of these crates is their
code size. Most are oriented towards game development with the exception of
[`nalgebra`] which has much broader design goals and supports many more features
than `glam`. `glam` is about 8.5K lines of Rust, `nalgebra` is more like 40K.

Feature wise `glam` is much closer to [`cgmath`] but without generics.  Aside
from the use of generics one big difference between `glam` and `cgmath` and
indeed most of the other crates is what dependencies are included by default.
The most recent release of `cgmath` on crates.io has non-optional dependencies
on `approx`, `num-traits` and `rand`. I noticed on `master` that `rand` is now
optional, but there hasn't been a release made in a while.

# Default features

In addition to making most `glam` dependencies optional I also made all optional
features opt-in rather than opt-out. That is `glam` optional dependencies are
not enabled by default. That is part of why `glam`'s build time is lower than
the others, by default its only dependency is `cfg-if`. `glam` does has have
support for `serde`, `mint` and `rand` but you have to enable those features if
you want to use them.

[`vek`] stands out as taking a significant amount of time compared to the others.
One thing to note though is the self time at 10.7 seconds is a lot less than
17.9 seconds of `nalgebra`. A large portion of the time building `vek` is
dependencies, mostly `serde` and `serde_derive`.

# No default features surprise

You could argue that for a fair comparison I should be building all these crates
with `default-features = false`. That is true and `buildbench` does support
this, but unfortunately with some crates one does not simply disable default
features. Many libraries that support [no_std] do so by making a `std` feature
which is included in the `default` feature list. Thus `no_std` can be supported
by disabling default features on a dependency using `default-features = false`.
Because of this building with `default-features = false` can give surprising
results.

Early versions of `mathbench` disabled default features for all the math
libraries in an attempt to improve build times. I encountered a few issues
taking this approach and eventually went back to using default features.

One problem was `nalgebra` was getting really poor results for some benchmarks
in a way that didn't make much sense. The dot product benchmark was similar to
other libraries but vector length was really slow, the only difference between
those is a square root. Fortunately some members of the Rust Community Discord
prompted the author of `nalgebra` to investigate the issue and submitted a PR.
The problem was that disabling defaults effectively put `nalgebra` in no_std
mode. In this mode it changed the way math libraries were linked which meant
calls to functions like `sqrt` or `sin` were no longer inlined, which has a big
performance impact. I also had an issue raised to benchmark with default
features enabled as that's what most people will use so between these two things
I started building all libraries with default features enabled.

The `nalgebra` no_std issue was quite surprising to me. I hadn't realised the
link to `no_std` and  If I wasn't writing benchmarks I don't think I would have
noticed that something strange was going on. This is mentioned in the `nalgebra`
documentation under [web assembly and embedded programming] but I think most
people aren't going to go looking for that when disabling default features.

`vek` also has some surprising issues around disabling default features and
support for no_std. `vek` has the largest total build time of all the crates
tested, but it's self time is only 25% of the total build time and building
dependences are the other 75% or 30 seconds on my laptop. Looking at [vek
build timings] the `serde` and `serde_derive` crates are a large chunk of
that 30 seconds, According to crates.io the in the 0.10.1 release that I was
using, `serde` is an optional dependency. OK, so I'll build `vek` with
`default-features = false`, but this is intended for use with no_std so if you
just disable default features `vek` doesn't build at all. To build with no_std
you need to manually add the `libm` feature, which I assume will link the
necessary math routines. I imagine that this will have the same performance
implications that it did for `nalgebra`. On the bright side this bought the
total build time down to 7.54 seconds!. If you are still building for std though
things get a bit trickier, you need to manually add the `num-traits/std`
feature. Doing this I was able to build in 6.93 seconds.  Initially I tried
disabling default features and enabling std again but this adds the `serde/std`
feature, pulling in the `serde` and `serde_derive` crates which were supposed to
be optional.

I'm glad it is possible but it's really not obvious how to build with default
features disabled.

This isn't really a criticism of `nalgebra` or `vek`. I feel that this confusion
has arisen due to the convention of using `default-features = false` to build
for no_std. If all you want to do is reduce unused feature dependencies
`default-features = false` should be the right lever to push but in reality due
to this being conflated with building for no_std it's often not that simple.
While I found a work around for `vek` I am not sure how they could make things
simpler for their users out of the box. Part of the issue is `vek` also tells
dependencies like `serde` to build with `default-features = false` and those
crates also treat this as building for no_std. This is why the std feature of
`vek` adds `serde/std` but that appears to have the effect force enabling the
optional `serde` feature when building for std. I don't know if there is a good
way around that. As far as I know it's not possible to conditionally enable
`serde/std` if both std and `serde` are enabled. I did try removing `serde/std`
from the `std` feature of `vek` and it compiled, so perhaps it isn't necessary.
Removing it certainly makes the default build a lot faster.

One criticism I do have of many crates is this kind of behaviour is not well
documented. Both `nalgebra` and `vek` do document how to build for no_std but
if you just want to disable optional features in a std build you seem to be on
your own.

# No default features

With all of that out of the way, I think that making the optional features of
`glam` not default features was a good choice. As I've discussed above turning
off default features has not always been an easy thing to do. I
think having them off by default and documenting how they can be enabled might
be a better approach for the majority of users.

# Fair benchmarks

While providing benchmarks for the default features is the most useful benchmark
it doesn't always represent the minimal build time for many crates.
Unfortunately it seems I will need to tune the minimal set of features for each
crate to achieve this. That's something I was trying to avoid doing but I think
in fairness I will support it in a future version of `buildbench`.  Equally
adding support for building all dependencies might also be informative.

# What isn't being measured

[Maik Klein] pointed out that one thing I am not measuring is the cost of
generics in user code. When using generics a lot of the cost is shifted into the
crate that is instantiating them. That is not something that `buildbench` is
attempting to measure at the moment.

# In conclusion

As usual trying to compare libraries on some metric turned out to be not that
simple.

Turning off default features is harder than it should be. This setting is often
an alias for `#![no_std]` support. I think that's unfortunate and perhaps there
should be more explicit flags added to `cargo` to build for no_std.

Ultimately the point of this exercise was to provide another metric to consider
when choosing a math library. Are you paying for features you aren't using?

[`glam`]: https://crates.io/crates/glam
[`mathbench`]: https://github.com/bitshifter/mathbench-rs
[`buildbench`]: https://github.com/bitshifter/mathbench-rs/tree/master/tools/buildbench
[`cgmath`]: https://crates.io/crates/cgmath
[`nalgebra`]: https://nalgebra.org
[`vek`]: https://crates.io/crates/vek
[`euclid`]: https://crates.io/crates/euclid
[`pathfinder_geometry]: https://crates.io/crates/pathfinder_geometry
[`-Z timings`]: https://internals.rust-lang.org/t/exploring-crate-graph-build-times-with-cargo-build-ztimings/10975
[Intel i7-4710HQ]: https://ark.intel.com/content/www/us/en/ark/products/78930/intel-core-i7-4710hq-processor-6m-cache-up-to-3-50-ghz.html
[cgmath build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-cgmath-release-defaults.html
[euclid build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-euclid-release-defaults.html
[glam build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-glam-release-defaults.html
[nalgebra build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-nalgebra-release-defaults.html
[pathfinder build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-pathfinder_geometry-release-defaults.html
[vek build timings]: https://bitshifter.github.io/buildbench/0.3.1/cargo-timing-vek-release-defaults.html
[no_std]: https://rust-embedded.github.io/book/intro/no-std.html
[web assembly and embedded programming]: https://nalgebra.org/wasm_and_embedded_programming/
[Maik Klein]: https://github.com/maikKlein
