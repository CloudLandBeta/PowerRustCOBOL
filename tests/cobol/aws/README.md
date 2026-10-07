# AWS controls on three hosts (spec 078 T-A16)

`forms/aws-lambda-demo.cfrm` and `forms/aws-mcp-demo.cfrm` drive every
Delivery A operation of `AwsLambda` and `AwsMcp` against the fake MCP server
and print a summary block: the operations exercised, the calls made, the time
the synchronous part took, the events seen in order, and the `PASS n FAIL m`
tally. `forms/aws-shell.cfrm` is a main form with a SideMenu that loads either
demo into its ContentPane.

`crates/cobolt-cli/tests/aws_hosts.rs` runs them under `rcrun run-form`, as
embedded child forms in the shell, and (with `--ignored`) in a built binary.
The generated programs are produced by the test, never committed: they are
what the designer would generate from these forms.
