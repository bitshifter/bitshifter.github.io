---
layout: post
title:  "Microbenching SIMD in Rust"
date:   2017-12-04 19:03:02 +1000
categories: blog
---

Today I read [Hugo Tunius](https://twitter.com/K0nserv)' blog post
[Exploring SIMD on Rust](https://hugotunius.se/2017/12/01/exploring-simd-in-rust.html), in which
after some experimentation he didn't get the performance boost he expected to see from SIMD. I've
also been meaning to have more of a play with SIMD so I thought I'd take a look at his
[git repo](https://github.com/k0nserv/vector-benchmarks).

Huge mentioned he was having trouble with Bencher, so let's start there.

So running `cargo bench` gave these results:

```
running 4 tests
test bench_f32            ... bench:       3,001 ns/iter (+/- 406)
test bench_f32_sse        ... bench: 297,489,793 ns/iter (+/- 5,615,941)
test bench_f32_sse_inline ... bench:       2,949 ns/iter (+/- 301)
test bench_f64            ... bench:       2,982 ns/iter (+/- 426)
```

OK, something is very wrong here. One sse benchmark is orders of magnitude slower than all of the
others. That doesn't make much sense. Time to look at some assembly.

To generate assemby via cargo you can run specify `RUSTFLAGS=--emit asm` before running
`cargo bench`. Since I'm only interested in the assembly output I'm running

```
RUSTFLAGS=--emit asm cargo bench --no-run`
```

This generates some AT&T style assembly output to `.s` files in the `target/release/deps` directory.
Ideally I'd prefer Intel format with demangled symbols but I don't think `rustc --emit` gives any
control over this.

Comparing the assembly from `bench_f32` and `bench_f32_sse` it's pretty clear what the difference
is. All of the work happens between bencher functions that record the start and end times, this is
the assembly for `bench_f32`:

```asm
	movq	%rcx, %rsi
	callq	_ZN3std4time7Instant3now17h5e80e4f6430f8677E
	movq	%rax, 32(%rsp)
	movq	(%rsi), %rdi
	testq	%rdi, %rdi
	je	.LBB15_2
	.p2align	4, 0x90
.LBB15_1:
	callq	_ZN7vector314num_iterations17h5defcb85d46ddaceE
	addq	$-1, %rdi
	jne	.LBB15_1
.LBB15_2:
	leaq	32(%rsp), %rcx
	callq	_ZN3std4time7Instant7elapsed17hebf7091c45b5403bE
	movq	%rax, 8(%rsi)
	movl	%edx, 16(%rsi)
	addq	$40, %rsp
	popq	%rdi
	popq	%rsi
	retq
```

and the assembly of `bench_f32_sse`:

```
	movq	%rcx, %r14
	callq	_ZN3std4time7Instant3now17h5e80e4f6430f8677E
	movq	%rax, 40(%rsp)
	movq	(%r14), %r15
	testq	%r15, %r15
	je	.LBB16_4
	xorl	%r12d, %r12d
	vmovaps	__xmm@0000000041a80000421c666641b9999a(%rip), %xmm6
	vmovaps	__xmm@00000000415666663dcccccdc0a66666(%rip), %xmm7
	leaq	64(%rsp), %rdi
	leaq	48(%rsp), %rbx
	.p2align	4, 0x90
.LBB16_2:
	addq	$1, %r12
	callq	_ZN7vector314num_iterations17h5defcb85d46ddaceE
	movq	%rax, %rsi
	testq	%rsi, %rsi
	je	.LBB16_3
	.p2align	4, 0x90
.LBB16_5:
	vmovaps	%xmm6, 64(%rsp)
	vmovaps	%xmm7, 48(%rsp)
	movq	%rdi, %rcx
	movq	%rbx, %rdx
	callq	_ZN17vector_benchmarks7dot_sse17h9d9212672ec3e2b3E
	addq	$-1, %rsi
	jne	.LBB16_5
.LBB16_3:
	cmpq	%r15, %r12
	jne	.LBB16_2
.LBB16_4:
	leaq	40(%rsp), %rcx
	callq	_ZN3std4time7Instant7elapsed17hebf7091c45b5403bE
	movq	%rax, 8(%r14)
	movl	%edx, 16(%r14)
	vmovaps	80(%rsp), %xmm6
	vmovaps	96(%rsp), %xmm7
	addq	$120, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	popq	%r12
	popq	%r14
	popq	%r15
	retq
```

Without going through everything that's going on in those listings it's pretty clear that one is a
lot longer than the other. But why? In the first listing there should be a bunch of math happening
between the two calls to `_ZN3std4time7Instant7elapsed17hebf7091c45b5403bE`, but there is only a
call to `num_iterations`.
