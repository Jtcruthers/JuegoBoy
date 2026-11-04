
## Ticking

Originally, I had each opcode return the number of M-cycles it used, and I'd try to step through each of the sub-processor's that many M-cycles.
I think I was hitting issues with timing and interrupts because the subprocessors were waiting for long CPU opcodes to finish.
The CPU could expect things to happen in between that time, or an interrupt flag could be set (I think).

To be more accurate, I've changed the MMU to have a tick function that happens every M-cycle.
Everytime the CPU reads or writes to the MMU, it will call MMU's tick function,
since each read/write is a single M-cycle.
However, the CPU does do some things that require extra ticks without read/writes to the MMU,
so it will keep track of the extra cycles and call tick manually.

This allows for timers and things to sync during a single operation.

Note that the MMU cannot call tick for every read/write, since other subprocessors also read/write memory.
We let the CPU drive the master clock and only call tick when the CPU ticks.

However, this means that the other subprocessors can only run one M-cycle at a time.
I will need to figure out a way to have them be able to run on a per-M-cycle basis.

### Ticking Guidelines
* In the doc comments, functions say how many M-cycles they take to do their function.
This number does NOT include the 1 M-cycle for fetching the opcode.
So the function that does the work for `POP r16`'s comment says it takes 2 M-cycles,
even though `POP r16` and `POP AF` both take 3 cycles.
The number of M-cycles they proclaim to take is how many individual actions happen within it.
Opcode fetching happens at the start of the CPU's step, so it doesn't get included for simplicity.

However, I'm not sure where some of these things tick:

### Issues with Conditional timing

`RET cc` takes 5 M-Cycles if condition is met, else 2.
The fetching of the opcode takes 1 cycle, so it's really 4 if met, else 1.

I assume that since regardless of meeting the condition on not a cycle is still taken, checking the condition takes 1 cycle.
If the condition is met, it then pop's which takes 2, and then sets it.
Then it needs 1 cycle to set the jump.
So we have:

* Condition met
    * 1 for fetching opcode
    * 1 for condition check
    * 2 for pop
    * 1 for setting `pc`
* Condition not met
    * 1 for fetching opcode
    * 1 for condition check

This makes sense, as non-conditional `RET` always takes 4. Drop the one conditional check, and there you go.

However, The jumps seem different:

`JR cc,n16` takes 3 M-cycles if condition is met, otherwise 2.
That would be 1 for opcode, 1 for condition check, and if conditon is true, 1 to set the `pc` register.

But the non-conditional `JR n16` also takes 3 M-cycles.

In the conditional case, since non-conditional takes 1 less cycle, you'd assume that would be for setting the `pc` register.
It still takes two cycles, so it must check take a cycle to check the conditional.
But that means that `JR cc` when the conditional is true takes the same amount of cycles as `JR n16`, which doesn't check a conditional at all. 
The true path for `JR cc` is the same as the default path as `JR` non-conditional, so the condition check must not take any cycles at all.

This begs the question: where do I put the manual tick?
If it's the conditional check, that's easy put it at the start of the function.
But it doesn't seem to be the condition set, so it must be the setting of the `pc` register.
That's the only difference between a the two paths for `JR` opcodes that actually jump, and the `JR cc` non-true conditional.

But if it's just setting the `pc` register, then why don't `RET` and `RET cc` take the same amount of time when the conditional is true?

This doesn't seem to make sense (yet).
