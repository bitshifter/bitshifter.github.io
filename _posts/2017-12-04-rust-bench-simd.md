---
layout: post
title:  "Microbenching SIMD in Rust"
excerpt_separator: <!--more-->
tags: rust simd
---

Today I read [Hugo Tunius](https://twitter.com/K0nserv)' blog post [Exploring SIMD on Rust](https://hugotunius.se/2017/12/01/exploring-simd-in-rust.html), in which after some experimentation he didn't get the performance boost he expected to see from SIMD. I've also been meaning to have more of a play with SIMD so I thought I'd take a look at his [git repo](https://github.com/k0nserv/vector-benchmarks) and see if I can work out what's going on.

<!--more-->

Hugo mentioned he was having trouble with [Bencher](https://doc.rust-lang.org/1.1.0/test/struct.Bencher.html), so let's start there. Running `cargo bench` gave these results

```
running 4 tests
test bench_f32            ... bench:         151 ns/iter (+/- 2)
test bench_f32_sse        ... bench:     162,004 ns/iter (+/- 212)
test bench_f32_sse_inline ... bench:         150 ns/iter (+/- 11)
test bench_f64            ... bench:         150 ns/iter (+/- 3)
```

Something is very wrong here. One SSE benchmark is orders of magnitude slower than all of the other benchmarks. That doesn't make much sense - time to look at some assembly.

To generate assembly via cargo you can run specify `RUSTFLAGS=--emit asm` before running `cargo bench`. Since we're only interested in the assembly output for now I'm running

```
RUSTFLAGS="--emit asm" cargo bench --no-run
```

This generates some AT&T style assembly output to `.s` files in the `target/release/deps` directory.  Ideally I'd prefer Intel format with demangled symbols but I don't think `rustc --emit` gives any control over this.

Comparing the assembly from `bench_f32` and `bench_f32_sse` there's some clear differences. All of the work happens between bencher functions that record the start and end times - the mangled calls to [`_ZN3std4time7Instant3now17he141c6f08d993cf9E@PLT`](https://doc.rust-lang.org/std/time/struct.Instant.html#method.now) and [`_ZN3std4time7Instant7elapsed17hc3711b876336edbcE@PLT`](https://doc.rust-lang.org/std/time/struct.Instant.html#method.elapsed). This is the assembly for `bench_f32`

```
	movq	%rdi, %r14
	callq	_ZN3std4time7Instant3now17he141c6f08d993cf9E@PLT
	movq	%rax, 8(%rsp)
	movq	%rdx, 16(%rsp)
	movq	(%r14), %rbx
	testq	%rbx, %rbx
	je	.LBB15_2
	.p2align	4, 0x90
.LBB15_1:
	callq	_ZN7vector314num_iterations17h62f9e330173945acE
	decq	%rbx
	jne	.LBB15_1
.LBB15_2:
	leaq	8(%rsp), %rdi
	callq	_ZN3std4time7Instant7elapsed17hc3711b876336edbcE@PLT
	movq	%rax, 8(%r14)
	movl	%edx, 16(%r14)
```

And the assembly of `bench_f32_sse`


```
	movq	%rdi, %r14
	callq	_ZN3std4time7Instant3now17he141c6f08d993cf9E@PLT
	movq	%rax, 8(%rsp)
	movq	%rdx, 16(%rsp)
	movq	(%r14), %r15
	testq	%r15, %r15
	je	.LBB16_4
	xorl	%r12d, %r12d
	.p2align	4, 0x90
.LBB16_2:
	incq	%r12
	callq	_ZN7vector314num_iterations17h62f9e330173945acE
	movq	%rax, %rbx
	testq	%rbx, %rbx
	je	.LBB16_3
	.p2align	4, 0x90
.LBB16_5:
	movaps	.LCPI16_0(%rip), %xmm0
	movaps	.LCPI16_1(%rip), %xmm1
	callq	_ZN17vector_benchmarks7dot_sse17hc439918e0ab4bdcfE@PLT
	decq	%rbx
	jne	.LBB16_5
.LBB16_3:
	cmpq	%r15, %r12
	jne	.LBB16_2
.LBB16_4:
	leaq	8(%rsp), %rdi
	callq	_ZN3std4time7Instant7elapsed17hc3711b876336edbcE@PLT
	movq	%rax, 8(%r14)
	movl	%edx, 16(%r14)
```

Without going through everything that's going on in those listings one is obviously a lot longer than the other, but why? In the first listing there should be a bunch of math happening between the two calls to `_ZN3std4time7Instant7elapsed17hebf7091c45b5403bE` to calculate a dot product, but there is only a call to `num_iterations`. The second listing has a call to the mangled function name for `dot_sse`. The compiler seems to be optimizing away all of the `bench_f32*` code except for the function call to `dot_sse`.

Let's look at the Rust code for `bench_f32`

```rust
fn bench_f32(b: &mut Bencher) {
    b.iter(|| {
               let a: Vector3<f32> = Vector3::new(23.2, 39.1, 21.0);
               let b: Vector3<f32> = Vector3::new(-5.2, 0.1, 13.4);

               (0..num_iterations()).fold(0.0, |acc, i| acc + a.dot(&b));
           });
}
```

The problem here is the perennial problem with micro-benchmarking suites - is my code actually being run. What is happening is the result of the `fold` call is discarded. Rust returns the value of the last expression if it's not followed by a semi-colon. Because the `fold` ends with a semi-colon, the closure returns nothing, well `()` to be precise. To fix this and stop the optimizer from very rightly removing unnecessary work we need to either add an explicit return statement or remove the semi-colon following the `fold`.

Returning from all of the `fold` calls results in a compile error

```
LLVM ERROR: Cannot select: intrinsic %llvm.x86.sse41.dpps
```

The compiler is now trying to generate code for `bench_f32_sse_inline` but the intrinsic is not available. To make sure SSE4.1 is available I created a `.cargo/config` file containing the following

```
[target.'cfg(any(target_arch = "x86", target_arch = "x86_64"))']
rustflags = ["-C", "target-cpu=native", "-C", "target-feature=+sse4.1"]
```

I have no idea if this is the "correct" way to enable SSE4.1, but it compiles.

The bench results now look like

```
running 4 tests
test bench_f32            ... bench:      90,383 ns/iter (+/- 2,185)
test bench_f32_sse        ... bench:     486,344 ns/iter (+/- 12,059)
test bench_f32_sse_inline ... bench:      89,788 ns/iter (+/- 3,894)
test bench_f64            ... bench:      89,237 ns/iter (+/- 1,299)
```

Again examining the assembly output, `bench_f32_sse` is still slower because `dot_sse` is not getting inlined. Let's add `#[inline(always)]` on all the `dot*` functions just do be sure.

```
running 4 tests
test bench_f32            ... bench:      89,652 ns/iter (+/- 1,113)
test bench_f32_sse        ... bench:      89,843 ns/iter (+/- 1,160)
test bench_f32_sse_inline ... bench:      89,648 ns/iter (+/- 1,428)
test bench_f64            ... bench:      89,794 ns/iter (+/- 1,311)
```

Now we're getting consistent looking results, but the SSE functions are still not noticeably faster than the scalar code. Time to reexamine the `bench_f32_sse` assembly yet again. So it's definitely inlined now and we're seeing our vector dot product

```
vmovaps	.LCPI16_0(%rip), %xmm0
vdpps	$113, .LCPI16_1(%rip), %xmm0, %xmm1
```

But below our dot product there's a lot of `vaddss` calls

```
.LBB16_2:
	addq	$1, %rbx
	callq	_ZN7vector314num_iterations17h62f9e330173945acE
	testq	%rax, %rax
	je	.LBB16_3
	leaq	-1(%rax), %rcx
	movq	%rax, %rsi
	vxorps	%xmm0, %xmm0, %xmm0
	xorl	%edx, %edx
	andq	$7, %rsi
	je	.LBB16_5
	vmovaps	16(%rsp), %xmm1
	.p2align	4, 0x90
.LBB16_7:
	addq	$1, %rdx
	vaddss	%xmm0, %xmm1, %xmm0
	cmpq	%rdx, %rsi
	jne	.LBB16_7
	jmp	.LBB16_8
	.p2align	4, 0x90
.LBB16_3:
	vxorps	%xmm0, %xmm0, %xmm0
	jmp	.LBB16_11
	.p2align	4, 0x90
.LBB16_5:
	vmovaps	16(%rsp), %xmm1
.LBB16_8:
	cmpq	$7, %rcx
	jb	.LBB16_11
	subq	%rdx, %rax
	.p2align	4, 0x90
.LBB16_10:
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	vaddss	%xmm0, %xmm1, %xmm0
	addq	$-8, %rax
	jne	.LBB16_10
```

So what's happened here? Yet again the optimizer is being clever. It realizes that the dot product calculation is invariant, so it's moved it out of the loop. The `fold` now consists of a loop summing `num_iterations` of the dot product. Even this has been loop unrolled by the compiler, which is why there are so many `vaddss`.

# In Conclusion

Micro-benchmarks are hard. Even when they're testing the right thing they can still be wrong due to other influences like other software on the system or different CPU pressures than when the code is run in a real program.

In this case we found that the code that was expected to be benchmarked wasn't being run at all in most cases. Firstly due to the small omission of a return value and a clever optimizing compiler. When the return was fixed and it was run the dot code was only executed once rather than the expected `num_iterations` time because again the compiler was smart enough to optimize away the loop invariant.

We still haven't answered the question is SSE faster but we have at least determined why it doesn't appear to be in this benchmark. The way I'd go about that would be to generate a `Vec` of data to dot product and fold on that, rather than an invariant.
