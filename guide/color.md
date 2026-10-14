## color (type)

```lua
---@type color number[] | integer[] | string
```

A color can be either an ununamed array of 1-4 floats or integers, or a hex string.

An array is red, green, blue, alpha, with 0 being nothing and 1 being max. Any omitted value defaults to 0 for rgb or 1 for alpha. If any entry is over 1, the whole array is read as 0–255 instead, so `{255, 128, 0}` works as you'd expect, `{255, 1, 0}` is red with a barely-there 1 of green, and `{1, 1, 1}` is still white. A missing alpha is opaque either way.

If you're more farmiliar with hex values (\*cough\* web developer), a string of 3-4 or 6-8 characters representing hex values can be used as well. No hash is needed, simply saying "fff" represents white, "f00" red, "0f0" green, etc. "0006" would be black at 50% alpha.
