---
layout: post
title:  "Performance is a feature"
excerpt_separator: <!--more-->
tags: performance profiling
---

How do you avoid premature optimisation?

Set targets

Hardware specs
Console is pretty easy
PC - minimum, recommended. Balance between possible customer base and lowest common denominator hardware (e.g. if min spec is 2 cores that's going to limit how much you can scale on high end machines)
Phones - I don't have any experience but I imagine it's a simiar process to setting PC targets

CPU Frame time
GPU Frame time
Not FPS - it's non linear which makes it harder to talk about gains and losses
Hitches - how many frames under target
Load time
Memory usage
Network bandwidth
Power consumption
Download size
What is important to your users?
Mean, Median, Standard deviation
etc.

Game dev but don't only care about game performance
Care about developer productivity
Development build performance
Compile time
Debug run time
Asset build time
Performance of tools

Track these in your game and record them somewhere
dump on exit
to a database
with a front end to see what's happening

What are we measuring?
Developer usage (not if debugger attached)
Crash handling is related and useful but not talking about here
Automation!

Rendering different camera positions in the world
Make a stress test level
Write test bots
Player replays - especially easy for multiplayer, just record and playback network traffic
Something repeatable and deterministic
Easy for a developer to run

Automate running these
If you have CI (you should have CI!) then run performance test for every build if possible
Always recording and monitoring metrics

The more of this you manage to have in place, the better 
So now you have a pretty good idea of how your game is performing. Do you need to profile?
There is more to this decisions, the above doesn't cover everything and maybe you're below target but don't have head room for more features or content. In any case, you should be better informed now.

Everyone should be responsible for performance. If there is a sudden drop after a commit, investigate why. Can it be improved or to target expectations need to be adjusted.

Performance is not just for experts. Yes when you are squeezing out 100% out of the hardware performance and optimisation get quite specialisted. But don't be discouraged, it's easy to make a start. If you've never profiled your game there's a good chance if you spend a short amount of time examining it in a profiler that you will find something slow. Unity and Unreal have built in profilers you can use. They're a good start. Not using an off the shelf engine? There are plenty of open source profilers that you can integrate into your game. That's probably the best starting point. Then look at external profiling tools.

If you are looking at something under target, keep digging into the biggest thing. Understand what the code is doing. Look for silly things or redundancies. Processing the same thing twice, or not using results are common. Keep digging.

Always measure your changes! Did it look better in the profiling. Microbenchmark - with caution. Run your performance test, did you see an improvement?

Sometimes performance problems are architecural. That does get harder to spot (death by 1000 cuts) and is harder to fix (need rearchitecting) but at least if you are always monitoring you will find out earlier that you have a problem.

This isn't expert stuff, it can be learned like anything else in development.
