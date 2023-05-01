### color (type)

```lua
---@type color number[] | integer[] | string
```

A color can be either an ununamed array of 1-4 floats or integers, or a hex string.

A float array assumes red, green, blue, alpha is passed with 0. being nothing and 1 being max. Any ommited value defaults to 0. for rgb or 1. for alpha. If integers are used instead they operate on the assumption 0 is off and 255 is the maximum value. Keep in mind then that an array of {1,1,1,1} is a different shade then {1.,1.,1.,1.}.

If you're more farmiliar with hex values (\*cough\* web developer), a string of 3-4 or 6-8 characters representing hex values can be used as well. No hash is needed, simply saying "fff" represents white, "f00" red, "0f0" green, etc. "0006" would be black at 50% alpha.
