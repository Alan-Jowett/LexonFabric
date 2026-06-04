# LexonFabric Scale Test Requirements

## Document Status

- **Phase:** Phase 2 - Specification Changes
- **Status:** Approved requirements patch being propagated into design and validation
- **Scope:** `lexonfabric-scale-test` local stress-test wrapper for rsync-backed mailbox acquisition and delegated block-tree generation

## USER-REQUEST

- **UR-SCALE-1 [KNOWN]:** Add a spec trifecta for a separate local wrapper/test tool under `docs/specs/lexonfabric-scale-test/{requirements|design|validation}.md`.
- **UR-SCALE-2 [KNOWN]:** The tool is not part of `lexonfabric-indexer`; it is built on top of existing LexonFabric components.
- **UR-SCALE-3 [KNOWN]:** The tool may be named `lexonfabric-scale-test`.
- **UR-SCALE-4 [KNOWN]:** The tool accepts one or more rsync URLs as input.
- **UR-SCALE-5 [KNOWN]:** The tool runs a Docker Compose-style local workflow for large-scale testing.
- **UR-SCALE-6 [KNOWN]:** The workflow stages are: fetch mailbox archives from rsync, discover mailboxes, generate an indexer request/config from the discovered mailboxes, then run the existing indexer/parser flow.
- **UR-SCALE-7 [KNOWN]:** The purpose is to stress test the LexonFabric parser and produce a block tree.
- **UR-SCALE-8 [KNOWN]:** The output must include a root block or root-id handoff artifact representing the produced block tree.
- **UR-SCALE-9 [KNOWN]:** MCP config generation is out of scope for this increment.
- **UR-SCALE-10 [KNOWN]:** This increment is for large-scale local testing only.
- **UR-SCALE-11 [KNOWN]:** Production orchestration will use ARM/Bicep plus Azure Functions and is out of scope here.
- **UR-SCALE-12 [KNOWN]:** This tool could be as simple as a Linux bash script.
- **UR-SCALE-13 [INFERRED]:** `lexonfabric-scale-test` should reuse existing LexonFabric request and output contracts where practical rather than inventing a second indexing protocol.
- **UR-SCALE-14 [ASSUMPTION]:** When multiple rsync URLs are provided in one run, the tool produces one combined run output and one root handoff artifact for that run.

## Change Manifest

| ID | Type | Summary | Traceability |
|---|---|---|---|
| CM-SCALE-001 | Add | Introduce a new structured requirements artifact for `lexonfabric-scale-test` as a separate local wrapper/test tool | UR-SCALE-1, UR-SCALE-2, UR-SCALE-3 |
| CM-SCALE-002 | Add | Define one-or-more rsync URL inputs for large-scale local mailbox acquisition | UR-SCALE-4, UR-SCALE-5 |
| CM-SCALE-003 | Add | Define the ordered local workflow: rsync fetch, mailbox discovery, generated request/config, delegated parser/indexer run, root handoff | UR-SCALE-5, UR-SCALE-6, UR-SCALE-7, UR-SCALE-8 |
| CM-SCALE-004 | Add | Define the tool purpose as parser stress testing and block-tree production rather than MCP-serving preparation | UR-SCALE-7, UR-SCALE-9 |
| CM-SCALE-005 | Add | Constrain the first realization to a lightweight Linux-local operator form such as a bash script | UR-SCALE-10, UR-SCALE-12 |
| CM-SCALE-006 | Add | Preserve local-only scope while keeping production orchestration out of this increment | UR-SCALE-10, UR-SCALE-11 |
| CM-SCALE-007 | Add | Require stable contract reuse so the wrapper composes existing LexonFabric request and output shapes where practical | UR-SCALE-13, UR-SCALE-14 |

## Before / After

### BA-SCALE-001

- **Before [KNOWN]:** The repository had no structured requirements artifact for a local rsync-driven stress-test wrapper layered above existing LexonFabric indexing behavior.
- **After [KNOWN]:** The repository has an explicit requirements baseline for `lexonfabric-scale-test` in `docs/specs/lexonfabric-scale-test/requirements.md`.

### BA-SCALE-002

- **Before [KNOWN]:** Local examples assumed hand-authored request/config files and direct indexer invocation.
- **After [KNOWN]:** `lexonfabric-scale-test` may generate the request/config inputs from rsync-discovered mailbox files before invoking the existing downstream indexing flow.

### BA-SCALE-003

- **Before [KNOWN]:** The rsync-driven workflow had not been separated architecturally from `lexonfabric-indexer`.
- **After [KNOWN]:** The rsync-driven workflow is explicitly defined as a separate local wrapper/test tool rather than part of the indexer feature boundary.

### BA-SCALE-004

- **Before [KNOWN]:** The wrapper implementation vehicle was open across crate, service, or script options.
- **After [KNOWN]:** The first realization may be a simple Linux bash script, provided it preserves the approved ordered workflow and required artifacts.

