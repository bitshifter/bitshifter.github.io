---
layout: post
title: Conditional compilation in Rust
excerpt_separator: <!--more-->
tags: performance profiling
---

In C++ there is a common idiom used when writing a low level interface that has different
implementations for multiple architectures. That is to use the preprocessor to select the
appropriate implementation at compile time. This pattern is frequently used in C++ math libraries.

For example, the C++ [Realtime Math] library sets up it's own defines based on definitions passed in
from the build system:

[Realtime Math]: https://github.com/nfrechette/rtm

```cpp
#if !defined(RTM_NO_INTRINSICS)
  #if defined(__SSE2__) || defined(_M_IX86) || defined(_M_X64)
    #define RTM_SSE2_INTRINSICS
  #endif
#endif
```

Intrinsics can be entirely disabled if `RTM_NO_INTRINSICS` is defined. Otherwise the predefined
compiler specific definitions (different compilers use different defines for this stuff, welcome to
C/C++) `__SSE2__`, `_M_IX86` and `_M_X64` are checked, if any of these are defined the Realtime
Math library enables `SSE2` support with it's own `RTM_SSE2_INTRINSICS` define.  These defines are
then used to select the appropriate implementation to compile:

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

I don't know if this idiom has a name but it's pretty common, especially in math libraries which
usually have a lot of small performance critical functions.

# From the C Preprocessor to Rust

There is nothing quite the same as the C Preprocessor in Rust. That is generally considered a good
thing, however when it comes to the above idiom there is nothing I've found in Rust that feels quite
as convenient.

I've tried 3 different approaches to this problem in [glam] which I'll discuss the pros on cons of
here. Perhaps first though, let's talk about the primary features that Rust provides to solve this
problem.

# Conditional compilation in Rust

Source code can be conditionally compiled using the attributes `cfg` and `cfg_attr` and the built-in
`cfg!` macro. These conditions are based on the target architecture of the compiled crate, arbitrary
values passed to the compiler, and so on. Usually for SIMD we're interested in `target_arch`,
`target_feature` and our own crate's `feature`s.

So can we achieve the same thing as the C/C++ example above using `cfg`?

```rust
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
// now what?
```

So Rust is simpler in that we only need to check `target_feature = sse` but at that point we're
stuck, we can't set a `feature` in Rust code, these can only come from the build system. I will come
back to this point later one.

There are some other differences also. There is not `else` with the attribute check which is why we
check that `not(feature = "no-intrinsics")` is checked here.

So perhaps instead of setting our own define lets just make these `cfg` checks in our function.

```rust
#[inline]
impl Vec4 {
  fn x(self) -> f32 {
    #[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
    unsafe { _mm_cvtss_f32(self.0) }
    #[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
    unsafe { f32x4_extract_lane(self.0, 0) }
    #[cfg(any(feature = "no-intrinsics, not(any(target_feature = "sse2", target_feature = "wasm32"))))]
    { self.0 }
  }
}
```

The lack of an `else` equivalent is making this a lot more verbose than the C preprocessor version.
It might be okay for one or two methods, but not for hundreds and every time a new `target_feature`
is supported the fallback `cfg` check is going to get even more complex.

If you are familiar with Rust you might be thinking we could use the `cfg!` macro here, let's
consider that:

```rust
#[inline]
impl Vec4 {
  fn x(self) -> f32 {
    if cfg!(all(not(feature = "no-intrinsics"), target_feature = "sse2")) {
      unsafe { _mm_cvtss_f32(input.0) }
    } else if cfg!(all(not(feature = "no-intrinsics"), target_feature = "wasm32")) {
      unsafe { f32x4_extract_lane(input.0, 0) }
    } else {
      input.0
    }
  }
}
```

This looks much better in principle but it doesn't compile. The problem here is the compiler still
tries to parse every block and if you are on a platform that supports `sse2` then the `wasm32`
module won't exist, and vice verse.

That is the definition of the problem, let's look at the different approaches I've tried with glam.

# Conditional compilation by module

This approach splits the different implementations for each architecture into it's own module and
then exports the appropriate module at compile time. This is the first approach I took in glam.
Continuing with the above example, separating into modules looks like so:

