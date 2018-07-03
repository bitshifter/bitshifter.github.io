---
layout: post
title:  "Ray Tracing in a Weekend in Rust"
excerpt_separator: <!--more-->
tags: rust raytracing c++
---

I was inspired to work through [Peter Shirley](https://twitter.com/Peter_shirley)'s [Ray Tracing in a Weekend](https://in1weekend.blogspot.com.au/2016/01/ray-tracing-in-one-weekend.html) mini book (for brevity RTIAW) but I wanted to write it in Rust instead of the C++ that's used in the book. I found out about the book via [@aras_p](https://twitter.com/aras_p)'s [blog series](http://aras-p.info/blog/2018/03/28/Daily-Pathtracer-Part-0-Intro/) about a [toy path tracer](https://github.com/aras-p/ToyPathTracer) he's been building.

This post will describe how I went about translating a C++ project to Rust, so it's really intended to be an introduction to Rust for C++ programmers. I will introduce some of the Rust features I used and how they compare to both the C++ used in RTIAW's code and more "Modern" C++ features that are similar to Rust. I probably won't talk about ray tracing much at all so if you are interested in learning about that I recommend reading Peter's book!

Additionally neither the book C++ or my Rust are optimized code, Aras's blog series covers a lot of different optimizations he's performed, I have not done that yet. My Rust implementation does appear to perform faster than the C++ (~40 seconds compared to ~90 seconds for a similar sized scene). I have not investigated why this is the case, but I have some ideas which will be covered later. I mostly wanted to check that my code was in the same ball park and it certainly seems to be.

<!--more-->

## Materials

RTIAW introduces three materials, Lambertian, Metal and Dielectric. These materials implement a common interface in the C++ code:

```c++
class material  {
  public:
    virtual bool scatter(
      const ray& r_in,
      const hit_record& rec,
      vec3& attenuation,
      ray& scattered) const = 0;
};

class metal : public material {
  public:
    metal(const vec3& a, float f);
    virtual bool scatter(
      const ray& r_in,
      const hit_record& rec,
      vec3& attenuation,
      ray& scattered) const;
    vec3 albedo;
    float fuzz;
};
```

Rust doesn't have classes, it's not strictly speaking an OOP language (see [is Rust an OOP Language](https://doc.rust-lang.org/book/second-edition/ch17-00-oop.html). That doesn't mean you can't achieve some of the useful things OOP provides like encapsulation and polymorphism. There are a couple of approaches to translating this interface to Rust. Rust traits are a bit like an abstract interface, although they aren't attached to a specific type, types implement the traits. So for example we could define a material trait:

```rust
pub trait Material {
  fn scatter(
    &self,
    r_in: &Ray,
    rec: &HitRecord,
    attenuation: &mut Vec3,
    scattered: &mut Vec)
  -> bool;
}

struct Metal {
  albedo: Vec3,
  fuzz: f32,
}

impl Material for Metal {
  fn scatter(
    &self,
    r_in: &Ray,
    rec: &HitRecord,
    attenuation: &mut Vec3,
    scattered: &mut Vec
  ) -> bool {
    // do stuff
  }
}
```

Note that in Rust [struct data](https://doc.rust-lang.org/book/second-edition/ch05-01-defining-structs.html) is declared separately to it's [methods](https://doc.rust-lang.org/book/second-edition/ch05-03-method-syntax.html) (the `impl`) and the [trait](https://doc.rust-lang.org/book/second-edition/ch10-02-traits.html) implementation is separate again. Personally I like this separation of data from implementation, I think it makes it easier to focus on what the data is. The first method parameter is `&self`, Rust uses an explicit `self` instead of the implicit `this` used in C++ method calls. Variables are immutable by default, so our output variables here are declared as mutable references with `&mut`.

That ends up feeling pretty similar to the C++ code. In the  RTIAW code the `sphere` object owns the `material`. This means `material` is heap allocated as each concrete material type could be a different size, the easy approach is to heap allocate the object and store a pointer to it. This is also true in Rust, if I wanted my `Sphere` to own a `Material` object I would need to store a `Box<Material>` on the `Sphere`. You can think of a [box](https://doc.rust-lang.org/book/second-edition/ch15-01-box.html) as similar to `std::unique_ptr` in C++.

Since there are a small number of materials and the data size of the different types of materials is not large I decided to implement these with Rust [enums](https://doc.rust-lang.org/book/second-edition/ch06-01-defining-an-enum.html) instead. Using enums has the advantage that their size is know at compile time so we can store them by value instead of by pointer and avoid some allocations and indirection. My enum looks like this:

```rust
#[derive(Clone, Copy)]
pub enum Material {
  Lambertian { albedo: Vec3 },
  Metal { albedo: Vec3, fuzz: f32 },
  Dielectric { ref_idx: f32 },
}
```

Each enum variant contains data fields. I've named my fields for clarity but you don't have to. Rust enums are effectively tagged unions. C++ unions are untagged and have restrictions on the data you can store in them. "Tagged" just means storing some kind of type identifier. The `#[derive(Clone, Copy)]` just tells Rust that this enum is trivially copyable, e.g. OK to `memcpy` under the hood. To implement the `scatter` method we [pattern match](https://doc.rust-lang.org/book/second-edition/ch06-02-match.html) on the material enum:

```rust
impl Material {
  fn scatter(&self, ray: &Ray, ray_hit: &RayHit, rng: &mut Rng)
  -> Option<(Vec3, Ray)> {
    match *self {
      Material::Lambertian { albedo } => {
        // lambertian implementation
      }
      Material::Metal { albedo, fuzz } => {
        // metal implementation
      }
      Material::Dielectric { ref_idx } => {
        // dielectric implementation
      }
    }
  }
}
```

The `match` statement in Rust is like C/C++ `switch` on steroids. I'm not doing anything particularly fancy in this match, one thing I am doing though is destructuring the different enum variants to access their fields, which I then use in the specific implementation for each material.

It's also worth talking about the return type here. The RTIAW C++ `scatter` interface returns a `bool` if the material scattered the ray and returns `attenuation` and `scattered` via reference parameters. This API does leave the question, what are these return parameters set to when `scatter` returns false? The RTIAW implementation only uses these values if `scatter` returns true but in the case of the `metal` material the `scattered` ray is calculated regardless. To avoid any ambiguity, I'm returning these values as `Option<(Vec3, Ray)>`. There are a couple of things going on here. First the `(Vec3, Ray)` is a [tuple](https://doc.rust-lang.org/book/second-edition/ch03-02-data-types.html#grouping-values-into-tuples), I was too lazy to make a dedicated struct for this return type and tuples are pretty easy to work with. The [option type](https://doc.rust-lang.org/book/second-edition/ch06-01-defining-an-enum.html#the-option-enum-and-its-advantages-over-null-values) is an optional value, it can either contain `Some` value or `None` if it does not.

This `scatter` call and it's return value are handled like so:

```rust
if let Some((attenuation, scattered)) =
  ray_hit.material.scatter(ray_in, &ray_hit, rng)
{
  // do stuff
}
```

The [if let](https://doc.rust-lang.org/book/second-edition/ch06-03-if-let.html) syntax is a convenient way to perform pattern matching when you only care about one value, in this case the `Some`. Destructuring is being used again here to access the contents of the tuple returned in the `Option`.

C++ does have support for both `tuple` in C++11 and `optional` in C++17 so I've written something somewhat equivalent to the Rust version using C++17 below (also on [Compiler Explorer](https://godbolt.org/g/KHEz2k). I find the Rust a lot more ergonomic and readable.

```c++
#include <optional>
#include <tuple>
#include <variant>

using std::optional;
using std::tuple;
using std::variant;

struct Vec3 { float x; float y; float z; };
struct Ray { Vec3 origin; Vec3 direction; };
struct RayHit;

struct Lambertian {
  Vec3 albedo;
  optional<tuple<Vec3, Ray>> scatter(
    const Ray&, const RayHit&) const;
};

struct Metal {
  Vec3 albedo;
  float fuzz;
  optional<tuple<Vec3, Ray>> scatter(
    const Ray&, const RayHit&) const;
};

struct Dielectric {
  float ref_idx;
  optional<tuple<Vec3, Ray>> scatter(
    const Ray&, const RayHit&) const;
};

typedef variant<Lambertian, Metal, Dielectric> Material;

optional<tuple<Vec3, Ray>> scatter(
    const Material & mat, const Ray& ray, const RayHit & hit) {
  if (auto p = std::get_if<Lambertian>(&mat)) {
    return p->scatter(ray, hit);
  }
  else if (auto p = std::get_if<Metal>(&mat)) {
    return p->scatter(ray, hit);
  }
  else if (auto p = std::get_if<Dielectric>(&mat)) {
    return p->scatter(ray, hit);
  }
  return {};
}

// dummy function declaration to prevent dead code removal
void dummy(const Vec3&, const Ray&);

// dummy function to call the scatter code
void test(const Ray& ray, const RayHit& hit, const Material& mat) {
  if (auto result = scatter(mat, ray, hit)) {
    const auto & [attenuation, scattered] = *result;
    dummy(attenuation, scattered);
  }
}
```

## Hitables

RTIAW introduces a ray collision result structure `hit_record` and a `hitable` abstract interface which is implemented for `sphere` in the book with the intention of adding other objects later. The C++ code looks like so:

```c++
class material;

struct hit_record {
  float t;  
  vec3 p;
  vec3 normal; 
  material *mat_ptr;
};

class hitable  {
  public:
    virtual bool hit(
      const ray& r,
      float t_min,
      float t_max,
      hit_record& rec) const = 0;
};
```

In this instance since we only ever deal with sphere's I didn't bother creating a `Hitable` trait and just added a `hit` method to my `Sphere` type. This meant that my spheres can stored in contiguous memory unlike the C++ code where each `sphere` is stored as a `hitable`  pointer, which is heap allocated. This probably explains the performance difference I saw in my Rust version - there will be less cache misses. Not that my `Sphere` implementation is particularly efficient, it contains data like the material which wouldn't be used most of the time so a future optimization would be to split the sphere data into a structure of arrays for better cache utilisation and SIMD usage.

I name my Rust implementation of `hit_record` `RayHit`:

```rust
struct RayHit {
  t: f32,
  point: Vec3,
  normal: Vec3,
  material: Material,
}
```

One difference here is the way the material is stored. The C++ version stores a pointer to the material of the sphere that was hit. This is something that is not so simple in Rust due to Rust's [ownership system](https://doc.rust-lang.org/book/second-edition/ch04-00-understanding-ownership.html). To achieve something similar to the pointer to the material in Rust we would have to have a reference which immutably "borrows" the original data. Since the `RayHit` structure is short lived, it would be possible to make it borrow the material from the sphere that has been hit, however to do this we would need to annotate the lifetime relationship so that the Rust compiler knows that everything is OK. In this case I was lazy any just copied the material into the `RayHit` struct. It might not be the most efficient solution but the material's aren't that large. For the purposes of this post it might have been more interesting to annotate the lifetime of the material borrow though. Perhaps I will go into this in a subsequent post.

## Summing Up

These seemed like some of the more interesting differences between the C++ version and my Rust implementation. There are of course other interesting things but I think this post has got quite long enough. My Rust implementation can be found [here](https://github.com/bitshifter/pathtrace-rs/tree/2018-04-29) and the book's C++ version [here](https://github.com/petershirley/raytracinginoneweekend).

Hopefully at some point I will find some time to add some more features to this path tracer and to start on some optimization work with [Rayon](https://github.com/rayon-rs/rayon) and SIMD.

I haven't got around to setting up a comment system on my blog so if you have any comments or questions hit me up on twitter at [@bitshifternz](https://twitter.com/bitshifternz).