### BA-SCALE-005

- **Before [KNOWN]:** MCP-serving preparation was still a plausible wrapper output.
- **After [KNOWN]:** MCP config generation is explicitly out of scope; the required output is the block tree and root handoff artifact.

## Requirements

### Functional Requirements

#### RQ-SCALE-001 - Wrapper boundary

LexonFabric SHALL provide a separate local wrapper/test tool named `lexonfabric-scale-test`.

- **Boundary [KNOWN]:** This tool is not part of `lexonfabric-indexer`.
- **Non-goal [KNOWN]:** This tool does not define repository-local indexing semantics or MCP semantics.
- **Traceability:** UR-SCALE-1, UR-SCALE-2, UR-SCALE-3

#### RQ-SCALE-002 - Rsync source inputs

`lexonfabric-scale-test` SHALL accept one or more rsync URLs as workflow inputs.

- **Traceability:** UR-SCALE-4

#### RQ-SCALE-003 - Ordered local stress-test workflow

`lexonfabric-scale-test` SHALL orchestrate the local workflow in ordered stages:

1. fetch rsync sources
2. discover mailbox files
3. generate an indexer-compatible request/config from the discovered mailbox set
4. invoke the existing LexonFabric parser/indexer flow
5. emit block-tree handoff artifacts

- **Constraint [KNOWN]:** The workflow is local/testing-only in this increment.
- **Traceability:** UR-SCALE-5, UR-SCALE-6, UR-SCALE-7, UR-SCALE-8, UR-SCALE-10

#### RQ-SCALE-003A - Minimal operator realization

The first `lexonfabric-scale-test` realization SHALL be allowed to use a simple Linux operator form such as a bash script.

- **Constraint [KNOWN]:** The implementation vehicle may stay lightweight so long as it preserves the approved ordered workflow and output artifacts.
- **Non-goal [KNOWN]:** This increment does not require a dedicated Rust crate, long-lived service, or control-plane component.
- **Traceability:** UR-SCALE-12

#### RQ-SCALE-004 - Delegated parser/indexer use

`lexonfabric-scale-test` SHALL invoke the existing LexonFabric batch contract as a downstream dependency rather than moving this workflow into `lexonfabric-indexer`.

- **Required property [KNOWN]:** The tool stress-tests existing parser/indexer behavior through generated inputs.
- **Traceability:** UR-SCALE-2, UR-SCALE-6, UR-SCALE-7, UR-SCALE-13

#### RQ-SCALE-005 - Mailbox discovery expansion

For fetched rsync mirrors, `lexonfabric-scale-test` SHALL discover mailbox files and translate the discovered set into indexer-compatible mailbox batch items.

- **Extensibility [KNOWN]:** This must not preclude future wrapper support for documents or other content classes with different metadata handling.
- **Filtering gap [UNKNOWN]:** Exact mailbox eligibility and include/exclude rules are not yet specified.
- **Traceability:** UR-SCALE-6

#### RQ-SCALE-006 - Block-tree output

`lexonfabric-scale-test` SHALL produce a block tree from the generated run.

- **Current baseline [KNOWN]:** The downstream LexonFabric flow already emits summary/root information sufficient to identify the produced tree root.
- **Traceability:** UR-SCALE-7, UR-SCALE-8

#### RQ-SCALE-007 - Root handoff artifact

`lexonfabric-scale-test` SHALL emit a machine-consumable handoff artifact containing the root identifier or reference for the produced block tree.

- **Boundary [KNOWN]:** This handoff is for inspection or downstream use; MCP config generation is not required in this increment.
- **Traceability:** UR-SCALE-8, UR-SCALE-9

#### RQ-SCALE-008 - No MCP-config generation in scope

The first `lexonfabric-scale-test` increment SHALL NOT require generation of MCP server configuration artifacts.

- **Rationale [KNOWN]:** The approved scope is stress testing and block-tree production only.
- **Traceability:** UR-SCALE-9

#### RQ-SCALE-009 - Local-only execution scope

The first `lexonfabric-scale-test` increment SHALL be executable for large-scale local testing and SHALL NOT define the production orchestration workflow.

- **Preserved seam [KNOWN]:** Production remains a separate ARM/Bicep plus Azure Functions concern.
- **Traceability:** UR-SCALE-10, UR-SCALE-11

#### RQ-SCALE-009A - Linux-local execution shape

The first executable `lexonfabric-scale-test` realization SHALL target a Linux-oriented local execution environment compatible with the existing containerized local workflow.

- **Rationale [INFERRED]:** A bash-script realization implies a Linux-shaped operator environment and aligns with the repository's existing container-oriented local profile.
- **Traceability:** UR-SCALE-5, UR-SCALE-10, UR-SCALE-12

#### RQ-SCALE-010 - Stable contract reuse

