---
layout: post
title: Yak shaving conditional compilation in Rust
excerpt_separator: <!--more-->
tags: performance profiling
---

In C++ there is a common idiom used when writing a low level interface that has
different implementations for multiple architectures. That is to use the
preprocessor to select the appropriate implementation at compile time. This
pattern is frequently used in C++ math libraries.

<!--more-->

For example, the C++ [Realtime Math] library sets up its own defines based on
definitions passed in from the build system:

[Realtime Math]: https://github.com/nfrechette/rtm

```cpp
#if !defined(RTM_NO_INTRINSICS)
  #if defined(__SSE2__) || defined(_M_IX86) || defined(_M_X64)
    #define RTM_SSE2_INTRINSICS
  #endif
#endif
```

Intrinsics can be entirely disabled if `RTM_NO_INTRINSICS` is defined. Otherwise
the predefined compiler specific definitions (different compilers use different
defines for this stuff, welcome to C/C++) `__SSE2__`, `_M_IX86` and `_M_X64` are
checked, if any of these are defined the Realtime Math library enables `SSE2`
support with it's own `RTM_SSE2_INTRINSICS` define.  These defines are then used
to select the appropriate implementation to compile:

```cpp
inline float RTM_SIMD_CALL vector_get_x(vector4f_arg0 input) RTM_NO_EXCEPT
{
#if defined(RTM_SSE2_INTRINSICS)
  return _mm_cvtss_f32(input);
#elif defined(RTM_NEON_INTRINSICS)
  return vgetq_lane_f32(input, 0);
#else
  return input.x;
#endif
}
```

I don't know if this idiom has a name but it's pretty common, especially in math
libraries which usually have a lot of small performance critical functions.

## From the C preprocessor to Rust

There is nothing quite the same as the C preprocessor in Rust. That is generally
considered a good thing, however when it comes to the above idiom there is
nothing I've found in Rust that feels quite as convenient.

I've tried three different approaches to solving this problem in [glam] which
I'll discuss the pros on cons of here. Perhaps first though, let's talk about
the primary features that Rust provides to solve this problem.

[glam]: https://github.com/bitshifter/glam-rs

## `cfg` attributes

Rust code can be conditionally compiled using the attributes `cfg` and
`cfg_attr` and the built-in `cfg!` macro. These can be used to check conditions
like the target architecture of the compiled crate, enabled features passed to
the compiler by cargo, and so on. Usually for SIMD we're interested in
`target_arch`, `target_feature` and our own crate's `feature` flags.

See the [conditional compilation] section of the Rust book for more details.

[conditional compilation]: https://doc.rust-lang.org/1.9.0/book/conditional-compilation.html

Can we achieve the same thing as the C/C++ example above using the `cfg`
attribute?

```rust
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
// now what?
```

In some ways Rust is simpler in that we only need to check `target_feature =
sse2` instead of multiple vendor specific compiler flags in C or C++, however
unlike `#define` we are unable to introduce a new `feature` in Rust code. These
can only come from the build system. I will come back to this point later one.

Perhaps instead of setting our own define let's just make these `cfg` checks
in our function.

```rust
#[inline]
impl Vec4 {
  fn x(self) -> f32 {
    #[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
    unsafe { _mm_cvtss_f32(self.0) }
    #[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
    unsafe { f32x4_extract_lane(self.0, 0) }
    #[cfg(any(feature = "no-intrinsics", not(any(target_feature = "sse2", target_feature = "wasm32"))))]
    { self.0 }
  }
}
```

The lack of an `else` equivalent means this final fallback `cfg` check is lot
more verbose than the C preprocessor version.  It might be okay for one or two
methods, but not for hundreds. Also there would be a combinatorial explosion
every time a new `target_feature` is supported - the fallback `cfg` check is
going to get even more complex.

If you are familiar with Rust you might be thinking we could use the `cfg!`
macro here, let's consider that:

```rust
#[inline]
impl Vec4 {
  fn x(self) -> f32 {
    if cfg!(all(not(feature = "no-intrinsics"), target_feature = "sse2")) {
      unsafe { _mm_cvtss_f32(self.0) }
    } else if cfg!(all(not(feature = "no-intrinsics"), target_feature = "wasm32")) {
      unsafe { f32x4_extract_lane(self.0, 0) }
    } else {
      self.0
    }
  }
}
```

This looks much better in principle but it doesn't compile. The problem here is
the compiler still tries to parse every conditional block regardless and if you
are on a platform that supports `sse2` then the `wasm32` module won't exist, and
vice versa.

