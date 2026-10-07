# AWS controls on three hosts (spec 078 T-A16)

One demo form per AWS control drives its operations against the fake MCP
server and prints a summary block: the operations exercised, the refusals
checked, the calls made, the time the synchronous part took, the events seen
in order, and the `PASS n FAIL m` tally.

| Form | Control | Checks |
|---|---|---|
| `aws-lambda-demo.cfrm` | `AwsLambda` (Delivery A) | 9 |
| `aws-mcp-demo.cfrm` | `AwsMcp` (Delivery A) | 9 |
| `aws-knowledge-base-demo.cfrm` | `AwsKnowledgeBase` (Delivery B) | 8 |
| `aws-agent-core-demo.cfrm` | `AwsAgentCore` (Delivery B) | 6 |
| `aws-agent-memory-demo.cfrm` | `AwsAgentMemory` (Delivery B) | 7 |
| `aws-s3-tables-demo.cfrm` | `AwsS3Tables` (Delivery B) | 10 |
| `aws-glue-demo.cfrm` | `AwsGlue` (Delivery B) | 9 |

`forms/aws-shell.cfrm` is a main form with a SideMenu that loads any demo
into its ContentPane. The fake answers each Delivery B tool with the shape
that server's source produces (see the fixtures' provenance in
`crates/cobolt-runtime/tests/fixtures/aws-mcp/`).

`crates/cobolt-cli/tests/aws_hosts.rs` runs them under `rcrun run-form`, as
embedded child forms in the shell, and (with `--ignored`) in a built binary.
The generated programs are produced by the test, never committed: they are
what the designer would generate from these forms.
