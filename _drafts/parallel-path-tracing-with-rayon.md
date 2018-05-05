---
layout: post
title:  "Parallel Path Tracing with Rayon"
categories: blog
---

The path tracer I talked about in my previous [post]({{ site.baseurl }}{% post_url 2018-04-29-rust-ray-tracer-in-one-weekend %}) runs on one core, but my laptop's CPU has 4 physical cores. That seems like an easy way to make this thing faster right? There's a Rust library called [Rayon](https://crates.io/crates/rayon) which provides parallel iterators to divide your data into tasks and run it across multiple threads. Sounds like an easy speed boost.

One of the properties of Rust's type system is it detects shared memory data races at compile time. This property is a product of Rust's ownership model which does not allow shared mutable state. You can read more about this in the [Fearless Concurrency](https://doc.rust-lang.org/book/second-edition/ch16-00-concurrency.html) chapter of the Rust Book or for a more formal analysis [Securing the Foundations of the Rust Programming Language](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf). As a consequence of this, Rayon also guarantees data-race freedom. Rusts strictness around ownership can be pretty difficult to get to grips with but fearless concurrency is quite the reward.

# Iterators

```rust
let mut buffer = Vec::with_capacity((nx * ny * 3) as usize);
for j in 0..ny {
	for i in 0..nx {
		let mut col = vec3(0.0, 0.0, 0.0);
		for _ in 0..ns {
			let u = (i as f32 + rng.next_f32()) / nx as f32;
			let v = ((ny - j - 1) as f32 + rng.next_f32()) / ny as f32;
			let ray = camera.get_ray(u, v, &mut rng);
			col += scene.ray_trace(&ray, 0, &mut rng);
		}
		col /= ns as f32;
		buffer.push((255.99 * col.x.sqrt()) as u8);
		buffer.push((255.99 * col.y.sqrt()) as u8);
		buffer.push((255.99 * col.z.sqrt()) as u8);
	}
}
```

# Data races

# Inlining
