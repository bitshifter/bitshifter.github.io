---
layout: post
title:  One interface, multiple architecures"
excerpt_separator: <!--more-->
tags: performance profiling
---

In C++ there is a common idiom used when writing a low level interface that
has different implementations for multiple architectures. That is to use the
preprocessor to select the appropriate implementation at compile time. This
pattern is frequently used in C++ math libraries.

For example, the RTM library sets up it's own defines based on definitions
passed in from the build system:

```cpp
#if !defined(RTM_NO_INTRINSICS)
#if defined(__SSE2__) || defined(_M_IX86) || defined(_M_X64)
	#define RTM_SSE2_INTRINSICS
#endif
#endif
```

These defines are then used to select the approprate implemtation to compile:

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

At the moment different implementations are split into different modules, e.g.
for `Vec4`:
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

In C/C++ the pre-processor makes it pretty easy to support multiple
implementations in a function at compile time, for example from the RTM library:

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
