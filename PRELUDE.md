## The Age of AI and What It Means for Developers

What a time to be alive. We're living in a world where AI is actively reshaping how we work, think, and behave—and like it or not, it's here to stay.

I've been a software engineer for over a decade, and watching our industry morph in real-time is a mix of astounding, terrifying, fascinating, and remarkable. Some see it as the end times; others view it as a fast track to utopia. The reality, I think, sits somewhere in the middle. AI will create entirely new opportunities and skill sets while simultaneously rendering other aspects of our work obsolete. But remember: this isn't humanity's first radical paradigm shift, and it won't be the last. We've handed off the loom, the calculator, the compiler, and the search engine. Each time, the people who panicked about the *tool* missed the point. The tool was never the job.

### The Backstory

Back in 2022, I started [gobc](https://github.com/duysqubix/gobc) as a side project. Like many who tumble down the emulation rabbit hole, I grew up with Game Boys. Fascinated by the hardware, looking to level up my skills, and wanting to learn Go in the process, I decided to take a stab at building an emulator.

It was a massive undertaking. It took a solid four to five weeks just to pass Matt Currie's `cgb-acid` test, and there were definitely moments where I wanted to pull my hair out. While `gobc` emulated full M-cycles and ran ROMs decently well, it was by no means hardware-accurate. Quirks were everywhere—timing edge cases I knew existed but couldn't justify the weeks it would take to chase down. It was a cool project, and I'm still proud of it, but it was fundamentally limited by the design choices I'd locked in early and the sheer number of hours I had to give it.

After `gobc`, I wanted to keep challenging myself. The plan was to write Game Boy emulators in different languages, sticking to the `<LANG>-bc` naming convention. I started `rubc`—the Rust one—a couple of years ago, but between work, life, and being a parent, it gathered dust like so many side projects do.

Fast forward to today, and I had a crazy idea.

### Handing Over the Keys

If you pitched this idea to the masses, you'd probably get laughed out of the room—because the prevailing narrative is always, *"AI just isn't there yet."*

Knowing that, I did exactly what would land me in social media jail: I wondered what would happen if I gave up the wheel entirely and handed my project over to an AI team. Not as some lazy vibe-coding exercise where you accept whatever the model spits out and pray it compiles, but to see what AI is *actually* capable of when given real autonomy and held to a real standard. I'd nudge it when it got stuck and provide missing context when it started spinning its wheels—but that was the extent of it. No writing code. No fixing its bugs for it. No quietly taking the wheel back when it got hard.

Years ago, a good friend told me something that stuck with me:

> "Programming is only an extension of the programmer's mind."

Programming is just the mechanical skill you master to translate thoughts into a machine. The *thinking* is the work; the coding is the bottleneck. A painter is limited by their physical command of the brush, not by the picture in their head. This has always aligned with my own philosophy about what separates someone who took a six-week Python bootcamp from a veteran who has been doing this for decades.

A 10+ year developer has built up wisdom. They've formed the synaptic connections that result in rapid pattern matching—what we usually just call "gut feelings." Knowing *how* software should run, *where* it'll break, and *what* to optimize is a hard-earned intuition that no amount of syntax memorization buys you. The junior and the senior can write the same `for` loop. What they can't both do is look at a subsystem and feel, in their bones, that the timing is going to be wrong three layers down.

So, what happens if we replace the standard paintbrush with an automatic one? We aren't replacing the artist's mind; we're just upgrading the tool they use to draw on the canvas. The vision, the taste, the judgment about what "done right" actually means—that still has to come from somewhere.

### The RUBC Experiment

That was the premise for `rubc`, and man, did it deliver.

The ground rules were simple. I already knew how to build an emulator architecturally, so I provided the vision: the build order, the rough shape of the CPU/PPU/APU split, the hard constraint of pure, `unsafe`-free Rust, and the non-negotiable that it be verified against the canonical hardware test ROMs rather than just "looking right." But I completely handed over control of the paintbrush, guiding only when strictly necessary.

`rubc` is the result of that experiment. It went from virtually nothing to ~99.9% hardware-accurate emulation for both DMG and CGB. It passes some of the most rigorous test ROMs available today—including Matt Currie's `cgb-acid-hell`, an undocumented PPU torture test that trips up a lot of well-known emulators—pixel for pixel.

What I found most striking wasn't just that it *worked*. It was *how* it worked. The AI scaffolded its own architecture, reasoned through genuinely subtle timing bugs, built itself ground-truth tooling (it compiled SameBoy from source to use as a hardware reference and diffed against it dot-by-dot), and—maybe most importantly—knew when to stop. When it hit the last cluster of mid-scanline timing edge cases, it didn't fake a passing test or quietly weaken a gate to claim victory. It diagnosed the root cause across a dozen failed attempts, consulted its `oracle` agent, concluded the honest fix required a deeper architectural rewrite than the experiment warranted, and *documented the limitation instead of lying about it.* That restraint is something I've watched human engineers fail at.

In just **three days**, the AI built an emulator that surpassed what I accomplished alone with `gobc` over many more weeks—higher accuracy, cleaner architecture, and a test suite it green-lit itself.

My involvement? I provided the documentation, a rough architectural suggestion, and instructed the AI to consult the `oracle`—a dedicated reasoning agent from the `oh-my-openagent` project—whenever it hit a wall. Beyond that, I explicitly asked to be left out of it.

### Don't Be the Next Blockbuster

Take it for what it's worth. You don't have to agree with my methods, and you're free to be skeptical—skepticism is healthy. But going from zero to a finished product that rivals today's heavyweights (SameBoy, BGB, Emulicious) in a ridiculously short amount of time is not a feat we can just hand-wave away.

Here is where I need to clear... The experiment didn't work because the AI is magic. It worked because the *vision* was already there. I knew what an emulator should look like, what "hardware-accurate" actually means, which tests were the ones that mattered, and what a cut corner smells like. Hand the same tools to someone who's never built one, and you don't get `rubc`—you get a confident pile of code that runs Tetris and silently mangles everything subtle. The AI was an extraordinary paintbrush. It was not the painter.

So whether you're a doomsday prepper, a tech utopian, or somewhere in between, the biggest question on everyone's mind is: *Will AI replace my job as a software developer?*

I'd argue we need to be more pedantic about it: **AI will replace whatever it is you are doing today.** If the entirety of your value is mechanical—translating a clear, already-solved problem into syntax—then yes, that part is going away, and quickly. But if your value is the judgment, the taste, the architectural intuition, and the ability to know what *correct* even looks like, then AI doesn't replace you. It hands you a paintbrush that moves at the speed of thought.

Adapt the way you work, or don't. But don't mistake the tool for the threat.

Don't be the next [Blockbuster](https://en.wikipedia.org/wiki/Blockbuster_(retailer)).