```rust
// vec4_sse2.rs
pub struct Vec4(__m128);
impl Vec4 {
  pub fn x(self) -> f32 {
    unsafe { _mm_cvtss_f32(input.0) }
  }
}
```

```rust
// file: vec4_wasm32.rs
pub struct Vec4(f32x4);
impl Vec4 {
  pub fn x(self) -> f32 {
    unsafe { f32x4_extract_lane(input.0) }
  }
}
```

```rust
// file: vec4_f32.rs
pub struct Vec4(f32, f32, f32, f32);
impl Vec4 {
  pub fn x(self) -> f32 {
    input.0
  }
}
```

```rust
// file: lib.rs
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
mod vec4_sse2;
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
mod vec4_wasm32;
#[cfg(any(feature = "no-intrinsics, not(any(target_feature = "sse2", target_feature = "wasm32"))))]
mod vec4_f32;

#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
pub use vec4_sse2::*;
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
pub use vec4_wasm32::*;
#[cfg(any(feature = "no-intrinsics, not(any(target_feature = "sse2", target_feature = "wasm32"))))]
pub use vec4_f32::*;

impl Vec4 {
  // common implementations
}
```

This simple code sample probably looks verbose compared to earlier samples, however when the module
contains 50 or so functions a complete absence of any `cfg` checks inside each architecture's module
is quite pleasant.

The advantages of doing things this way are:
* Keeps different implementations separate, so there's no need for `#[cfg(...)]` blocks in every
  method.
* The implementation for each architecture is normal code and easy to read.

However I found a number of downsides to this approach:
* The interface needs to be duplicated for each implementation - this is mitigated somewhat by
  ensuring the interface has near 100% test coverage.
* Documentation also needs to be duplicated for each implementation - this is a bit more annoying
  Adding additional architectures will start to make the library difficult to maintain due to the
  above issues

Having to duplicate the interface was annoying and even with a lot of tests sometimes I still made
mistakes. Having to duplicate the docs to keep `rust doc` happy was really the reason I moved away
from this method though.

Perhaps another way of doing this would be to have a single public interface with docs and have that
pull in the architecture specific module with the actual implementation. That would make it harder
to not have matching interfaces and there's one place for docs, but it would be a lot more boiler
plate and I'd need to make all the implementation methods `#[inline(always)]` to avoid overhead,
especially in non-optimised builds.

I could have also created trait for each type and make each architecture implement it, which would
mean documenting the trait and not each implementation and would mean there were no missing
implementations. The trait would have to be public to do this though and at that point it seemed
like I'm leaking internal implementation details to users.

# `cfg-if`

The `cfg-if` crate is probably the de facto solution to this problem in the Rust ecosystem. The
crate provides the `cfg_if` macro which supports similar functionality to the `if/else if/else`
branches of the C preprocessor. Here's our familiar sample code using `cfg_if`.

```rust
impl Vec4 {
  pub fn x(self) -> f32 {
    cfg_if! {
      if #[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))] {
        unsafe { _mm_cvtss_f32(self.0) }
      } else if #[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))] {
        unsafe { f32x4_extract_lane(input.0) }
      } else {
        self.0
      }
    }
  }
}
```

This is a pretty good option for mimicking the functionality provided by the C Preprocessor and
nicely solves the convoluted `else` block problem you run into using Rust's `cfg` attribute on it's
own, but I ran into a few problems.

Aesthetically, I didn't like having to wrap almost all of the glam code in a macro. This is a bit of
a personal preference but I did run into a couple of tooling issues also.

[Rustfmt] did not format code blocks inside `cfg_if` macros. I like being able to write code fast
and messy and then running `cargo fmt` to tidy it up when I'm done, so this was a bit of a
hindrance to my workflow. The other tooling problem was that [tarpaulin], the code coverage tool I
am using did not like `cfg_if` at all it seemed and my test coverage mysteriously dropped from 87%
to 73% which was a bit annoying.

The tooling issues are nothing to do with `cfg_if` and are most probably bugs that may well get
fixed one day, but until that happens it felt like I'd gone backwards a bit.

# `build.rs`

The `cfg_if` macro solves the convoluted `else` block problem that you encounter when using the
`cfg` attribute on it's own, but the other problem I wanted to solve was my `if` blocks were also a
bit complicated and they were duplicated in every method.

