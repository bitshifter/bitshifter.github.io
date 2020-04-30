---
layout: post
title:  One interface, multiple architecures"
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
C/C++) `__SSE2__`, `_M_IX86` and `_M_X64` are checked, if any of these are defined the the Realtime
Math library enables `SSE2` support with it's own `RTM_SSE2_INTRINSICS` define.  These defines are
then used to select the approprate implementation to compile:

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

I don't know if this idiom has a name but it's pretty common.

# From the C Preprocessor to Rust

There is nothing quite the same as the C Preprocessor in Rust. That is generally considered a good
thing, however when it comes to the above idiom there is nothing I've found in Rust that is quite as
convenient.

I've tried 3 different approaches to this problem in [glam] which I'll discuss the pros on cons of
here. Perhaps first though, let's talk about the primary features that Rust provides to solve this
problem.

# Conditional compilation in Rust

Source code can be conditionally compiled using the attributes `cfg` and `cfg_attr` and the built-in
`cfg!` macro. These conditions are based on the target architecture of the compiled crate, arbitrary
values passed to the compiler, and so on. Usually for SIMD we're interested in `target_arch`,
`target_feature` and our own crate's `feature`s.

So can we achive the same thing as the C/C++ example above using `cfg`?

```rust
#[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
// now what?
```

So Rust is simpler in that we only need to check `target_feature = sse` but at that point we're
stuck, se can't set a `feature` in Rust code, these can only come from the build system.

There are some other differences also. There is not `else` with the attribute check which is why we
check that `not(feature = "no-intrinsics")` is checked here.

So perhaps instead of setting our own define lets just make these `cfg` checks in our function.

```rust
#[inline]
fn vector_get_x(intput: vector4f) -> f32 {
  #[cfg(all(not(feature = "no-intrinsics"), target_feature = "sse2"))]
  unsafe { _mm_cvtss_f32(input.0) }
  #[cfg(all(not(feature = "no-intrinsics"), target_feature = "wasm32"))]
  unsafe { f32x4_extract_lane(input.0, 0) }
  #[cfg(any(feature = "no-intrinsics, not(any(target_feature = "sse2", target_feature = "wasm32"))))]
  { input.0 }
}
```

The lack of an `else` equivalent is making this a lot more verbose than the C preprocessor version.
It might be okay for one or two methods, but not for hundreds and every time a new `target_feature`
is supported the fallback `cfg` check is going to get even more complex.

If you are familiar with Rust you might be thinking we could use the `cfg!` macro here, let's
consider that:

```rust
  if cfg!(all(not(feature = "no-intrinsics"), target_feature = "sse2")) {
    unsafe { _mm_cvtss_f32(input.0) }
  } else if cfg!(all(not(feature = "no-intrinsics"), target_feature = "wasm32")) {
    unsafe { f32x4_extract_lane(input.0, 0) }
  } else {
    input.0
  }
```

This looks much better in principle but it doesn't compile. The problem here is the compiler still
tries to parse every block and if you are on a platform that supports SSE2 then the wasm32 module
won't exist, and vice verse.

That is the defintion of the problem, let's look at the different approaches I've tried with glam.

# Conditional compilation by module

At the moment different implementations are split into different modules, e.g.  for `Vec4`:
* vec4_f32.rs - scalar f32 `struct Vec4` declaration and implementation
* vec4_sse2.rs - SSE2 SIMD `struct Vec4` declaration and implementation vec4.rs
* - common methods use by both implementations

The advantages of doing things this way are:
* Keeps different implementations separate, so there's no need for `#[cfg(...)]`
* blocks everywhere, just where the modules are imported The code is simple - if
* you are debugging SSE2 it's straight Rust code and just the SSE2
* implementation

However, there are a number of downsides to this approach:
* The interface needs to be duplicated for each implementation - this is
* mitigated somewhat by ensuring the interface has 100% test coverage.
* Documentation also needs to be duplicated for each implementation - this is a
* bit more annoying Adding additional architectures will start to make the
* library difficult to maintain due to the above issues

In C/C++ the pre-processor makes it pretty easy to support multiple implementations in a function at
compile time, for example from the RTM library:

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

There are a few reasons why this is difficult to do in Rust. For one,
`RTM_SSE2_INTRINSICS` is itself a define which is based on definitions from the
build system:

```cpp
#if !defined(RTM_NO_INTRINSICS)
#if defined(__SSE2__) || defined(_M_IX86) || defined(_M_X64)
	#define RTM_SSE2_INTRINSICS
#endif
#endif
```

As far as I know, Rust can't create new `cfg` features in Rust code like you can
in C++. This could be done using a `build.rs` which might be an OK option but
`build.rs` itself is a little controversial even if using if for this purpose
wouldn't be. Because it's not easy to create new feature flags programatically,
the `cfg` blocks become complicated. The above block of code from RTM translated
to Rust would be:

```rust
fn vector_get_x(input: Vector4f) -> f32 {
	#[cfg(all(target_feature = "sse2", not(feature = "rtm-no-intrinsics")))]
	{
		unsafe { _mm_cvtss_f32(input) }
	}
	#[cfg(all(target_feature = "arm-neon", not(feature = "rtm-no-intrinsics")))]
	{
		unsafe { vgetq_lane_f32(input, 0) }
	}
	#[cfg(any(not(any(target_feature = "sse2", target_feature = "arm-neon")), feature = "rtm-no-intrinsics"))]
	{
		input.x
	}
}
```

The last block will continue to get more complicated as more target features are
added. This complexity will be in every function that contains a SIMD
implementation.

# `cfg-if`

Everything is wrapped in a macro.

I found rust fmt wouldn't format things inside the macro.
Debugging an unoptimized build with gdb seemed to work OK.
