<!-- powerrustcobol-kit: 1.80.100 -->
# The `.cidx` indexed-file definition

An indexed (ISAM) file is described by one XML file in `indexed/`, which the IDE's Indexed File
Editor reads and writes. From it PowerRustCOBOL generates `generated/<name>-indexed.cbl` and the
copybooks `COPYBOOKS/<name>.SEL` (the `SELECT`) and `COPYBOOKS/<name>.FD` (the `FD`). Write the
definition, call `validate`, `add_to_project` and `regenerate`; never edit what is generated.

## The `IndexedFile` element

The root. Attributes: `name` — the file's COBOL name (the `SELECT`/`FD` name, a COBOL word);
`finalized` — `true` once the developer has finalized it in the Indexed File Editor, which
creates the data file and locks the structure (write `false`); `version` — the format version,
`1.0`.

Its children, in this order:

- `assign-path` — the data file's path. A relative path starts at the application's folder
  (the project folder under Run Form), e.g. `data/customers.idx`.
- `access-mode` — `dynamic`, `sequential` or `random`.
- `record-format` — `fixed-length` (the record length in bytes), or `min` and `max` for a
  variable-length record.
- `storage` — `mode` (`disk` or `memory`), `compression` and `persistence` (`true`/`false`;
  `persistence` applies to `memory` only).
- `comment` — optional, CDATA: a description that becomes a comment above the `FD`.
- `keys` — the `primary` key and any `alternate` keys. Each has `duplicates`
  (`true`/`false`), `ordering` (`ascending` or `descending`) and, on an alternate, a `name`;
  each holds one or more `part` elements with `field` (a field of the record), `offset` and
  `length` (bytes from the record's start) and `encoding` (`bytes`, `display-ascii`,
  `display-utf8`, `ucs2-le`, `ucs2-be`, `utf32-le`, `utf32-be`, `packed-decimal`,
  `binary-be`, `binary-le`).
- `fields` — the record: one level-1 `Field` (the record group) holding its sub-fields.

## The `Field` element

Attributes: `level` (`1`, `5`, …), `name` (a COBOL word), `pic` (the PICTURE, without `PIC`),
`usage` (`display`, `comp`, `comp-3`, `comp-4`, `binary`, `packed-decimal`, `index`,
`pointer`), `offset` and `length` (bytes, on elementary fields), and optionally `occurs`,
`redefines` (a field above it), `synchronized` and `grid-control` (the control the IDE's grid
uses for the field). A group field holds its sub-fields as nested `Field` elements; a field may
hold a `comment` (CDATA), which trails it in the generated `FD`.

Offsets and lengths must agree with the PICTUREs: a `9(8)` field is 8 bytes, and the next
field starts where it ends. Every key `part` must name a field and use that field's offset and
length.

## A live example

Read and re-serialised by the same code the IDE saves definitions with (line breaks added
between elements; the IDE writes it on one line):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<IndexedFile name="CUSTOMER-FILE" finalized="false" version="1.0">
<assign-path>data/customers.idx</assign-path>
<access-mode>dynamic</access-mode>
<record-format fixed-length="56"/>
<storage mode="disk" compression="false" persistence="false"/>
<comment>
<![CDATA[One row per customer.]]>
</comment>
<keys>
<primary duplicates="false" ordering="ascending">
<part field="CUST-ID" offset="0" length="8" encoding="bytes"/>
</primary>
<alternate name="CUST-NAME" duplicates="true" ordering="ascending">
<part field="CUST-NAME" offset="8" length="40" encoding="bytes"/>
</alternate>
</keys>
<fields>
<Field level="1" name="CUSTOMER-RECORD" usage="display">
<Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8">
<comment>
<![CDATA[Primary key]]>
</comment>
</Field>
<Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40">
</Field>
<Field level="5" name="CUST-BALANCE" pic="9(6)V99" usage="display" offset="48" length="8">
</Field>
</Field>
</fields>
</IndexedFile>
```
