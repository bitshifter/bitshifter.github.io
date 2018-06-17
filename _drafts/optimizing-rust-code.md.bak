# Some Notes on Profiling and Optimizing Rust

I've been blogging recently about writing a simple path tracer in Rust. This was mostly inspired by @aras_p's blog posts on the same topic. Aras had been measuring his performance in terms of millions of rays processed per second. His numbers were orders of magnitude larger than what I was seeing  - I expected my unoptimised code to be slower but not that much slower. The problem with comparing this way was my scene was completely different, so since I was using his performance numbers as a target I've also started using his scene layout. My scene is not completely the same though, I haven't added emissive lights but at least the numbers are somewhat comparable now. Mine is in the order of 18Mray/s and on my machine his SIMD implementation was about 24Mray/s (I think, need to recheck next time I'm in Windows land). So, I have a target to work towards at least.

# Profiling Rust

Profilers require debugging symbol information to tell you anything useful. Because Rust support common debugging symbol formats every standard C/C++ profiler I've tried on Windows and Linux has worked just fine with my Rust code. The only downside is most profilers don't know about Rust's symbol name mangling so function names look a bit funny but they're perfectly recongisable.

To get debug symbols in your release build just add the following to your `Cargo.toml`:

```yaml
[profile.release]
debug = true
```

On Windows I've tried out Visual Studio 2017's built in [CPU profiler](https://docs.microsoft.com/en-gb/visualstudio/profiling/beginners-guide-to-performance-profiling) and Intel's [VTune](https://software.intel.com/en-us/intel-vtune-amplifier-xe) which now has a free 30 day trial. The Visual Studio profiler is simple and easy to use, VTune is extremely powerful and lets you look at intricate detail about how your program is (or isn't) utilising your hardware.

On Linux I've been primarily using Valgrind's [callgrind](http://valgrind.org/docs/manual/cl-manual.html) tool in conjunction with [kcachegrind](https://kcachegrind.github.io/html/Home.html) which is pretty easy to use. I've been meaning to try out Linux's [perf](https://perf.wiki.kernel.org/index.php/Main_Page) tool but I'm not very familiar with it yet. Intel's VTune should also support Linux but their website seems to not want to let me sign in or register any more so I haven't been able to download it.

# Inlining: Traits vs Generics

When looking at the assembly in the progiler I noticed is my random number generator was also not being inlined. The path tracer uses a lot of randomness. I'm using the `XorShiftRng` from the `rand` crate. The `next_f32` function is pretty small and should be a candidate for inlining but it wasn't happening. 

I had been passing the RNG around as a parameter to avoid constructing it everytime I needed one since they're used so frequently. The other reason they're passed around rather than using a static version or have my scene own one is so that they aren't shared between threads.

For example my `Scene` struct had the method `ray_trace` which looks like so:

```rust
pub fn ray_trace(&self, ray_in: &Ray, depth: u32, rng: &mut Rng) -> Vec3
```

and then deeper inside `ray_trace` eventually eventually `random_in_unit_sphere` is called which makes a bunch of `next_f32` calls.

```rust
pub fn random_in_unit_sphere(rng: &mut Rng) -> Vec3 {
    loop {
        let p = vec3(
            2.0 * rng.next_f32() - 1.0,
            2.0 * rng.next_f32() - 1.0,
            2.0 * rng.next_f32() - 1.0,
        );
        if p.length_squared() < 1.0 {
            return p;
        }
    }
}
```

Traits are a bit like virtual interfaces in C++ but they are not exactly the same. If the concrete type is known a method call in a trait will be statically dispatched as opposed to a dynamically like a virtual function would. Because the call is statically dispatched you might expect the compiler could inline calls to trait functions but this is not always the case. Trait function calls won't be inlined across crate boundaries unless you use link time optimisation (LTO). So great, enable LTO right? My build time went from about 1s for a small change to 17s with LTO enabled so it's not ideal.

Funtion that takes trait parameter and calls function on trait. Trait lives in another crate. If that function is called with a concrete type it is inlined, if it calling code only has a trait it is not. LTO means it's always inlined. If you want inlining without LTO use generic funcion instead of trait parameter.

The main difference between generics and traits is generics will definitely be resolved at compile time. Traits may or may not be depending on how they are used. There are pros and cons - if you want inlining, used generics but if you are concerned about code size use traits.

# Floats

32 bit floats are still the defacto floating point unit used in games.

Floats are difficult because they don't implement the Ord trait because they could be NaN and NaN != Nan. Means they don't work with containers and algorithms that need Ord. In C/C++ you can compile with -ffast-math which sacrifices accuracy for speed. There is no equivalent in Rust. There are intrinsics which will perform the fast float operations but they are only in nightly and they mean you need to wrap the float type.

More discussion https://internals.rust-lang.org/t/pre-rfc-whats-the-best-way-to-implement-ffast-math/5740 and https://github.com/rust-lang/rust/issues/21690

Other guides:

https://gist.github.com/jFransham/369a86eff00e5f280ed25121454acec1

# Disassmbling

Install https://github.com/luser/rustfilt.

```
objdump -S target/release/exename | rustfilt | less
```

It's not pretty but it works. Would be good to be able to specify a symbol to disassemble.

# Profiling

Valgrind to record and kcachegrind to view is easy on Linux.

```
valgrind --tool=callgrind --dump-instr=yes
```

Each source line annotated and you can view instuctions. Execution is slow though.

For cache and branch prediction information

```
valgrind --tool=callgrind --dump-instr=yes --cache-sim=yes --branch-sim=yes
```