That is the definition of the problem I'm trying to solve, let's look at the
different approaches I've tried with glam.

## Conditional compilation by module

This approach splits the different implementations for each architecture into
it's own module and then selects the appropriate module at compile time. This is
the first approach I took in glam.  Continuing with the above example,
separating into modules looks like so:

```rust
// vec4_sse2.rs
pub struct Vec4(__m128);
impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    unsafe { _mm_cvtss_f32(self.0) }
  }
}
```

```rust
// file: vec4_wasm32.rs
pub struct Vec4(f32x4);
impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    unsafe { f32x4_extract_lane(self.0) }
  }
}
```

```rust
// file: vec4_f32.rs
pub struct Vec4(f32, f32, f32, f32);
impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    self.0
  }
}
```

```rust
// file: lib.rs
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
mod vec4_sse2;
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
mod vec4_wasm32;
#[cfg(any(feature = "no-intrinsics", not(any(target_feature = "sse2", target_feature = "wasm32"))))]
mod vec4_f32;

#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
pub use vec4_sse2::*;
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
pub use vec4_wasm32::*;
#[cfg(any(feature = "no-intrinsics", not(any(target_feature = "sse2", target_feature = "wasm32"))))]
pub use vec4_f32::*;

impl Vec4 {
  // common implementations
}
```

This simple code sample probably looks verbose compared to the earlier example,
however when the module contains 50 or so functions a complete absence of any
`cfg` checks inside each architecture's module implementation is quite pleasant.

The advantages of doing things this way are:

* Keeps different implementations separate, so there's no need for complicated
  `#[cfg(...)]` blocks in every method.
* The implementation for each architecture is normal code and easy to read.

Unfortunately I found a number of downsides to this approach:

* The interface needs to be duplicated for each implementation - this is
  mitigated in glam somewhat by ensuring the interface has near 100% test
  coverage.
* Documentation also needs to be duplicated for each implementation - this is a
  bit more annoying Adding additional architectures will start to make the
  library difficult to maintain due to the above issues

Having to duplicate the interface was annoying and even with a lot of tests
sometimes I still made mistakes. Having to duplicate the docs to keep `rust doc`
happy was really the reason I moved away from this method though.

You can see an example of this approach in [glam 0.8.5].

[glam 0.8.5]: https://github.com/bitshifter/glam-rs/blob/0.8.5/src/f32/vec4_f32.rs

Some improvements to this approach were suggested on reddit, see the addendum
at the end of this post for more details.

## cfg-if

The [cfg-if] crate is probably the de facto solution to this problem in the Rust
ecosystem. The crate provides the `cfg_if` macro which supports similar
functionality to the `if/else if/else` branches of the C preprocessor. Here's
our familiar sample code using `cfg_if`.

[cfg-if]: https://crates.io/crates/cfg-if

```rust
impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    cfg_if! {
      if #[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))] {
        unsafe { _mm_cvtss_f32(self.0) }
      } else if #[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))] {
        unsafe { f32x4_extract_lane(self.0) }
      } else {
        self.0
      }
    }
  }
}
```

This is a pretty good option for mimicking the functionality provided by the C
preprocessor and nicely solves the convoluted `else` block problem you run into
using Rust's `cfg` attribute on it's own, but I ran into a few problems.

Aesthetically, I didn't like having to wrap almost all of the glam code in a
macro. This is a bit of a personal preference but I did run into a couple of
tooling issues also.

[Rustfmt] did not format code blocks inside `cfg_if` macros. I like being able
to write code fast and messy and then running `cargo fmt` to tidy it up when I'm
done, so this was a bit of a hindrance to my workflow. The other tooling problem
was that [tarpaulin], the code coverage tool I am using did not like `cfg_if` at
all it seemed and my test coverage mysteriously dropped from 87% to 73% which
was a bit annoying.

[Rustfmt]: https://rust-lang.github.io/rustfmt/
[tarpaulin]: https://crates.io/crates/cargo-tarpaulin

The tooling issues are nothing to do with `cfg_if` and are most probably bugs
that may well get fixed one day, but until that happens it felt like I'd gone
backwards a bit.

You can see an example of this approach in [glam 0.8.6].

[glam 0.8.6]: https://github.com/bitshifter/glam-rs/blob/0.8.6/src/f32/vec4.rs

## `build.rs`

The `cfg_if` macro solves the convoluted `else` block problem that you encounter
when using the `cfg` attribute on it's own, but the other problem I wanted to
solve was my `if` blocks were also a bit complicated and they were duplicated in
every method. What I really wanted was the C preprocessor ability to create
new defines. While you can't do this in your crate, you can do it from
[`build.rs`].

