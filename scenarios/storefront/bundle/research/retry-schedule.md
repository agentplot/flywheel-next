# Finding: a delayed retry is worth prototyping, and cannot be judged on paper

The research note settles that the existing retry is an immediate identical
repeat and recovers nothing. It cannot settle what a delayed retry would
recover here, because the recorded attempts do not carry the processor's
decline code and so cannot be sorted into the recoverable and the rest.

What would settle it: replay the fourteen days of recorded attempts against the
processor's sandbox, which does return decline codes, and measure what a
schedule of one hour and twenty-four hours would have recovered on soft
declines only.

This is throwaway work whose output is a number and a recommendation, not code
we keep. It wants a running page rather than a note, because the schedule has
parameters — the delays, the attempt cap, whether to retry across a card
expiry — and the useful form is one the operator can turn the dials on.

It needs a sandbox account on the processor, which we do not have.
