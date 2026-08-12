# leptos_read_then_replace_signals

## What it does

Checks for a writable signal that reads its current value to calculate the value passed back to
`set()`.

## Why is this bad?

Reading and then replacing reactive state separates one logical change into two operations. The
read may clone the stored value, and asynchronous or re-entrant code can make the replacement use
an older snapshot.

## Example

```rust,ignore
items.set({
    let mut next = items.get();
    next.push(item);
    next
});
```

## Use instead

Keep the change inside one bounded update:

```rust,ignore
items.update(|items| items.push(item));
```

The same applies to small scalar changes: `count.update(|count| *count += 1)` keeps the read and
write together. Direct replacement remains appropriate when the new value does not depend on the
signal's current value.
