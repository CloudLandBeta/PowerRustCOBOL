# TextBox — control test example

Subject control: **TextBox**. The form fires a console `DISPLAY "<Event> working"`
for every supported event, and each button changes one property of the control
from COBOL via `INVOKE SUBJ "SetProperty" USING "<name>" "<value>"`.

## Build

```sh
rcrun build examples/text-box/cobolt.toml
```

Or open `cobolt.toml` in the IDE and use **Build**. If the build reports an
error, read it, fix the form handler, regenerate, and rebuild — it must build
with zero errors.

> `generated/text-box.cbl` is codegen output (regenerable from the form);
> the COBOL lives in the form's event handlers, not hand-edited.