`lexonfabric-scale-test` SHALL reuse existing LexonFabric-compatible request and output contracts where practical.

- **Constraint [INFERRED]:** The wrapper should compose existing shapes rather than inventing a parallel protocol without need.
- **Traceability:** UR-SCALE-13

#### RQ-SCALE-011 - Combined run output

When multiple rsync URLs are provided in one run, `lexonfabric-scale-test` SHALL produce one coherent output set for that run.

- **Assumption [ASSUMPTION]:** One run yields one combined root handoff artifact.
- **Traceability:** UR-SCALE-4, UR-SCALE-8, UR-SCALE-14

### Boundary and Invariant Requirements

#### RQ-SCALE-012 - Indexer/MCP separation

`lexonfabric-scale-test` SHALL remain limited to local orchestration, input materialization, and delegated batch execution and SHALL NOT redefine indexer semantics or MCP-serving behavior.

- **Rationale [KNOWN]:** The user explicitly scoped this tool as a wrapper/test tool rather than an indexer or MCP feature.
- **Traceability:** UR-SCALE-2, UR-SCALE-9, UR-SCALE-13

#### RQ-SCALE-013 - Subordinate downstream contracts

`lexonfabric-scale-test` SHALL remain subordinate to the public contracts already owned by downstream LexonFabric entrypoints and their delegated LexonGraph dependencies and SHALL NOT redefine block-construction, parser, or embedding semantics within this repository.

- **Rationale [INFERRED]:** The wrapper exists to stress-test existing behavior, not to invent an alternative indexing protocol.
- **Traceability:** UR-SCALE-2, UR-SCALE-7, UR-SCALE-13

#### RQ-SCALE-014 - Future content extensibility

`lexonfabric-scale-test` SHALL preserve room for future non-mailbox stress-test inputs without redefining the core wrapper contract.

- **Initial focus [KNOWN]:** rsync-backed mailbox acquisition
- **Constraint [INFERRED]:** Future document-specific or other content-specific discovery policies should fit behind the same wrapper boundary.
- **Traceability:** UR-SCALE-6, UR-SCALE-13

## Out of Scope

- Moving this workflow into `lexonfabric-indexer`
- Defining repository-local indexing, parser, block-construction, or embedding algorithms
- Defining MCP server behavior or generating MCP configuration artifacts
- Defining the production ARM/Bicep plus Azure Functions workflow
- Requiring a dedicated Rust crate or service for the first wrapper realization
- Finalizing exact mailbox eligibility or include/exclude filtering policy

## Invariant Impact Assessment

| Invariant | Impact | Assessment |
|---|---|---|
| Indexing remains separate from search serving | Preserved | The wrapper is limited to local orchestration and delegated execution |
| `lexonfabric-indexer` remains focused on indexing contracts | Preserved | The rsync stress-test flow is explicitly outside the indexer boundary |
| Local/testing remains self-contained and batch-oriented | Preserved | The wrapper remains Linux-local and stage-ordered around batch execution |
| Production seams remain open | Preserved | Production orchestration remains a separate future workflow |
| Future content extensibility remains intact | Preserved | The wrapper adds mailbox stress testing now without closing off later document handling |
| LexonFabric remains subordinate to LexonGraph contracts | Preserved | The wrapper drives existing downstream flows rather than redefining block construction |

## Open Questions / Discovery Gaps

- **Q-SCALE-001 [UNKNOWN]:** What exact mailbox file patterns qualify for discovery from fetched rsync mirrors?
- **Q-SCALE-002 [UNKNOWN]:** Should the generated request/config artifact be persisted as a durable fixture, ephemeral run output, or both?
- **Q-SCALE-003 [UNKNOWN]:** What metadata from the rsync source URL, if any, must be preserved in generated mailbox batch items?
- **Q-SCALE-004 [UNKNOWN]:** If one rsync source is unreachable during a multi-source run, should the wrapper fail the whole run or allow partial stress-test completion?

## Coverage Notes

- **Covered sources [KNOWN]:**
  - `README.md:91-168`
  - `README.md:183-203`
  - `docker-compose.yml:1-45`
  - `examples/local/request.sample.json:1-34`
  - user clarification in this session: "I don't want this to be part of the lexonfabric indexer. This is a wrapper / test tool built on top of it"
  - user clarification in this session: "This tool is just a way to run a local stress test on the LexonFabric parser and produce a block tree. Feel free to name it lexonfabric-scale-test or something along those lines."
  - user clarification in this session selecting block tree/root handoff only rather than MCP config output
  - user clarification in this session: "This could be as simple as a Linux bash script, no need for anything fancy here."
- **Excluded for now [KNOWN]:**
  - Exact generated file locations and directory layout
  - Specific script flags, shell ergonomics, and Docker Compose command lines
  - Rust implementation files, tests, and operational assets
