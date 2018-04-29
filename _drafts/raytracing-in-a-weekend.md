---
layout: post
title:  "Ray Tracing in a Weekend (in Rust)"
categories: blog
---

I was inspired to work through Peter Shirley's Ray Tracing in a Weekend minibook (let's call it RTW for the rest of this post) but of course, to write it in Rust instead of the C++ that's in the book. I found out about the book via [@aras_p](https://twitter.com/aras_p)'s [blog series](http://aras-p.info/blog/2018/03/28/Daily-Pathtracer-Part-0-Intro/) about a [toy path tracer](https://github.com/aras-p/ToyPathTracer) he's been building.

This post will describe how I went about translating a C++ project to Rust. I will introduce some of the Rust features I used and how they compare to both the C++ used in RTW's code and C++ features that are similar to Rust. I probably won't talk about ray tracing much at all so if you are interested in learning about that I recommend reading Peter's book!

Additionally neither the book C++ or my Rust are optimized code, Aras's blog series covers a lot of different optimizations he's performed, I have not done that yet. My Rust implementation does appear to perform faster than he C++ implementation (~40 seconds compared to ~90 seconds for a similar sized scene). I have not investigated why this is the case, but I have some ideas which will be covered later. I mostly wanted to check that my code was in the same ball park and it certainly seems to be.

## Materials

RTIAW introduces three materials, Lambertial, Metal and Dielectric. These materials implement a common interface in the C++ code:

```c++
class material  {
    public:
        virtual bool scatter(const ray& r_in, const hit_record& rec, vec3& attenuation, ray& scattered) const = 0;
};

class metal : public material {
    public:
        metal(const vec3& a, float f) : albedo(a) { if (f < 1) fuzz = f; else fuzz = 1; }
        virtual bool scatter(const ray& r_in, const hit_record& rec, vec3& attenuation, ray& scattered) const;
        vec3 albedo;
        float fuzz;
};

```

Rust doesn't have classes, it's not an OOP language. That doesn't mean you can't achieve some of the useful things OOP provides like encapsulation and polymorphism. There are a couple of approaches to translating this interface to Rust. Rust traits are a bit like an abstract interface, although they aren't attached to a specific type, types implement the traits. So for example we could define a material trait:

```rust
pub trait Material {
	fn scatter(r_in: &Ray, rec: &HitRecord, attenuation: &mut Vec3, scattered: &mut Vec) -> bool;
}

struct Metal {
	albedo: Vec3,
	fuzz: f32,
}

impl Material for Metal {
	fn scatter(r_in: &Ray, rec: &HitRecord, attenuation: &mut Vec3, scattered: &mut Vec) -> bool {
		// do stuff
	}
}
```

Note that in Rust struct data is declared separately to it's methods (the `impl`) and the trait implementation is separate again. Personally I like this separation of data from implementation, I think it keeps things more focused - what data do I have versus what methods I've implemented.

That ends up feeling pretty similar to the C++ code. In the  RTIAW code the `sphere` object owns the `material`. This means `material` is heap allocated, as each implementation could be a different size, the best we can do is heap allocate the object and store a pointer to it. This is also true in Rust, if I wanted my `Sphere` to own a `Material` object I would need to store a `Box<Material>` on the `Sphere`. You can think of `Box` as similar to `std::unique_ptr` in C++.

Since there are a small number of materials and the data size of the different types of materials is not large I decided to implement these with Rust enums instead. Using enums has the advantage that their size is know at compile time so we can store them by value instead of by pointer and avoid some allocations and indirection. My enum looks like this:

```rust
#[derive(Clone, Copy)]
pub enum Material {
    Lambertian { albedo: Vec3 },
    Metal { albedo: Vec3, fuzz: f32 },
    Dielectric { ref_idx: f32 },
}

```

Each enum variant contains data fields. I've named my fields for clarity but you don't have to. Rust enums are effictively tagged unions. C++ unions are untagged and have restrictions on the data you can store in them. "Tagged" just means storing some kind of type identifier. The `#[derive(Clone, Copy)]` just tells Rust that this enum is trivially copyable, e.g. OK to `memcpy` under the hood. To implement the `scatter` method we pattern match on the material enum:

```rust
impl Material {
    fn scatter(&self, ray: &Ray, ray_hit: &RayHit, rng: &mut Rng) -> Option<(Vec3, Ray)> {
        match *self {
            Material::Lambertian { albedo } => {
                Material::scatter_lambertian(albedo, ray, ray_hit, rng)
            }
            Material::Metal { albedo, fuzz } => {
                Material::scatter_metal(albedo, fuzz, ray, ray_hit, rng)
            }
            Material::Dielectric { ref_idx } => {
                Material::scatter_dielectric(ref_idx, ray, ray_hit, rng)
            }
        }
    }
}

```

The `match` statement in Rust is like C/C++ `switch` on steriods. I'm not doing anything particularly fancy in this match, one thing I am doing though is destructuring the different enum variants to access their fields, which I then use in the specific implementation for each material.

It's also worth talking about the return type here. The RTIAW C++ `scatter` interface returns a `bool` if the material scattered the ray and returns `attenuation` and `scattered` via reference parameters. This API does leave the question, what are these return parameters set to when `scatter` returns false? The RTIAW implementaton only uses these values if `scatter` returns true but in the case of the `metal` material the `scattered` ray is calculated regardless. To avoid any ambiguity, I'm returning these values as `Option<(Vec3, Ray)>`. There are a couple of things going on here. First the `(Vec3, Ray)` is a tuple, I was too lazy to make a dedicated struct for this return type and tuples are pretty easy to work with. The `Option` type is an optional value, it can either contain `Some` value or `None` if it dones not.

This `scatter` call and it's return value are handled like so:

```rust
if let Some((attenuation, scattered)) =
	ray_hit.material.scatter(ray_in, &ray_hit, rng)
{
	// do stuff
}
```

The `if let` is a convenient syntactic suger for pattern matching when you only care about one value, in this case the `Some`. Destructuring is being used again here to access the contents of the tuple returned in the `Option`.

## Hitables

RTW introduces a ray collision result structure `hit_record` and a `hitable` abstract interface which is implemented for `sphere` in the book with the intention of adding other objects later. The C++ code looks like so:

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
        virtual bool hit(const ray& r, float t_min, float t_max, hit_record& rec) const = 0;
};
```

There are a couple of different ways to approach this in Rust. Rust doesn't have classes, it's not an OOP language.

abstract interface -> trait

option type for return

if let

option in c++

