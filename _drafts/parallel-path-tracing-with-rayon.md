---
layout: post
title:  "Parallel Path Tracing with Rayon"
categories: blog
---

The path tracer I talked about in my previous [post]({{ site.baseurl }}{% post_url 2018-04-29-rust-ray-tracer-in-one-weekend %}) runs on one core, but my laptop's CPU has 4 physical cores. That seems like an easy way to make this thing faster right? There's a Rust library called [Rayon](https://crates.io/crates/rayon) which provides parallel iterators to divide your data into tasks and run it across multiple threads. Sounds like an easy speed boost.

One of the properties of Rust's type system is it detects shared memory data races at compile time. This property is a product of Rust's ownership model which does not allow shared mutable state. You can read more about this in the [Fearless Concurrency](https://doc.rust-lang.org/book/second-edition/ch16-00-concurrency.html) chapter of the Rust Book or for a more formal analysis [Securing the Foundations of the Rust Programming Language](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf). As a consequence of this, Rayon also guarantees data-race freedom.

# Iterators

My initial Rust implementation of the main loop was pretty similar to the original C++. I'm reserving space in a vector and pushing RGB values into it for each pixel.

```rust
// create a vector and reserve enough space for the final image to avoid allocations
let mut buffer = Vec::with_capacity((nx * ny * 3) as usize);
for j in 0..ny { // loop through rows
	for i in 0..nx { // loop through columns
		let mut col = vec3(0.0, 0.0, 0.0);
		for _ in 0..ns { // generate a number of samples for each pixel
			let u = (i as f32 + rng.next_f32()) / nx as f32;
			let v = ((ny - j - 1) as f32 + rng.next_f32()) / ny as f32;
			let ray = camera.get_ray(u, v, &mut rng);
			col += scene.ray_trace(&mut ray, 0, &mut rng);
		}
		col /= ns as f32;
		// push each colour channel to the output buffer
		buffer.push((255.99 * col.x.sqrt()) as u8);
		buffer.push((255.99 * col.y.sqrt()) as u8);
		buffer.push((255.99 * col.z.sqrt()) as u8);
	}
}
```

The above code is looping through all rows (j) then all columns (i) and ray casting ns samples for each pixel to produce a final colour, which is then pushed into the output buffer. This takes about 39 seconds to process on my laptop.

I don't strictly need to keep this structure to use Rayon, but processing each row in parallel sounds like an OK start.


```rust
// create an output buffer to write pixel data to (initialized to 0)
let mut buffer: Vec<u8> = std::iter::repeat(0)
	.take((nx * ny * channels) as usize)
	.collect();
buffer
	.chunks_mut((nx * channels) as usize) // iterate each row
	.rev() // iterate in reverse (otherwise image is upside down)
	.enumerate() // generate an index for each row we're iterating
	.for_each(|(j, row)| {
		for (i, rgb) in row.chunks_mut(channels as usize).enumerate() {
			let mut col = vec3(0.0, 0.0, 0.0);
			for _ in 0..ns {
				let u = (i as f32 + rng.next_f32()) / nx as f32;
				let v = (j as f32 + rng.next_f32()) / ny as f32;
				let ray = camera.get_ray(u, v, &mut rng);
				col += scene.ray_trace(&ray, 0, &mut rng);
			}
			col /= ns as f32;
			let mut iter = rgb.iter_mut();
			*iter.next().unwrap() = (255.99 * col.x.sqrt()) as u8;
			*iter.next().unwrap() = (255.99 * col.y.sqrt()) as u8;
			*iter.next().unwrap() = (255.99 * col.z.sqrt()) as u8;
		}
	});
```

Iterator code runs in ~37 seconds

# Data races

Change `chunks_mut` to `par_chunks_mut`


```
error[E0387]: cannot borrow data mutably in a captured outer variable in an `Fn` closure
  --> src/main.rs:95:41
   |
95 |                     let u = (i as f32 + rng.next_f32()) / nx as f32;
   |                                         ^^^
   |
help: consider changing this closure to take self by mutable reference
  --> src/main.rs:91:19
   |
91 |           .for_each(|(j, row)| {
   |  ___________________^
92 | |             for (i, rgb) in row.chunks_mut(channels as usize).enumerate() {
93 | |                 let mut col = vec3(0.0, 0.0, 0.0);
94 | |                 for _ in 0..ns {
...  |
105| |             }
106| |         });
   | |_________^

error[E0387]: cannot borrow data mutably in a captured outer variable in an `Fn` closure
  --> src/main.rs:99:28
   |
99 |                     col += scene.ray_trace(&ray, 0, &mut rng);
   |                            ^^^^^
   |
help: consider changing this closure to take self by mutable reference
  --> src/main.rs:91:19
   |
91 |           .for_each(|(j, row)| {
   |  ___________________^
92 | |             for (i, rgb) in row.chunks_mut(channels as usize).enumerate() {
93 | |                 let mut rng = XorShiftRng::from_seed(seed);
94 | |                 let mut col = vec3(0.0, 0.0, 0.0);
...  |
106| |             }
107| |         });
   | |_________^
```

Fix data races and it runs 10 seconds, nearly a 4x speed up, which is what I would hope for on a 4 core machine.
