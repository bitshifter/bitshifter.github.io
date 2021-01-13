# Another glam rewrite

The primary design goals of `glam` are to have an simple to understand and use API and to offer good
performance. For this reason, `glam` has largely avoiding using traits and generics in its public
interface and instead opted for concrete types such as `Vec3` for an `f32` 3D vector over something
like `Vector<f32>`. `glam` has also made used a lot of SSE2 for `f32` types behind the scenes, which
generally results in better performance than regular scalar `f32` code.

`glam` more recently added support for other primitive types such as `f64`, `i32`, `u32` and `bool`.
So now in addition to `Vec3` there are now `DVec3`, `IVec3`, `UVec3` and `BVec3` types. All of these
types are using scalar math. Some types like `Vec4` also have a SSE2 implementation if the SSE2
target feature is available as well as a regular scalar `f32` implementation for when SSE2 is not
supported. In order to support all of these different primitive types and implementations behind the
scenes `glam` makes use of traits and generics. This is hidden from the user and the public API only
exposes concrete types and for the most part not how they are implemented.

This may seem like overkill which you could feasibly have a generic API and some convenient type
aliases along the lines of `type Vec3 = Vector3<f32>` there are a couple of reasons why this
approach would not be able to achieve the same result:

1. This would present a more complicated interface to the user, documentation is also more complex.
   Especially when some types can support some functionality with real numbers, but not unsigned
   integers and so on.
2. Rust specialisation is not advanced enough to support use SSE2's `__m128` for storage for a
   `Vector<f32>` type, for example.

