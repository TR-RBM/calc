# Security policy

## What counts as a vulnerability

calc reads expressions, session files and requests in JSON, and writes session files and images where it is told to. A vulnerability is a case where crafted input makes calc do more than that. Examples:

- input that makes calc read or write a file other than the ones named on its command line;
- input that makes calc run another program;
- a session file or request that makes calc crash with memory corruption, or use memory or time without bound where the documentation says a limit applies.

A wrong answer is a serious bug, but report it as an ordinary issue with the input that shows it, unless it can be used against someone who relies on calc.

If you are unsure, report it privately.

## How to report

Report privately. Do not open a public issue, and do not publish details, before a fix is available.

Send the report by e-mail to the maintainer, Tim Richter, at `info@richter-it-service.eu`. Include:

- the output of `calc --version`;
- the exact command and input, and what you expected and what happened;
- the distribution and architecture.

Do not include real credentials or private data.

## What happens next

The maintainers confirm that the report arrived, examine it, and tell you whether they regard it as a vulnerability and why. A confirmed vulnerability is fixed in a new release. Please allow time for a fix before you publish.

## Supported versions

Fixes are made on the current release. Earlier releases receive no fixes.