[`build.rs`]: https://doc.rust-lang.org/cargo/reference/build-scripts.html

The idea here is simplify the `feature` and `target_feature` options into a
single `cfg` check. The other idea is to create a `cfg` for what would be the
`else` condition. The advantage of doing this in the `build.rs` is I only need
to maintain this complicated condition in one place, and I can use normal rust
code to define it.

```rust
// build.rs
fn main() {
  let force_no_intrinsics = env::var("CARGO_FEATURE_NO_INTRINSICS").is_ok();

  let target_feature_sse2 = env::var("CARGO_CFG_TARGET_FEATURE")
    .map_or(false, |cfg| cfg.split(',').find(|&f| f == "sse2").is_some());

  if target_feature_sse2 && !force_no_intrinsics {
    println!("cargo:rustc-cfg=vec4sse2");
  } else if target_feature_wasm32 && !force_no_intrinsics {
    println!("cargo:rustc-cfg=vec4wasm32");
  } else {
    println!("cargo:rustc-cfg=vec4f32");
  }
}
```

Then in my Rust code I can check for this with the `cfg` attribute.

```rust
impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    #[cfg(vec4sse2)]
    unsafe {
      _mm_cvtss_f32(self.0)
    }

    #[cfg(vec4wasm32)]
    unsafe {
      f32x4_extract_lane(self.0)
    }

    #[cfg(vec4f32)]
    self.0
  }
}
```

Each `cfg` is mutually exclusive so there will never be more than one used in a
single method. It's a bit different if you are used to the C preprocessor
`if/else if/else` conditionals but the end result achieves the same goal.

You can see an example of this approach in [glam 0.8.7].

[glam 0.8.7]: https://github.com/bitshifter/glam-rs/blob/0.8.7/src/f32/vec4.rs

It is possible to pass key value `cfg` pairs from `build.rs` so I could have
something like `#[cfg(myfeature = "sse2")]` if I liked, see the cargo
[rustc-cfg] documentation for more details.

[rustc-cfg]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-cfg

Also there's no reason this technique of creating your own `cfg` keys with
`build.rs` won't also work with the other technique's I've explored above.

## Many yaks were shaved

This is the end of my journey in conditional compilation in Rust, for now. I'm
not totally sure I'll stick with the solution I've got. If I were to support
many architectures in glam it may get too unwieldy to continue doing things this
way, but that is a future problem.

## By modules addendum

[CAD1197 suggested some improvements] to my by module approach which would
address both the duplicated documentation and inconsistent public interface
issues I was seeing in my original implementation. The idea is a public
interface calls an internal method which is implemented in conditionally
compiled modules.

[CAD1197 suggested some improvements]: https://www.reddit.com/r/rust/comments/ghkunu/yak_shaving_conditional_compilation_in_rust/fqac3cm/

```rust
// file: vec4.rs
// public interface with docs

#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
mod vec4_sse2;
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
mod vec4_wasm32;
#[cfg(any(feature = "no-intrinsics", not(any(target_feature = "sse2", target_feature = "wasm32"))))]
mod vec4_f32;

#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
pub struct Vec4(__m128);

#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
pub struct Vec4(f32x4);

#[cfg(any(feature = "no-intrinsics", not(any(target_feature = "sse2", target_feature = "wasm32"))))]
pub struct Vec4(f32, f32, f32, f32);

impl Vec4 {
  /// Returns element `x`.
  #[inline]
  pub fn x(self) -> f32 {
    self._x()
  }
}
```

```rust
// vec4_sse2.rs
use super::Vec4;
impl Vec4 {
  #[inline(always)]
  pub(super) fn _x(self) -> f32 {
    unsafe { _mm_cvtss_f32(self.0) }
  }
}
```

```rust
// file: vec4_wasm32.rs
use super::Vec4;
impl Vec4 {
  #[inline(always)]
  pub(super) fn _x(self) -> f32 {
    unsafe { f32x4_extract_lane(self.0) }
  }
}
```

```rust
// file: vec4_f32.rs
use super::Vec4;
impl Vec4 {
  #[inline(always)]
  pub(super) fn _x(self) -> f32 {
    self.0
  }
}
```

I think if I were to support a lot of different architectures in glam I would
switch to this approach. There is a lot more boilerplate than my current
solution, however once the API is stable the boilerplate won't need to change
often.

If you have an comments or feedback you can reply to my posts on [/r/rust].

[/r/rust]: https://www.reddit.com/r/rust/comments/ghkunu/yak_shaving_conditional_compilation_in_rust/
