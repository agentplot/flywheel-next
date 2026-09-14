# Why cards decline at checkout

The question: the declined-payment rate at checkout rose from 1.1 per cent to
4.2 per cent over eight days. What is declining, who is declining it, and is
any of it ours to fix?

## What was measured

Fourteen days of authorization attempts from the `payments` service log, joined
to the checkout session that made them. 41,900 attempts, 1,180 declines. The
processor's response body is not stored, so the join is on what the service
does record: amount, currency, card brand, issuing country, attempt ordinal and
the boolean the service writes as `declined`.

That boolean is the whole of what we keep, and it is the first finding rather
than a note on method. A decline that the issuer will approve on a second
attempt an hour later and a decline that says the card is closed are the same
row in our data. Everything below is inferred around that hole.

## What is declining

The rise is not spread evenly. It is one shape:

- **American Express is 9 per cent of attempts and 38 per cent of declines.**
  Before the eighth of August it was 9 per cent of attempts and 11 per cent of
  declines. Nothing about the amex traffic changed — same amounts, same
  countries, same hours.
- **The declines cluster in a band of amounts**, 40 to 120 in the store's
  currency, which is where most of the catalogue sits. There is no cluster at
  the high end, which is where a fraud rule would put one.
- **The second attempt declines with the first**, 96 per cent of the time,
  inside 900 milliseconds of it.

An issuer that has decided a card is closed does not change its mind in 900
milliseconds, and neither does one that has decided a transaction is
fraudulent. A rate that moves for one brand alone, on unchanged traffic, is
the issuer's risk model moving, not the shopper's behaviour. Amex re-tuned
something and did not tell us; that is ordinary and it is not a thing we can
appeal.

## Why we cannot tell the two kinds apart

The card networks separate *soft* declines — try again, the issuer may say yes
— from *hard* declines — do not try again, the answer will not change. The
separation is carried in the processor's decline code, two characters, present
on every response we receive.

`payments/src/processor/client.rs` reads the response, maps anything that is
not an approval to `Declined`, and drops the body. The code has never been
persisted. So:

- We cannot count how many of the 1,180 declines were soft.
- We cannot retry only the ones worth retrying, because we cannot see which
  they are.
- We cannot tell whether last week's rise is soft or hard, which is the
  question that decides whether any of this is recoverable.

Recording the code is not a fix for the decline rate. It is the precondition
for knowing whether there is anything to fix, and it is two fields.

## Why the retry does not help

`payments` retries once. The retry is issued from the same call frame, with the
same idempotency key, the same amount and the same card, on the same
connection, as soon as the first response is parsed — a mean of 340
milliseconds later.

An immediate repeat of an identical request is not a retry. It is the same
question asked twice in the same breath, and the issuer's risk decision has not
had time to be anything else. The 96 per cent figure above is that, measured.

The industry answer is a delayed retry with backoff, typically at one hour and
twenty-four hours, on soft declines only. Whether it recovers anything on *our*
traffic is not something this note can settle, because we cannot yet see which
declines are soft.

## Cart abandonment

Abandonment rose 12 per cent week on week and began the same day. 71 per cent
of the abandoned carts contain a declined attempt. That is correlation observed
once and it is enough to say the two are the same event, not enough to size
what recovering the declines would recover.

## What this settles, and what it does not

Settled:

1. The rise is concentrated in one brand on unchanged traffic, so the cause is
   outside the shop.
2. The decline code is discarded at the processor boundary, so soft and hard
   declines are indistinguishable in everything we hold.
3. The existing retry is an immediate identical repeat and cannot recover a
   soft decline.

Not settled, and not settleable without the code:

- How much of the 4.2 per cent is recoverable at all.
- What retry schedule recovers it, and what that schedule costs in processor
  fees and in shopper confusion when a charge lands an hour after checkout.

The next piece of work is a prototype: replay the recorded attempts against a
sandbox processor that does return codes, and measure what a delayed retry
would have recovered. It answers the second question and it is throwaway.
