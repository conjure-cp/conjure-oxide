[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 10/02/2026)

# Onboarding
Congratulations you are joining a very exciting project, pushing forward the field of Constraint Programming. 
However, there is a lot to learn before you are up to speed. Expect to spend the first few weeks on the project learning, and so do not rush starting to contribute right away.

Here are some tips for starting out.

## What You Need to Learn
The Conjure-Oxide project fundamentally involves the development of a constraint modelling tool in the Rust programming language. Therefore, learning about constraint programming and Rust will be your initial challenges.
Getting a solid understanding of Rust and constraints now will stand you in good stead for the rest of the project so do not worry that spending time learning is not productive, this learning is expected.

### Constraint Programming
[Constraint programming](https://en.wikipedia.org/wiki/Constraint_programming) is a paradigm for solving combinatorial problems. Instead of defining an algorithm to solve a complex problem, we instead model it as a constraint satisfaction problem (CSP), and let one of many optimised solver programs solve it from there.

Whilst not essential to be a fully-fledged constraint programmer it is useful to understand roughly how this works. There are good video lectures online that would be worth a watch for a general understanding, such as the [CP Summer School lectures](http://www.youtube.com/playlist?list=PLcByDTr7vRTYJ2s6DL-3bzjGwtQif33y3).

Conjure-Oxide uses the input language Essence. Getting an understanding of how to code in Essence will be really useful when it comes to debugging and testing anything. Reading through [Essence's documentation](https://conjure.readthedocs.io/en/latest/essence.html) will be useful for this. Another very useful method of learning Essence and some aspects of constraint programming is to explore the selection of example [Jupyter notebooks](http://www.github.com/ozgurakgun/notebooks).

Conjure-Oxide is built to fulfil a similar role to Conjure. Conjure is the current constraint modelling tool built in Haskell, which we are basing a lot of our implementation around. Understanding a little bit about [Conjure](https://conjure.readthedocs.io/en/latest/welcome.html) will be useful for you, as you may find yourself needing to refer to Conjure's source code later in your project.

### Rust
If you do not currently know how to program in Rust, this is very important to learn to contribute to most parts of the project.
The best resource for learning Rust is [The Rust Programming Language book](https://doc.rust-lang.org/book/), alongside the [Rustlings](https://rustlings.rust-lang.org/) resource. Rustlings will give you exercises to practice what you learned in the book as you go. If you find the book slow-going there are also great [video tutorials](https://www.youtube.com/playlist?list=PLai5B987bZ9CoVR-QEIN9foz4QCJ0H2Y8) available on YouTube that cover the book's content. Whilst both resources are fantastic, you will most likely not find it necessary to complete them in their entirety. However, I would recommend you complete the Rustlings exercise `19_smart_pointers` to understand concepts such as `Box` and `Cow`, as these are used heavily throughout Conure-Oxide's codebase.

## Understanding the Codebase
Once you have enough of an understanding to read Rust code you should start reading through Conjure-Oxide's codebase. This is a large complex codebase, and it is likely that it will not all make sense initially. A good method of understanding the codebase is to follow the control flow of the program and try to understand why each step occurs. There will be areas of the code you do not need to understand the inner-workings of as a new user, and so gaining a full holistic understanding from the outset is not necessary. Instead focus on getting a good general understanding of Conjure-Oxide as a whole, including what each step in process does and where in the codebase it is located.

This is a fundamentally cooperative project, and so do not be afraid to ask for help from a more experienced member of the team if you are struggling to understand an important part of the code. Feel free to reach out through our [GitHub Dicussions page](https://github.com/conjure-cp/conjure-oxide/discussions) for help whenever you need it.

This stage will take a while, but understanding the core of the program will be tremendously helpful when you start implementing it.

## Selecting a Project
Once you have spent time learning you will feel ready to select a project and begin contributing. There are lots that can be done in Conjure-Oxide, so if you have an idea of a project that is great, but also consider asking on our [GitHub Dicussions page](https://github.com/conjure-cp/conjure-oxide/discussions) for ideas, because there are many tasks new starts could reasonably achieve. Do not feel rushed into selecting a project. Having a good understanding of the system before you start writing code, will really facilitate your productivity later in the semester.

### Selecting a Starter Project
Whilst you are familiarising yourself with the codebase, you may want to try tackling a couple smaller starter projects. There are always little issues in the codebase needing addressed and working on one of these, while you learn, can be a fantastic way to get a confidence boost from feeling productive. These issues are specifically marked with `good first issue` on GitHub. However, be aware that taking on a wide project with a lot of reach, even if small, can be very challenging for a new start, as it requires deeply understanding many different areas of the code.

### Selecting a Project Early to Help Learn
If the scale of the codebase is feeling overwhelming, then consider talking to the project lead [@ozgurakgun](https://github.com/ozgurakgun) and selecting a project earlier on. The project would not need to be well defined at this point but even if you are not initially implementing features, having a project in mind can help focus your learning of the codebase. This narrows the scope of what you try to understand and helps make the codebase less overwhelming. However, you must ensure you still a good general understanding of Conjure-Oxide as a whole, or this will make expanding your project harder in the future.

## The Most Important Things
You should realise that it is okay to spend time learning and not contributing, and you should not feel bad, it is expected. If you get a good understanding now it will help you in the future.

Also ensure to ask for help and talk to the rest of the team, using the [GitHub Discussions](https://github.com/conjure-cp/conjure-oxide/discussions). There will be people on the project who have done it for years now. Use their expertise.