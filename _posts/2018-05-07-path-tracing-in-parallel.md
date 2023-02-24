---
layout: post
title:  "Path tracing in parallel with Rayon"
excerpt_separator: <!--more-->
tags: rust raytracing c++ multithreading
---

The path tracer I talked about in my [previous post]({{ site.baseurl }}{% post_url 2018-04-29-rust-ray-tracer-in-one-weekend %}) runs on one core, but [my laptop's CPU](https://ark.intel.com/products/78930/Intel-Core-i7-4710HQ-Processor-6M-Cache-up-to-3_50-GHz) has 4 physical cores. That seems like an easy way to make this thing faster right? There's a Rust library called [Rayon](https://crates.io/crates/rayon) which provides parallel iterators to divide your data into tasks and run it across multiple threads.

One of the properties of Rust's type system is it detects shared memory data races at compile time. This property is a product of Rust's ownership model which does not allow shared mutable state. You can read more about this in the [Fearless Concurrency](https://doc.rust-lang.org/book/second-edition/ch16-00-concurrency.html) chapter of the Rust Book or for a more formal analysis [Securing the Foundations of the Rust Programming Language](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf). As a consequence of this, Rayon's API also guarantees data-race freedom.

<!--more-->

# Iterators

My initial Rust implementation of the main loop was pretty similar to the original C++. I'm reserving space in a vector and pushing RGB values into it for each pixel.

```rust
let mut buffer = vec![0u8; (nx * ny * channels) as usize];
for j in 0..ny { // loop through rows
  for i in 0..nx { // loop through columns
    let mut col = vec3(0.0, 0.0, 0.0);
    for _ in 0..ns { // generate ns samples for each pixel
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

The above code is looping through all `j` rows and all `i` columns and ray casting `ns` samples for each pixel to produce a final colour, which is then pushed into the output buffer. This takes about 39 seconds to process on my laptop.

I don't strictly need to keep this structure to use Rayon, but processing each row in parallel sounds like an OK starting point.

If I was to parallelise this in C++ I would make sure the output buffer is pre-created (not just reserved) since I know exactly how big it is beforehand. In C++ I would determine the output address from the i and j values and just write there since I know each thread will be writing to a different location there are no data races, assuming I don't make any mistakes. To do the same in Rust would require each thread to have a mutable reference to the output buffer, but multiple mutable references are not allowed in safe Rust. You can achieve this in unsafe Rust, and that's what the iterators are using behind the scenes which is presented as a safe API for the programmer. Coming from C++ I'm not used to Rust's heavier use of iterators and it usually takes me a while to translate my loops into idiomatic Rust.

There is a base `Iterator` trait which all `Iterators` are built from:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
```

There's not much to it - just one method when returns an `Option` type. There are more sophisticated traits built on top of that to create quite a rich variety of iterators. You can read more about iterators in the [Processing a Series of items with Iterators](https://doc.rust-lang.org/book/second-edition/ch13-02-iterators.html) chapter of the Rust book.

Back to my loop. I want to iterate over my output buffer a row at a time, generating a number of samples per pixel and finally writing the RGB values to the output. The slice method `chunks_mut` returns an iterator over the slice in non-overlapping mutable chunks. Using this should give me my rows of pixels. I also want to process rows in reverse so the image is the right way up, the `rev` method will return a reverse iterator. I need to know which row I'm processing, `enumerate` is an iterator which will return a tuple with the number of the row being iterated and the row itself.  I'm doing this in a `for_each` iterator which takes a closure. Inside the closure I'm using `chunks_mut` chained with `enumerate` again to iterate over each RGB pixel in the row and for each pixel I'm ray tracing `nx` samples, the same as the original loop. Finally I get an iterator to the RGB pixels and write out each channel. Phew!

```rust
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

It took me quite a while to work out the right combination of iterators to produce a nested loop that behaved similarly to the original. The iterator version of the code runs in ~37 seconds, very slightly faster than the original.

Now that I have this, calculating the rows in parallel is simply a case of changing the first `chunks_mut` to `par_chunks_mut` and...

# Data races

As I mentioned earlier, Rust detects data races at compile time and there were a couple in my code, this is the abridged version:

```
error[E0387]: cannot borrow data mutably in a captured outer variable in an `Fn` closure
  --> src/main.rs:95:41
   |
95 |                     let u = (i as f32 + rng.next_f32()) / nx as f32;
   |                                         ^^^
   |

error[E0387]: cannot borrow data mutably in a captured outer variable in an `Fn` closure
  --> src/main.rs:99:28
   |
99 |                     col += scene.ray_trace(&ray, 0, &mut rng);
   |                            ^^^^^
   |
```

I had added a counter to the `Scene` struct which is updated in calls to `ray_trace`, so this method takes `&mut self`. I made the counter a `std::sync::atomic::AtomicUsize`. The other race was the `rng` which was captured in the `for_each` closure of the main loop. I just construct a new `rng` for each row instead.

Once these issues were fixed it compiled and ran in ~10 seconds, nearly a 4x speed up which is what I would hope for on a 4 core machine. Although, I do have 8 logical cores my understanding is I probably wouldn't get any extra speed out of these, unfortunately my BIOS doesn't have an option to disable hyper threading.

After changing my loop to use iterators it was a one line change to my main loop to make it run in parallel using Rayon and thanks to Rust all the data races in my code failed to actually compile. That is much easier to deal with than data races happening at runtime.

I still have a lot of optimisations I want to try, primarily making my spheres [SOA](https://en.wikipedia.org/wiki/AOS_and_SOA#Structure_of_arrays) and trying out Rust's SIMD support.

The code for this post can be found on [github](https://github.com/bitshifter/pathtrace-rs/tree/2018-05-05-post).

If you have any feedback you can comment on [/r/rust](https://www.reddit.com/r/rust/comments/8hj03a/path_tracing_in_parallel_with_rayon/).
